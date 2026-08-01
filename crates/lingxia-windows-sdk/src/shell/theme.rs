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
use windows::Win32::Graphics::Gdi::{
    COLOR_3DFACE, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_WINDOW, COLOR_WINDOWTEXT, GetSysColor,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{PCWSTR, w};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static IS_DARK: AtomicBool = AtomicBool::new(false);
static IS_HIGH_CONTRAST: AtomicBool = AtomicBool::new(false);
static ACCENT_RGB: AtomicU32 = AtomicU32::new(0);
static SYSTEM_WINDOW_RGB: AtomicU32 = AtomicU32::new(0xffffff);
static SYSTEM_WINDOW_TEXT_RGB: AtomicU32 = AtomicU32::new(0x000000);
static SYSTEM_GRAY_TEXT_RGB: AtomicU32 = AtomicU32::new(0x6d6d6d);
static SYSTEM_HIGHLIGHT_RGB: AtomicU32 = AtomicU32::new(0x0078d4);
static SYSTEM_CONTROL_RGB: AtomicU32 = AtomicU32::new(0xf0f0f0);

#[derive(Clone, Copy)]
pub(super) struct SystemColors {
    pub window: u32,
    pub window_text: u32,
    pub gray_text: u32,
    pub highlight: u32,
    pub control: u32,
}

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
    let high_contrast = read_high_contrast();
    lingxia_platform::windows::set_windows_host_appearance_dark(dark);
    let colors = read_system_colors();
    let accent = read_dword(w!("Software\\Microsoft\\Windows\\DWM"), w!("AccentColor"))
        .map(accent_abgr_to_rgb)
        .unwrap_or(colors.highlight);
    let prev_dark = IS_DARK.swap(dark, Ordering::Relaxed);
    let prev_accent = ACCENT_RGB.swap(accent, Ordering::Relaxed);
    let prev_high_contrast = IS_HIGH_CONTRAST.swap(high_contrast, Ordering::Relaxed);
    let window_changed = SYSTEM_WINDOW_RGB.swap(colors.window, Ordering::Relaxed) != colors.window;
    let window_text_changed =
        SYSTEM_WINDOW_TEXT_RGB.swap(colors.window_text, Ordering::Relaxed) != colors.window_text;
    let gray_text_changed =
        SYSTEM_GRAY_TEXT_RGB.swap(colors.gray_text, Ordering::Relaxed) != colors.gray_text;
    let highlight_changed =
        SYSTEM_HIGHLIGHT_RGB.swap(colors.highlight, Ordering::Relaxed) != colors.highlight;
    let control_changed =
        SYSTEM_CONTROL_RGB.swap(colors.control, Ordering::Relaxed) != colors.control;
    let colors_changed = window_changed
        || window_text_changed
        || gray_text_changed
        || highlight_changed
        || control_changed;
    let was_initialized = INITIALIZED.swap(true, Ordering::Relaxed);
    if was_initialized && prev_dark != dark {
        lxapp::refresh_auto_appearances();
    }
    !was_initialized
        || prev_dark != dark
        || prev_accent != accent
        || prev_high_contrast != high_contrast
        || colors_changed
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

pub(super) fn is_high_contrast() -> bool {
    ensure_initialized();
    IS_HIGH_CONTRAST.load(Ordering::Relaxed)
}

pub(super) fn system_colors() -> SystemColors {
    ensure_initialized();
    SystemColors {
        window: SYSTEM_WINDOW_RGB.load(Ordering::Relaxed),
        window_text: SYSTEM_WINDOW_TEXT_RGB.load(Ordering::Relaxed),
        gray_text: SYSTEM_GRAY_TEXT_RGB.load(Ordering::Relaxed),
        highlight: SYSTEM_HIGHLIGHT_RGB.load(Ordering::Relaxed),
        control: SYSTEM_CONTROL_RGB.load(Ordering::Relaxed),
    }
}

/// `DWM\AccentColor` stores the accent little-endian as `0xAABBGGRR` (low byte
/// is red); repack to the shell's `0xRRGGBB`.
fn accent_abgr_to_rgb(value: u32) -> u32 {
    let r = value & 0xff;
    let g = (value >> 8) & 0xff;
    let b = (value >> 16) & 0xff;
    (r << 16) | (g << 8) | b
}

fn read_high_contrast() -> bool {
    let mut high_contrast = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            high_contrast.cbSize,
            Some(&mut high_contrast as *mut _ as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
            && high_contrast.dwFlags.contains(HCF_HIGHCONTRASTON)
    }
}

fn read_system_colors() -> SystemColors {
    unsafe {
        SystemColors {
            window: colorref_to_rgb(GetSysColor(COLOR_WINDOW)),
            window_text: colorref_to_rgb(GetSysColor(COLOR_WINDOWTEXT)),
            gray_text: colorref_to_rgb(GetSysColor(COLOR_GRAYTEXT)),
            highlight: colorref_to_rgb(GetSysColor(COLOR_HIGHLIGHT)),
            control: colorref_to_rgb(GetSysColor(COLOR_3DFACE)),
        }
    }
}

fn colorref_to_rgb(value: u32) -> u32 {
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

#[cfg(test)]
mod tests {
    use super::{accent_abgr_to_rgb, colorref_to_rgb};

    #[test]
    fn native_color_encodings_are_normalized_to_rgb() {
        assert_eq!(accent_abgr_to_rgb(0xff33_2211), 0x112233);
        assert_eq!(colorref_to_rgb(0x0033_2211), 0x112233);
    }
}
