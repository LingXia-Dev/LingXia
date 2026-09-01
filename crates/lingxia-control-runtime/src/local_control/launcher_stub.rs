//! Target-native product launcher embedded by `build.rs`.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::process::Command;

const MAGIC: &[u8] = b"LXCL\x01\r\n";
const CLI_ARGUMENT: &str = "--cli";
const ENDPOINT: &str = "LINGXIA_CONTROL_ENDPOINT";

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("Error: product command launcher failed: {error}");
            10
        }
    };
    std::process::exit(code);
}

fn run() -> Result<i32, String> {
    let launcher =
        std::env::current_exe().map_err(|error| format!("cannot locate itself: {error}"))?;
    let contents = std::fs::read(launcher.with_extension("control"))
        .map_err(|error| format!("cannot read its product target: {error}"))?;
    let (target, endpoint) = decode_config(&contents)?;
    let status = Command::new(Path::new(&target))
        .args(std::env::args_os().skip(1))
        .arg(CLI_ARGUMENT)
        .env(ENDPOINT, endpoint)
        .status()
        .map_err(|error| format!("cannot start the product: {error}"))?;
    Ok(status.code().unwrap_or(10))
}

fn decode_config(contents: &[u8]) -> Result<(OsString, OsString), String> {
    if !contents.starts_with(MAGIC) {
        return Err("its target file has the wrong format".to_string());
    }
    let mut cursor = MAGIC.len();
    let target = decode_field(contents, &mut cursor)?;
    let endpoint = decode_field(contents, &mut cursor)?;
    if cursor != contents.len() {
        return Err("its target file has trailing data".to_string());
    }
    Ok((target, endpoint))
}

fn decode_field(contents: &[u8], cursor: &mut usize) -> Result<OsString, String> {
    let length = contents
        .get(*cursor..*cursor + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "its target file is truncated".to_string())? as usize;
    *cursor += 4;
    let byte_length = length
        .checked_mul(2)
        .ok_or_else(|| "its target file has an invalid length".to_string())?;
    let bytes = contents
        .get(*cursor..*cursor + byte_length)
        .ok_or_else(|| "its target file is truncated".to_string())?;
    let units = bytes
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
    *cursor += byte_length;
    Ok(OsString::from_wide(&units))
}
