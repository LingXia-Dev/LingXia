//! Anisette from this Mac's AOSKit.
//!
//! `retrieveOTPHeadersForDSID:` returns `X-Apple-MD` and `X-Apple-MD-M` for
//! DSID `-2`. GrandSlam wants those as `X-Apple-I-MD` and `X-Apple-I-MD-M`,
//! plus routing info `17106176` and this machine's own client identity.

use super::anisette::AnisetteData;
use super::grandslam::GRANDSLAM_USER_AGENT;
use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::process::Command;

const RINFO: u64 = 17_106_176;
const AOSKIT: &str = "/System/Library/PrivateFrameworks/AOSKit.framework/AOSKit";

pub(super) fn local_anisette() -> Result<AnisetteData> {
    let headers = otp_headers()?;
    let otp = header_value(&headers, &["X-Apple-I-MD", "X-Apple-MD"])
        .context("AOSKit omitted the one-time password")?;
    let machine_id = header_value(&headers, &["X-Apple-I-MD-M", "X-Apple-MD-M"])
        .context("AOSKit omitted the machine id")?;
    let routing_info = header_value(&headers, &["X-Apple-I-MD-RINFO", "X-Apple-MD-RINFO"])
        .and_then(|value| value.parse().ok())
        .unwrap_or(RINFO);

    let (device_id, serial_number) = platform_identity()?;
    let model = trimmed_stdout("sysctl", &["-n", "hw.model"])?;
    let version = trimmed_stdout("sw_vers", &["-productVersion"])?;
    let build = trimmed_stdout("sw_vers", &["-buildVersion"])?;

    Ok(AnisetteData {
        machine_id,
        otp,
        routing_info,
        local_user_id: local_user_id()?,
        device_id,
        client_info: client_info(&model, &version, &build),
        user_agent: GRANDSLAM_USER_AGENT.to_string(),
        serial_number,
    })
}

fn header_value(headers: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        headers
            .get(*key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn client_info(model: &str, version: &str, build: &str) -> String {
    format!("<{model}> <macOS;{version};{build}> <com.apple.AuthKit/1 (com.apple.akd/1.0)>")
}

fn platform_identity() -> Result<(String, String)> {
    let text = command_stdout("ioreg", &["-rd1", "-c", "IOPlatformExpertDevice"])?;
    let device_id = assignment_value(&text, "IOPlatformUUID")
        .context("missing platform UUID")?
        .to_uppercase();
    let serial = assignment_value(&text, "IOPlatformSerialNumber").context("missing serial")?;
    Ok((device_id, serial))
}

fn local_user_id() -> Result<String> {
    let user = std::env::var("USER").context("USER is unset")?;
    let text = command_stdout(
        "dscl",
        &[".", "-read", &format!("/Users/{user}"), "GeneratedUID"],
    )?;
    generated_uid(&text).context("missing GeneratedUID")
}

fn trimmed_stdout(program: &str, args: &[&str]) -> Result<String> {
    let text = command_stdout(program, args)?;
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return Err(anyhow!("{program} returned nothing"));
    }
    Ok(line.to_string())
}

fn command_stdout(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {program}"))?;
    if !output.status.success() {
        return Err(anyhow!("{program} failed"));
    }
    String::from_utf8(output.stdout).context("command output was not utf-8")
}

fn assignment_value(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\" = \"");
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix(&needle) else {
            continue;
        };
        let value = rest.strip_suffix('"')?.trim();
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

fn generated_uid(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("GeneratedUID:") else {
            continue;
        };
        let value = rest.trim();
        if value.is_empty() {
            return None;
        }
        return Some(value.to_string());
    }
    None
}

fn otp_headers() -> Result<HashMap<String, String>> {
    let headers = unsafe { read_otp_headers() }?;
    if headers.is_empty() {
        return Err(anyhow!("AOSKit returned no headers"));
    }
    Ok(headers)
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const libc::c_char) -> *mut libc::c_void;
    fn sel_registerName(name: *const libc::c_char) -> *mut libc::c_void;
    fn objc_msgSend();
}

struct AutoreleasePool(*mut libc::c_void);

impl Drop for AutoreleasePool {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            msg0(self.0, sel(c"drain"));
        }
    }
}

unsafe fn read_otp_headers() -> Result<HashMap<String, String>> {
    let path = CString::new(AOSKIT).context("invalid AOSKit path")?;
    if unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_LAZY) }.is_null() {
        return Err(anyhow!("AOSKit is unavailable"));
    }

    let utilities = class(c"AOSUtilities").context("AOSUtilities is unavailable")?;
    let nsstring = class(c"NSString").context("NSString is unavailable")?;
    let _pool = AutoreleasePool(autorelease_pool()?);

    let dsid = unsafe { msg_cstr_arg(nsstring, sel(c"stringWithUTF8String:"), c"-2".as_ptr()) };
    if dsid.is_null() {
        return Err(anyhow!("failed to build the AOSKit DSID"));
    }
    let dict = unsafe { msg1(utilities, sel(c"retrieveOTPHeadersForDSID:"), dsid) };
    if dict.is_null() {
        return Err(anyhow!("AOSKit returned no headers"));
    }
    let keys = unsafe { msg0(dict, sel(c"allKeys")) };
    if keys.is_null() {
        return Err(anyhow!("AOSKit returned no headers"));
    }
    let count = unsafe { msg0_usize(keys, sel(c"count")) }.min(32);

    let mut headers = HashMap::new();
    for index in 0..count {
        let key = unsafe { msg1_usize(keys, sel(c"objectAtIndex:"), index) };
        let value = unsafe { msg1(dict, sel(c"objectForKey:"), key) };
        let Some(key) = (unsafe { ns_description(key) }) else {
            continue;
        };
        let Some(value) = (unsafe { ns_description(value) }) else {
            continue;
        };
        headers.insert(key, value);
    }
    Ok(headers)
}

