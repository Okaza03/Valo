//! Fullscreen without a white strip where the taskbar sits (Windows).
//!
//! The window has no system frame. For such windows winit clips a *maximized*
//! window's client area to the monitor's work area (the screen minus the
//! taskbar), and going fullscreen doesn't clear the maximized state. So a window
//! that was maximized before going fullscreen leaves the taskbar's strip outside
//! its client area, and Windows paints it white.
//!
//! Un-maximizing first avoids that but makes the window visibly shrink and grow
//! again. Instead we hook the window's messages ahead of winit and, while
//! fullscreen, make the whole window the client area. The window stays
//! maximized, so entering and leaving fullscreen is a single resize.
use std::sync::atomic::{AtomicBool, Ordering};

static FULLSCREEN: AtomicBool = AtomicBool::new(false);

/// Tell the hook whether the window is (about to be) fullscreen.
pub fn set(on: bool) {
    FULLSCREEN.store(on, Ordering::Relaxed);
}

#[cfg(windows)]
pub fn install(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::Shell::SetWindowSubclass;

    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    // SAFETY: the HWND is our live main window, and this runs on the thread that owns it.
    unsafe {
        SetWindowSubclass(h.hwnd.get() as _, Some(subclass_proc), 1, 0);
    }
}

/// The same, for a window that isn't eframe's (the Iced UI).
#[cfg(all(windows, feature = "iced"))]
pub fn install_hwnd(hwnd: isize) {
    use windows_sys::Win32::UI::Shell::SetWindowSubclass;
    // SAFETY: the HWND is our live main window, and Iced runs this on the thread that owns it.
    unsafe {
        SetWindowSubclass(hwnd as _, Some(subclass_proc), 1, 0);
    }
}

#[cfg(not(windows))]
pub fn install(_cc: &eframe::CreationContext<'_>) {}

#[cfg(windows)]
unsafe extern "system" fn subclass_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _id: usize,
    _data: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::Shell::DefSubclassProc;
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_NCCALCSIZE;

    // Returning 0 with the proposed rectangle untouched makes the whole window
    // the client area, instead of winit's work-area clip for maximized windows.
    if msg == WM_NCCALCSIZE && wparam != 0 && FULLSCREEN.load(Ordering::Relaxed) {
        return 0;
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
