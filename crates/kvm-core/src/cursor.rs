/// Cursor warp (absolute move) and visibility control.
///
/// Warp uses enigo's Abs coordinate mode.
/// Visibility uses platform FFI: CoreGraphics on macOS, ShowCursor on Windows.
use anyhow::Result;
use enigo::{Coordinate, Enigo, Mouse, Settings};

pub struct Cursor {
    enigo: Enigo,
    is_hidden: bool,
}

impl Cursor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            enigo: Enigo::new(&Settings::default())?,
            is_hidden: false,
        })
    }

    /// Move the OS cursor to an absolute position.
    pub fn warp(&mut self, x: i32, y: i32) -> Result<()> {
        self.enigo.move_mouse(x, y, Coordinate::Abs)?;
        Ok(())
    }

    /// Show or hide the cursor. No-op if already in the requested state.
    pub fn set_visible(&mut self, visible: bool) {
        if visible == !self.is_hidden {
            return;
        }
        self.is_hidden = !visible;
        platform::set_cursor_visible(visible);
    }
}

/// Warp the cursor to an absolute position without Enigo (Send-safe, any thread).
pub fn warp_sync(x: i32, y: i32) {
    platform::warp(x, y);
}

/// Hide or show the cursor (Send-safe, any thread).
pub fn set_visible_sync(visible: bool) {
    platform::set_cursor_visible(visible);
}

/// Primary display size in the same coordinate space rdev reports for mouse moves.
///
/// macOS: CGDisplayBounds (logical pixels / points). Matches CGEventGetLocation.
/// Windows: GetSystemMetrics SM_CXSCREEN (physical pixels). Matches GetCursorPos.
/// Falls back to 1920x1080 when no display is available (headless / CI).
pub fn primary_display_size() -> (u32, u32) {
    platform::display_size()
}

mod platform {
    const FALLBACK_W: u32 = 1920;
    const FALLBACK_H: u32 = 1080;

    #[cfg(target_os = "macos")]
    pub fn warp(x: i32, y: i32) {
        unsafe {
            let pt = macos::CGPoint { x: x as f64, y: y as f64 };
            macos::CGWarpMouseCursorPosition(pt);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn warp(x: i32, y: i32) {
        unsafe {
            win::SetCursorPos(x, y);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub fn warp(_x: i32, _y: i32) {}

    #[cfg(target_os = "macos")]
    pub fn set_cursor_visible(visible: bool) {
        // kCGDirectMainDisplay = 0 is accepted by both Hide and Show.
        unsafe {
            if visible {
                macos::CGDisplayShowCursor(0);
            } else {
                macos::CGDisplayHideCursor(0);
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn set_cursor_visible(visible: bool) {
        unsafe {
            // ShowCursor increments/decrements an internal counter; a single
            // call per state change is sufficient for our use-case.
            win::ShowCursor(if visible { 1 } else { 0 });
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub fn set_cursor_visible(_visible: bool) {
        // Linux / other: no implementation yet. TODO: x11rb or XFixesHideCursor.
    }

    /// Returns logical display dimensions matching rdev's coordinate space.
    #[cfg(target_os = "macos")]
    pub fn display_size() -> (u32, u32) {
        unsafe {
            let display = macos::CGMainDisplayID();
            if display == 0 {
                return (FALLBACK_W, FALLBACK_H);
            }
            let bounds = macos::CGDisplayBounds(display);
            let w = bounds.size.width as u32;
            let h = bounds.size.height as u32;
            if w == 0 || h == 0 { (FALLBACK_W, FALLBACK_H) } else { (w, h) }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn display_size() -> (u32, u32) {
        unsafe {
            // SM_CXSCREEN=0, SM_CYSCREEN=1 — matches GetCursorPos pixel space.
            let w = win::GetSystemMetrics(0);
            let h = win::GetSystemMetrics(1);
            if w <= 0 || h <= 0 { (FALLBACK_W, FALLBACK_H) } else { (w as u32, h as u32) }
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub fn display_size() -> (u32, u32) {
        (FALLBACK_W, FALLBACK_H)
    }

    #[cfg(target_os = "macos")]
    mod macos {
        #[repr(C)]
        pub struct CGPoint {
            pub x: f64,
            pub y: f64,
        }

        #[repr(C)]
        pub struct CGSize {
            pub width: f64,
            pub height: f64,
        }

        #[repr(C)]
        pub struct CGRect {
            pub origin: CGPoint,
            pub size: CGSize,
        }

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            pub fn CGDisplayHideCursor(display: u32) -> i32;
            pub fn CGDisplayShowCursor(display: u32) -> i32;
            pub fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint) -> i32;
            pub fn CGMainDisplayID() -> u32;
            pub fn CGDisplayBounds(display: u32) -> CGRect;
        }
    }

    #[cfg(target_os = "windows")]
    mod win {
        extern "system" {
            pub fn ShowCursor(bShow: i32) -> i32;
            pub fn SetCursorPos(x: i32, y: i32) -> i32;
            pub fn GetSystemMetrics(nIndex: i32) -> i32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Live tests require a display. Marked #[ignore]; run manually with
    // `cargo test -p kvm-core cursor -- --ignored`.
    #[test]
    #[ignore]
    fn cursor_warp_and_restore() {
        let mut c = Cursor::new().unwrap();
        c.warp(100, 100).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        c.warp(500, 500).unwrap();
    }

    #[test]
    #[ignore]
    fn cursor_hide_show() {
        let mut c = Cursor::new().unwrap();
        c.set_visible(false);
        std::thread::sleep(std::time::Duration::from_millis(500));
        c.set_visible(true);
    }

    /// Display size is always positive (real display or safe fallback).
    #[test]
    fn primary_display_size_nonzero() {
        let (w, h) = primary_display_size();
        assert!(w > 0, "display width must be > 0");
        assert!(h > 0, "display height must be > 0");
    }

    /// Verify set_visible no-op and flip logic without a real display.
    #[test]
    fn cursor_state_tracking() {
        // Replicate the is_hidden flip logic from Cursor::set_visible.
        fn apply(is_hidden: &mut bool, visible: bool) -> bool {
            if visible == !*is_hidden { return false; } // no-op
            *is_hidden = !visible;
            true // changed
        }

        let mut hidden = false;
        // set_visible(true) when not hidden -> no-op
        assert!(!apply(&mut hidden, true));
        assert!(!hidden);
        // set_visible(false) -> flip
        assert!(apply(&mut hidden, false));
        assert!(hidden);
        // set_visible(false) again -> no-op
        assert!(!apply(&mut hidden, false));
        assert!(hidden);
        // set_visible(true) -> flip back
        assert!(apply(&mut hidden, true));
        assert!(!hidden);
    }
}
