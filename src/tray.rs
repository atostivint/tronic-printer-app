//! An icon in the Windows notification area that says what the printer is doing, and brings the
//! window back on a click.
//!
//! Closing the window minimises it instead of quitting (`chrome`'s subclass turns `WM_CLOSE` into
//! a minimise): the printer link, the MCP endpoint and this icon live on, and the icon is the way
//! back. Quit in the icon's menu is what really ends the app.
//!
//! The shell answers `Shell_NotifyIconW` through a window procedure, so the icon needs a window and
//! a message loop of its own: Rune's loop is busy drawing and must not be blocked in `GetMessage`.
//! Hence a thread that owns a 0x0 window (never shown), installs the icon there and pumps that
//! window's messages. The window is deliberately not message-only: a `HWND_MESSAGE` window never
//! receives the shell's tray callbacks.
//!
//! Windows only. Everywhere else `start` and `announce` do nothing.

#[cfg(windows)]
pub use win::{announce, start};

/// There is no notification area to put an icon in.
#[cfg(not(windows))]
pub fn start(_title: &'static str, _status: &str) {}

/// Nothing to keep up to date.
#[cfg(not(windows))]
pub fn announce(_status: &str) {}

#[cfg(windows)]
mod win {
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock, mpsc};
    use std::thread;

    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIN_SELECT, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW, GetCursorPos, GetSystemMetrics, HICON,
        IDI_APPLICATION, IMAGE_ICON, LR_DEFAULTCOLOR, LR_LOADFROMFILE, LoadIconW, LoadImageW, MF_SEPARATOR, MF_STRING, MSG,
        MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, PostQuitMessage, QS_ALLINPUT,
        RegisterClassW, SM_CXSMICON, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP,
        WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP, WM_QUIT, WM_RBUTTONUP, WNDCLASSW, WINDOW_EX_STYLE, WINDOW_STYLE,
    };
    use windows::core::PCWSTR;

    use crate::chrome;

    /// The message the shell sends us for every click on the icon (the app owns `WM_APP` upward).
    const CLICK: u32 = WM_APP + 1;
    /// Menu commands, ours to choose.
    const OPEN: usize = 1;
    const QUIT: usize = 2;

    /// The status the icon should be showing. The tray thread reads it when it refreshes the
    /// tooltip; the channel below only says that it changed.
    static STATUS: Mutex<String> = Mutex::new(String::new());
    /// The way to tell the tray thread there is something new for it. `None` until `start`.
    static NUDGE: OnceLock<mpsc::Sender<()>> = OnceLock::new();
    /// The main window's title, so a click finds the window without an argument.
    static TITLE: OnceLock<&'static str> = OnceLock::new();

    /// The tray icon and its thread, started once. `status` is what the tooltip says at first.
    pub fn start(title: &'static str, status: &str) {
        let (tx, rx) = mpsc::channel();
        // A second call (a second app in the same process, say) keeps the first icon.
        if NUDGE.set(tx).is_err() {
            return;
        }
        *lock(&STATUS) = status.to_string();
        let first = status.to_string();
        thread::spawn(move || run(title, first, rx));
    }

    /// Say what the icon should show. An unchanged status costs one comparison and nothing else:
    /// `poll` calls this on every frame.
    pub fn announce(status: &str) {
        {
            let mut shown = lock(&STATUS);
            if *shown == status {
                return;
            }
            *shown = status.to_string();
        }
        if let Some(nudge) = NUDGE.get() {
            // A full channel already says "look again"; a dead thread is not worth a panic.
            let _ = nudge.send(());
        }
    }

    /// The lock, whether or not a panic somewhere else poisoned it: a stale tooltip is not worth
    /// losing the icon over.
    fn lock<T>(cell: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        cell.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A null-terminated UTF-16 copy of `text`, for the wide Win32 calls.
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Copy `text` into the tooltip field (127 units and a terminator), clearing what was there.
    fn put_tip(nid: &mut NOTIFYICONDATAW, text: &str) {
        nid.szTip = [0; 128];
        let room = nid.szTip.len() - 1;
        for (slot, unit) in nid.szTip.iter_mut().zip(text.encode_utf16().take(room)) {
            *slot = unit;
        }
    }

    /// The app's own icon. The `.ico` in the source tree wins, so a change to it shows without a
    /// rebuild; `build.rs` also embeds a copy in the executable (id 1), which is what a shipped
    /// copy finds; the generic application icon is the last resort.
    fn icon() -> HICON {
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) };
        let mut candidates = vec![PathBuf::from("assets").join("icon.ico")];
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)) {
            candidates.push(dir.join("assets").join("icon.ico"));
        }
        for path in candidates {
            let Some(text) = path.to_str() else { continue };
            let text = wide(text);
            if let Ok(loaded) = unsafe { LoadImageW(None, PCWSTR(text.as_ptr()), IMAGE_ICON, size, size, LR_LOADFROMFILE) } {
                return HICON(loaded.0);
            }
        }
        let embedded = unsafe { GetModuleHandleW(None) }
            .ok()
            .and_then(|module| unsafe { LoadImageW(Some(HINSTANCE(module.0)), PCWSTR(1 as *const u16), IMAGE_ICON, size, size, LR_DEFAULTCOLOR) }.ok());
        match embedded {
            Some(loaded) => HICON(loaded.0),
            None => unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap_or_default(),
        }
    }

    /// Bring the main window back: restore it if it was minimised, then raise it.
    fn open_window() {
        if let Some(title) = TITLE.get() {
            let _ = chrome::show_window(title);
        }
    }

    /// End the process: the one thing the window's close button no longer does.
    ///
    /// Leaving outright is the reliable way here. `WM_CLOSE` on the main window now only hides it,
    /// and `DestroyWindow` refuses to work on another thread's window, so driving the toolkit out
    /// of its own loop from here would be a race with its event loop. Nothing is pending on the
    /// way out: settings are written as they change (`settings::update`, on every edit).
    fn quit_app() {
        std::process::exit(0);
    }

    /// The right-click menu: Open brings the window back, Quit ends the app.
    fn popup(hwnd: HWND) {
        unsafe {
            let Ok(menu) = CreatePopupMenu() else { return };
            let (open, quit) = (wide("Open"), wide("Quit"));
            let _ = AppendMenuW(menu, MF_STRING, OPEN, PCWSTR(open.as_ptr()));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, QUIT, PCWSTR(quit.as_ptr()));
            // Windows only closes a tray menu that the user clicks away from when its owner is the
            // foreground window. A right click on our own icon is a user gesture, so this sticks.
            let _ = SetForegroundWindow(hwnd);
            let mut at = POINT::default();
            let _ = GetCursorPos(&mut at);
            // `TPM_RETURNCMD` puts the chosen command in the return value: no `WM_COMMAND` to route.
            let picked = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, at.x, at.y, None, hwnd, None).0 as usize;
            let _ = DestroyMenu(menu);
            match picked {
                OPEN => open_window(),
                QUIT => quit_app(),
                _ => {}
            }
        }
    }

    /// The window procedure of the icon's window: only the clicks the shell relays, and the end.
    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            CLICK => {
                // Before shell version 4 the callback carries the mouse message itself (the low
                // word is all of it); version 4 carries `NIN_SELECT` for a selection. Handle both.
                match lparam.0 as u32 & 0xffff {
                    WM_LBUTTONUP | NIN_SELECT => open_window(),
                    WM_RBUTTONUP | WM_CONTEXTMENU => popup(hwnd),
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    /// The tray thread: create the window, add the icon, then serve clicks and status changes until
    /// the window is destroyed.
    fn run(title: &'static str, first: String, rx: mpsc::Receiver<()>) {
        let _ = TITLE.set(title);
        unsafe {
            let Ok(module) = GetModuleHandleW(None) else { return };
            let instance = HINSTANCE(module.0);
            let class = wide("TronicPrinterTray");
            let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: instance, lpszClassName: PCWSTR(class.as_ptr()), ..Default::default() };
            if RegisterClassW(&wc) == 0 {
                return;
            }
            // 0x0 and never shown: it exists only to own the icon and its callback.
            let hwnd = match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class.as_ptr()),
                PCWSTR(class.as_ptr()),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            ) {
                Ok(hwnd) => hwnd,
                Err(_) => return,
            };
            let mut nid = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: 1,
                uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
                uCallbackMessage: CLICK,
                hIcon: icon(),
                ..Default::default()
            };
            put_tip(&mut nid, &first);
            if !Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                // No notification area to take it (Explorer is down): the app still runs without.
                eprintln!("tray: Shell_NotifyIconW(NIM_ADD) failed, no icon for this session");
                return;
            }
            loop {
                // A nudge means the status changed; the text itself lives in STATUS.
                let mut nudged = false;
                while rx.try_recv().is_ok() {
                    nudged = true;
                }
                if nudged {
                    let status = lock(&STATUS).clone();
                    put_tip(&mut nid, &status);
                    nid.uFlags = NIF_TIP;
                    let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
                }
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                        return;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                // Sleep until a click (a posted message ends the wait at once) or until the status
                // may have been nudged: the channel cannot wake the message queue, this timeout is
                // what picks a new status up. A quarter second of stale tooltip is invisible.
                MsgWaitForMultipleObjectsEx(None, 250, QS_ALLINPUT, MSG_WAIT_FOR_MULTIPLE_OBJECTS_EX_FLAGS(0));
            }
        }
    }
}
