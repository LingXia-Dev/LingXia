/// Copy the touch-through dev mark as UTF-8, without a trailing NUL.
///
/// Kept out of `ffi.rs`: swift-bridge parses that file and rejects
/// `#[unsafe(no_mangle)]`. `compact` is non-zero for the phone tab. Returns
/// the byte length, or `-1` when the mark stays hidden. A null or short
/// buffer writes nothing.
#[unsafe(no_mangle)]
pub extern "C" fn lingxia_dev_mark(compact: i32, out: *mut u8, out_len: i32) -> i32 {
    let Some(label) = lingxia_app_context::dev_mark(compact != 0) else {
        return -1;
    };
    let bytes = label.as_bytes();
    let len = i32::try_from(bytes.len()).unwrap_or(i32::MAX);
    if out.is_null() || out_len < len {
        return len;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
    }
    len
}