fn class(name: &CStr) -> Option<*mut libc::c_void> {
    let class = unsafe { objc_getClass(name.as_ptr()) };
    if class.is_null() { None } else { Some(class) }
}

fn sel(name: &CStr) -> *mut libc::c_void {
    unsafe { sel_registerName(name.as_ptr()) }
}

fn autorelease_pool() -> Result<*mut libc::c_void> {
    let pool_class = class(c"NSAutoreleasePool").context("NSAutoreleasePool is unavailable")?;
    let allocated = unsafe { msg0(pool_class, sel(c"alloc")) };
    if allocated.is_null() {
        return Err(anyhow!("failed to allocate an autorelease pool"));
    }
    let pool = unsafe { msg0(allocated, sel(c"init")) };
    if pool.is_null() {
        return Err(anyhow!("failed to allocate an autorelease pool"));
    }
    Ok(pool)
}

unsafe fn ns_description(obj: *mut libc::c_void) -> Option<String> {
    if obj.is_null() {
        return None;
    }
    let desc = unsafe { msg0(obj, sel(c"description")) };
    if desc.is_null() {
        return None;
    }
    let ptr = unsafe { msg_cstr(desc, sel(c"UTF8String")) };
    if ptr.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

unsafe fn msg0(recv: *mut libc::c_void, sel: *mut libc::c_void) -> *mut libc::c_void {
    let call: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> *mut libc::c_void =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { call(recv, sel) }
}

unsafe fn msg1(
    recv: *mut libc::c_void,
    sel: *mut libc::c_void,
    arg: *mut libc::c_void,
) -> *mut libc::c_void {
    let call: unsafe extern "C" fn(
        *mut libc::c_void,
        *mut libc::c_void,
        *mut libc::c_void,
    ) -> *mut libc::c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { call(recv, sel, arg) }
}

unsafe fn msg1_usize(
    recv: *mut libc::c_void,
    sel: *mut libc::c_void,
    arg: usize,
) -> *mut libc::c_void {
    let call: unsafe extern "C" fn(
        *mut libc::c_void,
        *mut libc::c_void,
        usize,
    ) -> *mut libc::c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { call(recv, sel, arg) }
}

unsafe fn msg0_usize(recv: *mut libc::c_void, sel: *mut libc::c_void) -> usize {
    let call: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> usize =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { call(recv, sel) }
}

unsafe fn msg_cstr_arg(
    recv: *mut libc::c_void,
    sel: *mut libc::c_void,
    arg: *const libc::c_char,
) -> *mut libc::c_void {
    let call: unsafe extern "C" fn(
        *mut libc::c_void,
        *mut libc::c_void,
        *const libc::c_char,
    ) -> *mut libc::c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { call(recv, sel, arg) }
}

unsafe fn msg_cstr(recv: *mut libc::c_void, sel: *mut libc::c_void) -> *const libc::c_char {
    let call: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> *const libc::c_char =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { call(recv, sel) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_info_names_this_machine() {
        assert_eq!(
            client_info("MacBookPro16,2", "15.6", "24G84"),
            "<MacBookPro16,2> <macOS;15.6;24G84> <com.apple.AuthKit/1 (com.apple.akd/1.0)>"
        );
    }

    #[test]
    fn aoskit_header_names_map_onto_grandslam() {
        let headers = HashMap::from([
            ("X-Apple-MD".to_string(), "otp".to_string()),
            ("X-Apple-MD-M".to_string(), "machine".to_string()),
        ]);
        assert_eq!(
            header_value(&headers, &["X-Apple-I-MD", "X-Apple-MD"]).as_deref(),
            Some("otp")
        );
        assert_eq!(
            header_value(&headers, &["X-Apple-I-MD-M", "X-Apple-MD-M"]).as_deref(),
            Some("machine")
        );
        assert!(header_value(&headers, &["X-Apple-I-MD-RINFO", "X-Apple-MD-RINFO"]).is_none());
    }

    #[test]
    fn platform_text_parses_without_keeping_the_raw_dump() {
        let text = "\"IOPlatformUUID\" = \"AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE\"\n\
                    \"IOPlatformSerialNumber\" = \"C02TESTSERIAL\"\n";
        assert_eq!(
            assignment_value(text, "IOPlatformUUID").as_deref(),
            Some("AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE")
        );
        assert_eq!(
            assignment_value(text, "IOPlatformSerialNumber").as_deref(),
            Some("C02TESTSERIAL")
        );
        assert_eq!(
            generated_uid("GeneratedUID: FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF\n").as_deref(),
            Some("FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF")
        );
    }

    #[test]
    fn local_anisette_uses_this_mac() {
        let data = local_anisette().expect("local anisette");
        let model = trimmed_stdout("sysctl", &["-n", "hw.model"]).expect("hw.model");
        assert!(data.client_info.starts_with(&format!("<{model}>")));
        assert!(data.client_info.contains("com.apple.akd/1.0"));
        assert_eq!(data.user_agent, GRANDSLAM_USER_AGENT);
        let complete = data.otp.len() > 8
            && data.machine_id.len() > 8
            && data.routing_info == RINFO
            && data.device_id.len() == 36
            && !data.serial_number.is_empty()
            && !data.local_user_id.is_empty();
        assert!(complete, "local anisette identity was incomplete");
    }
}
