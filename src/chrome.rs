//! A title bar that belongs to the app. Rune keeps the native Windows frame, so once the window
//! exists we ask Windows 11's compositor to paint that frame in the app's colours: dark
//! controls, the palette's caption colour, a hairline border and round corners. The colours come
//! from `style::pal()`, so a palette change repaints the frame along with the body.
//!
//! Windows only. On Linux and macOS the window keeps the desktop's own frame and everything
//! here is a no-op.

// The fields are read by the Windows implementation only.
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Look {
    pub caption: u32,
    pub text: u32,
    pub border: u32,
}

/// Windows wants `0x00BBGGRR`.
#[cfg_attr(not(windows), allow(dead_code))]
pub const fn colorref(rgb: u32) -> u32 {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff)
}

#[cfg(windows)]
pub use win::{animations_enabled, dress_when_ready, repaint, show_window};

/// There is no "Show animations" setting to read here: effects start on.
#[cfg(not(windows))]
pub fn animations_enabled() -> bool {
    true
}

/// The desktop draws the frame; nothing to do.
#[cfg(not(windows))]
pub fn dress_when_ready(_title: &'static str) {}

/// The desktop draws the frame; there is nothing to repaint either.
#[cfg(not(windows))]
pub fn repaint() {}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::time::Duration;

    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, EnumWindows, GWLP_WNDPROC, GetSystemMetrics, GetWindowTextW, GetWindowThreadProcessId, IMAGE_ICON, IsIconic, IsWindow,
        IsWindowVisible, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SPI_GETCLIENTAREAANIMATION, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageW, SetForegroundWindow, SetWindowLongPtrW, ShowWindow, SystemParametersInfoW, WM_CLOSE,
        WM_SETICON, WNDPROC,
    };
    use windows::core::{BOOL, PCWSTR};

    use super::{Look, colorref};

    /// Whether Windows' "Show animations" setting is on. Effects default to off when it is not.
    pub fn animations_enabled() -> bool {
        let mut on = BOOL(1);
        let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut on as *mut BOOL as *mut c_void), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
        ok.is_err() || on.as_bool()
    }

    const DARK_MODE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(20);
    const CORNERS: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(33);
    const BORDER: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
    const CAPTION: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(35);
    const CAPTION_TEXT: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(36);
    const ROUND: u32 = 2;

    /// The frame colours of the palette being worn, read now: `set_palette` changes the answer.
    fn look() -> Look {
        let frame = crate::style::pal().frame;
        Look { caption: frame.caption, text: frame.text, border: frame.border }
    }

    /// The window we dressed, as a raw handle so this thread-safe cell can hold it. 0 until the
    /// window has been found.
    static DRESSED: AtomicIsize = AtomicIsize::new(0);

    /// The procedure the window had before ours (the GUI toolkit's own), as a raw address: 0 until
    /// the subclass is installed. Kept so every message but the close goes where it used to.
    static OLD_PROC: AtomicIsize = AtomicIsize::new(0);

    /// The one message our subclass takes: closing the window.
    pub(super) fn is_close(msg: u32) -> bool {
        msg == WM_CLOSE
    }

    /// Closing the window minimises it instead of destroying it, so the app (its Bluetooth link,
    /// its MCP endpoint, the tray icon) keeps running and the tray is the way back. Winit never
    /// sees the close, so its event loop does not end.
    ///
    /// A minimised window, not a hidden one: Rune keeps polling for a window it knows is shown,
    /// but a hidden one stops its turns and the app goes deaf (no MCP reply, no printer tick). A
    /// minimised window still draws nothing and still keeps the taskbar button to come back with.
    ///
    /// Every other message is chained to the procedure that was there before, through
    /// `CallWindowProcW`, which is what knows how to call it (a procedure cannot be called
    /// directly from Rust).
    unsafe extern "system" fn on_close(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if is_close(msg) {
            let _ = unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
            return LRESULT(0);
        }
        let old: WNDPROC = unsafe { std::mem::transmute(OLD_PROC.load(Ordering::Relaxed)) };
        unsafe { CallWindowProcW(old, hwnd, msg, wparam, lparam) }
    }

    /// Put `on_close` in front of the window's procedure, once. Called from this module's dressing
    /// thread, not from the window's: changing a procedure of a window of this same process is
    /// allowed, and the chain keeps the toolkit's own behaviour for everything else.
    fn minimize_on_close(hwnd: HWND) {
        if OLD_PROC.load(Ordering::Relaxed) != 0 {
            return;
        }
        let proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT = on_close;
        // A failed call returns 0 and installs nothing; a window always had a procedure, so a 0
        // here means the subclass is not in place and must not be chained to.
        let old = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, proc as usize as isize) };
        if old != 0 {
            OLD_PROC.store(old, Ordering::Relaxed);
        }
    }

    /// Dress the window called `title` once it appears. Runs on its own thread and gives up after
    /// a few seconds; on Windows 10 the colour attributes are simply ignored.
    pub fn dress_when_ready(title: &'static str) {
        std::thread::spawn(move || {
            for _ in 0..100 {
                if let Some(hwnd) = find_window(title) {
                    DRESSED.store(hwnd.0 as isize, Ordering::Relaxed);
                    apply(hwnd, &look());
                    minimize_on_close(hwnd);
                    // The frame is repainted a moment after creation: once more, to be sure. The
                    // palette is read again, in case it changed in the meantime.
                    std::thread::sleep(Duration::from_millis(400));
                    paint(hwnd, &look());
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }

    /// Repaint the frame in the colours of the palette being worn, so it switches with the body
    /// without a restart. Cheap and repeatable: it is one `DwmSetWindowAttribute` per colour on the
    /// window already found, nothing is queued, and before that window exists (or after it is gone)
    /// it does nothing. `DwmSetWindowAttribute` is a plain request to the compositor for a window of
    /// this process, so calling it from the UI thread is fine.
    pub fn repaint() {
        let raw = DRESSED.load(Ordering::Relaxed);
        if raw == 0 {
            return;
        }
        let hwnd = HWND(raw as *mut c_void);
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            DRESSED.store(0, Ordering::Relaxed);
            return;
        }
        paint(hwnd, &look());
    }

    fn set(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: u32) {
        // Failures are fine: an older Windows does not know the attribute.
        let _ = unsafe { DwmSetWindowAttribute(hwnd, attribute, &value as *const u32 as *const c_void, 4) };
    }

    /// The logo, from the icon resource `build.rs` embedded (id 1), at the sizes the title bar
    /// (small) and Alt+Tab (big) want on this display.
    fn set_icons(hwnd: HWND) {
        let Ok(module) = (unsafe { GetModuleHandleW(None) }) else { return };
        let instance = HINSTANCE(module.0);
        for (kind, metric) in [(0usize, SM_CXSMICON), (1usize, SM_CXICON)] {
            let size = unsafe { GetSystemMetrics(metric) };
            if let Ok(icon) = unsafe { LoadImageW(Some(instance), PCWSTR(1 as *const u16), IMAGE_ICON, size, size, LR_DEFAULTCOLOR) } {
                unsafe { SendMessageW(hwnd, WM_SETICON, Some(WPARAM(kind)), Some(LPARAM(icon.0 as isize))) };
            }
        }
    }

    fn apply(hwnd: HWND, look: &Look) {
        set_icons(hwnd);
        paint(hwnd, look);
    }

    /// The colours alone, without touching the icon: what a palette switch needs.
    fn paint(hwnd: HWND, look: &Look) {
        set(hwnd, DARK_MODE, 1);
        set(hwnd, CORNERS, ROUND);
        set(hwnd, CAPTION, colorref(look.caption));
        set(hwnd, CAPTION_TEXT, colorref(look.text));
        set(hwnd, BORDER, colorref(look.border));
    }

    struct Search {
        /// The window's title starts with this. Only the start is fixed: the rest of the title is
        /// the status line (`Tronic Printer · Connected · 87%`), so an exact match would only find
        /// the window while the printer is offline.
        caption: &'static str,
        pid: u32,
        /// A minimised window is still ours: the tray has to find one that its close button put
        /// away.
        visible_only: bool,
        found: Option<HWND>,
    }

    unsafe extern "system" fn visit(hwnd: HWND, param: LPARAM) -> BOOL {
        let search = unsafe { &mut *(param.0 as *mut Search) };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        let shown = !search.visible_only || unsafe { IsWindowVisible(hwnd) }.as_bool();
        if pid == search.pid && shown {
            let mut buf = [0u16; 128];
            let n = unsafe { GetWindowTextW(hwnd, &mut buf) } as usize;
            if String::from_utf16_lossy(&buf[..n.min(buf.len())]).starts_with(search.caption) {
                search.found = Some(hwnd);
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    /// Bring the window called `title` back to the front: restore it if it was minimised, then
    /// raise it. False when it is not there (yet, or any more). A tray click is a user gesture, so
    /// Windows normally grants the foreground.
    pub fn show_window(title: &'static str) -> bool {
        let Some(hwnd) = find_any_window(title) else { return false };
        unsafe {
            let cmd = if IsIconic(hwnd).as_bool() { SW_RESTORE } else { SW_SHOW };
            let _ = ShowWindow(hwnd, cmd);
            let _ = SetForegroundWindow(hwnd);
        }
        true
    }

    /// The window whose title starts with `title`, in this process and on screen: the one to dress.
    pub fn find_window(title: &'static str) -> Option<HWND> {
        search(title, true)
    }

    /// The window whose title starts with `title`, shown or not.
    fn find_any_window(title: &'static str) -> Option<HWND> {
        search(title, false)
    }

    fn search(caption: &'static str, visible_only: bool) -> Option<HWND> {
        let mut search = Search { caption, pid: unsafe { GetCurrentProcessId() }, visible_only, found: None };
        let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize)) };
        search.found
    }
}

#[cfg(test)]
mod tests {
    use super::colorref;

    #[test]
    fn night_keeps_the_frame_the_app_was_painted_with_before() {
        // These three numbers used to be hard-coded in `main.rs`: moving them into the palettes
        // must leave Night exactly as it was.
        let night = crate::style::PaletteId::Night.palette();
        assert_eq!((night.frame.caption, night.frame.text, night.frame.border), (0x1a1228, 0xf4effa, 0x3a2d52));
    }

    #[test]
    fn colours_are_swapped_into_windows_byte_order() {
        assert_eq!(colorref(0x1a1228), 0x28121a);
        assert_eq!(colorref(0xff0000), 0x0000ff);
        assert_eq!(colorref(0x00ff00), 0x00ff00);
    }

    /// The subclass itself can only be checked by hand (a `WndProc` needs a real window and a
    /// message loop); what it decides is a pure question, and this pins the answer: the close is
    /// swallowed, and everything else — the destroy above all — still reaches Win32.
    #[cfg(windows)]
    #[test]
    fn only_the_close_is_taken_by_the_subclass() {
        use windows::Win32::UI::WindowsAndMessaging::{WM_CLOSE, WM_DESTROY, WM_NULL, WM_SYSCOMMAND};
        assert!(super::win::is_close(WM_CLOSE));
        assert!(!super::win::is_close(WM_DESTROY));
        assert!(!super::win::is_close(WM_SYSCOMMAND));
        assert!(!super::win::is_close(WM_NULL));
    }
}
