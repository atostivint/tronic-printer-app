//! A title bar that belongs to the app. Rune keeps the native Windows frame, so once the window
//! exists we ask Windows 11's compositor to paint that frame in the app's colours: dark
//! controls, the night-violet caption, a hairline border and round corners.

use std::ffi::c_void;
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWINDOWATTRIBUTE, DwmSetWindowAttribute};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetSystemMetrics, GetWindowTextW, GetWindowThreadProcessId, IMAGE_ICON, IsWindowVisible, LR_DEFAULTCOLOR, LoadImageW,
    SM_CXICON, SM_CXSMICON, SendMessageW, WM_SETICON,
};
use windows::core::{BOOL, PCWSTR};

/// Windows wants `0x00BBGGRR`.
pub const fn colorref(rgb: u32) -> u32 {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff)
}

const DARK_MODE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(20);
const CORNERS: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(33);
const BORDER: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
const CAPTION: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(35);
const CAPTION_TEXT: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(36);
const ROUND: u32 = 2;

pub struct Look {
    pub title: &'static str,
    pub caption: u32,
    pub text: u32,
    pub border: u32,
}

/// Dress the window called `look.title` once it appears. Runs on its own thread and gives up
/// after a few seconds; on Windows 10 the colour attributes are simply ignored.
pub fn dress_when_ready(look: Look) {
    std::thread::spawn(move || {
        for _ in 0..100 {
            if let Some(hwnd) = find_window(look.title) {
                apply(hwnd, &look);
                // The frame is repainted a moment after creation: once more, to be sure.
                std::thread::sleep(Duration::from_millis(400));
                apply(hwnd, &look);
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
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
    set(hwnd, DARK_MODE, 1);
    set(hwnd, CORNERS, ROUND);
    set(hwnd, CAPTION, colorref(look.caption));
    set(hwnd, CAPTION_TEXT, colorref(look.text));
    set(hwnd, BORDER, colorref(look.border));
}

struct Search {
    title: &'static str,
    pid: u32,
    found: Option<HWND>,
}

unsafe extern "system" fn visit(hwnd: HWND, param: LPARAM) -> BOOL {
    let search = unsafe { &mut *(param.0 as *mut Search) };
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == search.pid && unsafe { IsWindowVisible(hwnd) }.as_bool() {
        let mut buf = [0u16; 128];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) } as usize;
        if String::from_utf16_lossy(&buf[..n.min(buf.len())]) == search.title {
            search.found = Some(hwnd);
            return BOOL(0);
        }
    }
    BOOL(1)
}

fn find_window(title: &'static str) -> Option<HWND> {
    let mut search = Search { title, pid: unsafe { GetCurrentProcessId() }, found: None };
    let _ = unsafe { EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize)) };
    search.found
}

#[cfg(test)]
mod tests {
    use super::colorref;

    #[test]
    fn colours_are_swapped_into_windows_byte_order() {
        assert_eq!(colorref(0x1a1228), 0x28121a);
        assert_eq!(colorref(0xff0000), 0x0000ff);
        assert_eq!(colorref(0x00ff00), 0x00ff00);
    }
}
