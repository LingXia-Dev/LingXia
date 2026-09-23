/// Prod build currently talking to the dev service. Apple hosts draw the
/// pass-through chip from this. `1` means show it.
///
/// Kept out of `ffi.rs`: swift-bridge parses that file and rejects
/// `#[unsafe(no_mangle)]`.
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_dev_service_banner() -> i32 {
    i32::from(lingxia_app_context::dev_service_banner())
}
