//! Win11 theme detection (light/dark + system accent) for the shell palette.
//!
//! The values are read from the registry and cached. [`refresh`] re-reads them
//! so a live theme/accent change (`WM_SETTINGCHANGE` /
//! `WM_DWMCOLORIZATIONCOLORCHANGED`) repaints the shell in the new theme
//! without a restart. All accessors lazily initialize on first use, so a paint
//! that happens before any explicit refresh still sees the real system theme.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::{PCWSTR, w};

/// Fallback accent (the legacy hardcoded shell blue) used when the system value
/// is unavailable.
pub(super) const DEFAULT_ACCENT: u32 = 0x1677ff;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static IS_DARK: AtomicBool = AtomicBool::new(false);
static ACCENT_RGB: AtomicU32 = AtomicU32::new(DEFAULT_ACCENT);

/// Re-read the system theme into the cache. Safe to call from the UI thread on
/// a settings/colorization change. Returns `true` when the cached values
/// actually changed (or were uninitialized), so callers only repaint on a real
/// theme change rather than on every unrelated `WM_SETTINGCHANGE` broadcast.
pub(super) fn refresh() -> bool {
    let dark = read_dword(
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        w!("AppsUseLightTheme"),
    )
    .map(|apps_use_light| apps_use_light == 0)
    .unwrap_or(false);
    let accent = read_dword(w!("Software\\Microsoft\\Windows\\DWM"), w!("AccentColor"))
        .map(accent_abgr_to_rgb)
        .unwrap_or(DEFAULT_ACCENT);
    let prev_dark = IS_DARK.swap(dark, Ordering::Relaxed);
    let prev_accent = ACCENT_RGB.swap(accent, Ordering::Relaxed);
    let was_initialized = INITIALIZED.swap(true, Ordering::Relaxed);
    !was_initialized || prev_dark != dark || prev_accent != accent
}

fn ensure_initialized() {
    if !INITIALIZED.load(Ordering::Relaxed) {
        refresh();
    }
}

/// Whether Win11 apps are currently in dark mode.
pub(super) fn is_dark() -> bool {
    ensure_initialized();
    IS_DARK.load(Ordering::Relaxed)
}

/// The system accent color as `0xRRGGBB` (the format `rgb_to_colorref` expects).
pub(super) fn system_accent() -> u32 {
    ensure_initialized();
    ACCENT_RGB.load(Ordering::Relaxed)
}

/// `DWM\AccentColor` stores the accent little-endian as `0xAABBGGRR` (low byte
/// is red); repack to the shell's `0xRRGGBB`.
fn accent_abgr_to_rgb(value: u32) -> u32 {
    let r = value & 0xff;
    let g = (value >> 8) & 0xff;
    let b = (value >> 16) & 0xff;
    (r << 16) | (g << 8) | b
}

fn read_dword(subkey: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(&mut data as *mut u32 as *mut c_void),
            Some(&mut size),
        )
    };
    (status == ERROR_SUCCESS).then_some(data)
}
