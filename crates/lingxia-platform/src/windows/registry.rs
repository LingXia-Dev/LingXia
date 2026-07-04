//! Shared readers over `RegGetValueW` (size probe, then data).

use windows::Win32::System::Registry::{HKEY, REG_ROUTINE_FLAGS, RRF_RT_REG_SZ, RegGetValueW};
use windows::core::HSTRING;

pub(super) fn read_value(
    root: HKEY,
    subkey: &str,
    value: &str,
    flags: REG_ROUTINE_FLAGS,
) -> Option<Vec<u8>> {
    let subkey = HSTRING::from(subkey);
    let value = HSTRING::from(value);
    let mut size = 0u32;
    let status = unsafe { RegGetValueW(root, &subkey, &value, flags, None, None, Some(&mut size)) };
    if !status.is_ok() || size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    let status = unsafe {
        RegGetValueW(
            root,
            &subkey,
            &value,
            flags,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if !status.is_ok() {
        return None;
    }
    data.truncate(size as usize);
    Some(data)
}

/// A `REG_SZ` value as a Rust string, NUL-terminated tail dropped. Whitespace
/// is preserved — trim at the call site when it matters.
pub(super) fn read_string(root: HKEY, subkey: &str, value: &str) -> Option<String> {
    let data = read_value(root, subkey, value, RRF_RT_REG_SZ)?;
    if data.len() < 2 {
        return None;
    }
    let units: Vec<u16> = data
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&unit| unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}
