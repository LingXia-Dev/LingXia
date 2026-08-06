//! The product's `term` command line on Windows.
//!
//! The Apple SDK's counterpart is `Lingxia.runTerminalCommandIfInvoked()`.
//! Both run before the host opens a window and before the runtime initializes:
//! initialization opens the app's databases, and a configuration command must
//! not collide with an instance already running.

use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_terminal_config::InstalledFont;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DEFAULT_CHARSET, DeleteObject, EnumFontFamiliesExW, GLYPHSET, GetDC,
    GetFontUnicodeRanges, HDC, LOGFONTW, ReleaseDC, SelectObject, TEXTMETRICW, WCRANGE,
};

/// Run the `term` command line and exit, when this process was invoked as one.
///
/// Call this as the first statement of `main`. It returns normally when the
/// process should carry on and become the app.
pub fn run_if_invoked() {
    let Ok(platform) = lingxia::windows::Platform::from_env() else {
        return;
    };
    // Enumerating families is GDI, not the runtime, so the command line can
    // report what is installed without becoming an app.
    lingxia::terminal::set_installed_fonts(installed_fonts());
    if let Some(code) =
        lingxia::terminal::run_if_invoked(&platform.app_data_dir(), system_prefers_dark())
    {
        std::process::exit(code);
    }
}

/// Whether Windows currently has apps in dark mode.
///
/// Read straight from the registry rather than through the shell's theme
/// tracker: the appearance decides which of the two configured schemes an
/// unqualified theme change writes, and reading it must not pull in window
/// chrome that a terminal-only host does not build.
fn system_prefers_dark() -> bool {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_DWORD, RegCloseKey, RegGetValueW,
        RegOpenKeyExW,
    };
    use windows::core::w;

    let mut key = HKEY::default();
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            Some(0),
            KEY_READ,
            &mut key,
        )
    };
    if opened.is_err() {
        return false;
    }
    let mut value: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    let read = unsafe {
        RegGetValueW(
            key,
            None,
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut std::ffi::c_void),
            Some(&mut size),
        )
    };
    let _ = unsafe { RegCloseKey(key) };
    read.is_ok() && value == 0
}

/// The installed monospace families, as `term font list` reports them.
fn installed_fonts() -> Vec<InstalledFont> {
    let hdc = unsafe { GetDC(None) };
    if hdc.is_invalid() {
        return Vec::new();
    }
    let mut families: Vec<String> = Vec::new();
    let request = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    unsafe {
        EnumFontFamiliesExW(
            hdc,
            &request,
            Some(collect_family),
            LPARAM(&mut families as *mut Vec<String> as isize),
            0,
        );
    }
    families.sort();
    families.dedup();
    let fonts = families
        .into_iter()
        .map(|family| InstalledFont {
            nerd_icons: has_powerline_glyphs(hdc, &family),
            monospace: true,
            // GDI does no shaping, so no ligature would be drawn even by a font
            // that has them. This becomes a real answer with the GPU renderer.
            ligatures: false,
            family,
        })
        .collect();
    unsafe { ReleaseDC(None, hdc) };
    fonts
}

/// Enumeration callback: keep the fixed-pitch, non-vertical families.
unsafe extern "system" fn collect_family(
    logfont: *const LOGFONTW,
    metric: *const TEXTMETRICW,
    _font_type: u32,
    lparam: LPARAM,
) -> i32 {
    let families = unsafe { &mut *(lparam.0 as *mut Vec<String>) };
    let logfont = unsafe { &*logfont };
    // `TMPF_FIXED_PITCH` is set for *variable* pitch — the flag's name has been
    // inverted since Windows 3.x. Clear means the grid is safe.
    let fixed_pitch = unsafe { (*metric).tmPitchAndFamily }.0 & 0x01 == 0;
    let name = String::from_utf16_lossy(&logfont.lfFaceName)
        .trim_end_matches('\0')
        .to_string();
    // A leading `@` marks the vertical-writing variant of a family, which is
    // the same typeface rotated and never what someone picks for a terminal.
    if fixed_pitch && !name.is_empty() && !name.starts_with('@') {
        families.push(name);
    }
    1
}

/// Whether the family carries the powerline range (U+E0B0…), which is what
/// makes a Nerd Font worth choosing for a prompt.
fn has_powerline_glyphs(hdc: HDC, family: &str) -> bool {
    let mut logfont = LOGFONTW {
        lfCharSet: DEFAULT_CHARSET,
        ..Default::default()
    };
    for (slot, unit) in logfont
        .lfFaceName
        .iter_mut()
        .zip(family.encode_utf16().chain(std::iter::once(0)))
    {
        *slot = unit;
    }
    let font = unsafe { CreateFontIndirectW(&logfont) };
    if font.is_invalid() {
        return false;
    }
    let previous = unsafe { SelectObject(hdc, font.into()) };
    let bytes = unsafe { GetFontUnicodeRanges(hdc, None) };
    let mut found = false;
    if bytes > 0 {
        let mut buffer = vec![0u8; bytes as usize];
        let glyphs = buffer.as_mut_ptr() as *mut GLYPHSET;
        unsafe {
            (*glyphs).cbThis = bytes;
            if GetFontUnicodeRanges(hdc, Some(glyphs)) > 0 {
                let count = (*glyphs).cRanges as usize;
                let ranges = std::ptr::addr_of!((*glyphs).ranges) as *const WCRANGE;
                for index in 0..count {
                    let range = &*ranges.add(index);
                    let start = range.wcLow as u32;
                    let end = start + range.cGlyphs as u32;
                    if start <= 0xE0B0 && 0xE0B0 < end {
                        found = true;
                        break;
                    }
                }
            }
        }
    }
    unsafe {
        SelectObject(hdc, previous);
        let _ = DeleteObject(font.into());
    }
    found
}
