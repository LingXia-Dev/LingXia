//! Shell chrome geometry is authored at 96 DPI; [`px`] maps it to the host
//! window's physical pixels. The process is PerMonitorV2-aware, so every
//! HWND coordinate is physical, and GDI text already takes its height from
//! the DC's DPI — only the layout side needs this factor.
//!
//! WebView2 keeps raw-pixel bounds but rasterizes at the monitor scale, so a
//! page's CSS px are logical px: any page-requested size (window content,
//! float extent) must be multiplied by `window_css_scale` before it becomes
//! a window size.

use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Foundation::HWND;

/// Scale in thousandths; 1000 until a host window reports its DPI, which is
/// also what unit tests run at.
static SCALE_MILLI: AtomicU32 = AtomicU32::new(1000);

/// Current chrome scale (`dpi / 96`).
pub(crate) fn chrome_scale() -> f64 {
    f64::from(SCALE_MILLI.load(Ordering::Relaxed)) / 1000.0
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

/// Adopts `hwnd`'s DPI as the chrome scale. A simulated device keeps 1.0: its
/// bezel geometry is authored in physical pixels by design.
pub(crate) fn sync_chrome_scale(hwnd: HWND) {
    #[cfg(feature = "device-frame")]
    if crate::device_frame::window_has_device_frame(hwnd.0 as isize) {
        SCALE_MILLI.store(1000, Ordering::Relaxed);
        return;
    }
    let milli = (window_scale(hwnd) * 1000.0).round().max(1000.0) as u32;
    SCALE_MILLI.store(milli, Ordering::Relaxed);
}
