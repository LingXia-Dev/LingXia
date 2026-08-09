//! Windows terminal font discovery.

use lingxia_terminal_config::InstalledFont;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DEFAULT_CHARSET, DeleteObject, EnumFontFamiliesExW, GLYPHSET, GetDC,
    GetFontUnicodeRanges, HDC, LOGFONTW, ReleaseDC, SelectObject, TEXTMETRICW, WCRANGE,
};

pub(crate) fn installed_fonts() -> Vec<InstalledFont> {
    let hdc = unsafe { GetDC(None) };
    if hdc.is_invalid() {
        return Vec::new();
    }
    let mut families = Vec::new();
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
    let installed = families
        .into_iter()
        .map(|family| InstalledFont {
            nerd_icons: has_powerline_glyphs(hdc, &family),
            monospace: true,
            // GDI does not shape text, so this remains conservative until the
            // DirectWrite path exposes an equivalent feature probe.
            ligatures: false,
            family,
        })
        .collect();
    unsafe { ReleaseDC(None, hdc) };
    installed
}

unsafe extern "system" fn collect_family(
    logfont: *const LOGFONTW,
    metric: *const TEXTMETRICW,
    _font_type: u32,
    lparam: LPARAM,
) -> i32 {
    let families = unsafe { &mut *(lparam.0 as *mut Vec<String>) };
    let logfont = unsafe { &*logfont };
    // TMPF_FIXED_PITCH is inverted for historical compatibility: clear means
    // fixed pitch and therefore safe for a terminal grid.
    let fixed_pitch = unsafe { (*metric).tmPitchAndFamily }.0 & 0x01 == 0;
    let name = String::from_utf16_lossy(&logfont.lfFaceName)
        .trim_end_matches('\0')
        .to_string();
    if fixed_pitch && !name.is_empty() && !name.starts_with('@') {
        families.push(name);
    }
    1
}

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
                let ranges = std::ptr::addr_of!((*glyphs).ranges) as *const WCRANGE;
                for index in 0..(*glyphs).cRanges as usize {
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
