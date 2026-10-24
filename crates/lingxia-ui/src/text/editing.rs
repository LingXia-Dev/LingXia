//! Displayed-text offset mapping and caret reveal for editors.

pub(crate) fn reveal_caret(previous: f32, start: f32, end: f32, viewport: f32, extent: f32) -> f32 {
    let max = (extent.max(end) - viewport).max(0.0);
    let mut offset = previous.clamp(0.0, max);
    if end > offset + viewport {
        offset = end - viewport;
    }
    if start < offset {
        offset = start;
    }
    offset.clamp(0.0, max)
}

pub(crate) fn display_byte(
    value: &str,
    offset: u32,
    secure: bool,
    single_line: bool,
) -> Option<usize> {
    let mut units = 0;
    let mut bytes = 0;
    for ch in value.chars() {
        if units == offset {
            return Some(bytes);
        }
        units += ch.len_utf16() as u32;
        if units > offset {
            return None;
        }
        bytes += if secure {
            '\u{2022}'.len_utf8()
        } else if single_line && matches!(ch, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
            1
        } else {
            ch.len_utf8()
        };
    }
    (units == offset).then_some(bytes)
}

pub(crate) fn display_offset(
    value: &str,
    byte: usize,
    secure: bool,
    single_line: bool,
) -> Option<u32> {
    let mut units = 0;
    let mut bytes = 0;
    for ch in value.chars() {
        if bytes == byte {
            return Some(units);
        }
        bytes += if secure {
            '\u{2022}'.len_utf8()
        } else if single_line && matches!(ch, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
            1
        } else {
            ch.len_utf8()
        };
        if bytes > byte {
            return None;
        }
        units += ch.len_utf16() as u32;
    }
    (bytes == byte).then_some(units)
}
