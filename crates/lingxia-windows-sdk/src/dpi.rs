//! Shell chrome geometry is authored at 96 DPI; [`px`] maps it to the host
//! window's physical pixels. The process is PerMonitorV2-aware, so every
//! HWND coordinate is physical, and GDI text already takes its height from
//! the DC's DPI — only the layout side needs this factor. (Text painted
//! through a screen-compatible DC, as the layered popups do, sizes at the
//! system DPI, which only differs on mixed-DPI setups.)
//!
//! WebView2 keeps raw-pixel bounds but rasterizes at the monitor scale, so a
//! page's CSS px are logical px: any page-requested size (window content,
//! float extent) must be multiplied by `window_css_scale` before it becomes
//! a window size.

use std::cell::Cell;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::HWND;

/// Scale in thousandths of the most recent shell window to handle a message:
/// the fallback for layout computed outside any window message. 1000 until a
/// window reports its DPI, which is also what unit tests run at.
static SHELL_SCALE_MILLI: AtomicU32 = AtomicU32::new(1000);

thread_local! {
    /// Scale of the window whose message is being handled, if any.
    static MESSAGE_SCALE_MILLI: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Current chrome scale (`dpi / 96`).
pub(crate) fn chrome_scale() -> f64 {
    let milli = MESSAGE_SCALE_MILLI
        .with(Cell::get)
        .unwrap_or_else(|| SHELL_SCALE_MILLI.load(Ordering::Relaxed));
    f64::from(milli) / 1000.0
}

/// A 96-DPI length in the current host window's physical pixels.
pub(crate) fn px(value: i32) -> i32 {
    (f64::from(value) * chrome_scale()).round() as i32
}

/// `dpi / 96` for a window; 1.0 when the DPI cannot be read.
pub(crate) fn window_scale(hwnd: HWND) -> f64 {
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 }
}

/// Restores the scale of the enclosing message when dropped.
pub(crate) struct MessageScale(Option<u32>);

impl Drop for MessageScale {
    fn drop(&mut self) {
        MESSAGE_SCALE_MILLI.with(|scale| scale.set(self.0));
    }
}

/// Makes `root`'s DPI the chrome scale while one of its messages is handled.
/// A simulated device keeps 1.0 — its bezel geometry is authored in physical
/// pixels by design — and never becomes the fallback shell scale, so a
/// floating window beside it still lays out at its own monitor DPI.
pub(crate) fn enter_window_message(root: HWND) -> MessageScale {
    let previous = MESSAGE_SCALE_MILLI.with(Cell::get);
    #[cfg(feature = "device-frame")]
    let framed = crate::device_frame::window_has_device_frame(root.0 as isize);
    #[cfg(not(feature = "device-frame"))]
    let framed = false;
    let milli = if framed {
        1000
    } else {
        let milli = (window_scale(root) * 1000.0).round().max(1000.0) as u32;
        SHELL_SCALE_MILLI.store(milli, Ordering::Relaxed);
        milli
    };
    MESSAGE_SCALE_MILLI.with(|scale| scale.set(Some(milli)));
    MessageScale(previous)
}
