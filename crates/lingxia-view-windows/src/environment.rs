//! What the user set in Windows that a page's environment carries.
use windows::Win32::Globalization::{
    GetLocaleInfoEx, GetUserDefaultLocaleName, LOCALE_IFIRSTDAYOFWEEK, LOCALE_STIMEFORMAT,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SystemParametersInfoW,
};
use windows::core::{PCWSTR, w};

const LOCALE_NAME_MAX: usize = 85;

/// The user's BCP 47 locale.
pub(crate) fn locale() -> String {
    let mut name = [0u16; LOCALE_NAME_MAX];
    // SAFETY: a buffer of the documented maximum.
    let len = unsafe { GetUserDefaultLocaleName(&mut name) };
    if len <= 1 {
        return "en-US".into();
    }
    String::from_utf16_lossy(&name[..len as usize - 1])
}

fn locale_info(kind: u32) -> Option<String> {
    let mut buffer = [0u16; 128];
    // SAFETY: the user's locale (null name) into a stack buffer.
    let len = unsafe { GetLocaleInfoEx(PCWSTR::null(), kind, Some(&mut buffer)) };
    (len > 1).then(|| String::from_utf16_lossy(&buffer[..len as usize - 1]))
}

/// `h23` when the user's time format is 24-hour (`H`), else `h12`.
pub(crate) fn hour_cycle() -> String {
    match locale_info(LOCALE_STIMEFORMAT) {
        Some(format) if format.contains('H') => "h23".into(),
        _ => "h12".into(),
    }
}

/// 1 = Sunday; Windows counts 0 = Monday.
pub(crate) fn first_weekday() -> u8 {
    locale_info(LOCALE_IFIRSTDAYOFWEEK)
        .and_then(|day| day.trim().parse::<u8>().ok())
        .map_or(1, |monday_based| (monday_based + 1) % 7 + 1)
}

/// "Show animations in Windows" is off.
pub(crate) fn reduce_motion() -> bool {
    let mut animate = windows::core::BOOL(1);
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut animate as *mut windows::core::BOOL).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && !animate.as_bool()
}

pub(crate) fn high_contrast() -> bool {
    let mut contrast = HIGHCONTRASTW {
        cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    // SAFETY: SPI_GETHIGHCONTRAST fills the sized struct.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && contrast.dwFlags.contains(HCF_HIGHCONTRASTON)
}

/// Apps follow the dark theme ("AppsUseLightTheme" is 0).
pub(crate) fn system_dark() -> bool {
    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: a DWORD into a sized stack value.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    status.is_ok() && value == 0
}
