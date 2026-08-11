//! Minimal macOS Accessibility authorization checks shared by diagnostics and
//! AX-backed desktop capabilities.

#[cfg(feature = "diagnostics")]
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};
#[cfg(feature = "diagnostics")]
use std::ffi::c_void;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    #[cfg(feature = "diagnostics")]
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

pub(super) fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Ask macOS to surface the Accessibility consent UI. The return value is the
/// current grant; user approval can still require a process restart.
#[cfg(feature = "diagnostics")]
pub(super) fn prompt_trusted() -> bool {
    let key = CFString::from_str("AXTrustedCheckOptionPrompt");
    let value = CFBoolean::new(true);
    let options = CFDictionary::from_slices(&[&*key], &[value]);
    unsafe { AXIsProcessTrustedWithOptions((&*options as *const CFDictionary<CFString, _>).cast()) }
}
