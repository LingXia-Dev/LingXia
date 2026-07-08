//! Small CoreFoundation helpers for pulling typed values out of the untyped
//! `CFDictionary`s that `CGWindowListCopyWindowInfo` (and the AX API) hand back.
//! Everything here works on raw `*const c_void` so we can read heterogeneous
//! dictionaries without fighting the typed `CFDictionary<K, V>` generics.

use objc2_core_foundation::{CFDictionary, CFNumber, CFString, CGRect};
use std::ffi::c_void;

// Raw CoreFoundation entry points. Re-declared locally (they link to the same
// symbols the typed wrappers use) so we can pass bare pointers.
unsafe extern "C-unwind" {
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFNumberGetTypeID() -> usize;
}

/// Number of elements in a `CFArray` addressed by raw pointer.
pub(super) unsafe fn array_count(array: *const c_void) -> usize {
    if array.is_null() {
        return 0;
    }
    unsafe { CFArrayGetCount(array).max(0) as usize }
}

/// Element at `index` of a `CFArray` (borrowed; not retained).
pub(super) unsafe fn array_get(array: *const c_void, index: usize) -> *const c_void {
    unsafe { CFArrayGetValueAtIndex(array, index as isize) }
}

/// Look up a key (a `&CFString`) in a dictionary; returns a borrowed value.
pub(super) unsafe fn dict_get(dict: *const c_void, key: &CFString) -> *const c_void {
    if dict.is_null() {
        return std::ptr::null();
    }
    unsafe { CFDictionaryGetValue(dict, key as *const CFString as *const c_void) }
}

/// Read a dictionary value as a `String`, guarding the runtime type.
pub(super) unsafe fn dict_string(dict: *const c_void, key: &CFString) -> Option<String> {
    unsafe {
        let v = dict_get(dict, key);
        if v.is_null() || CFGetTypeID(v) != CFStringGetTypeID() {
            return None;
        }
        Some((*(v as *const CFString)).to_string())
    }
}

/// Read a dictionary value as an `i64`, guarding the runtime type.
pub(super) unsafe fn dict_i64(dict: *const c_void, key: &CFString) -> Option<i64> {
    unsafe {
        let v = dict_get(dict, key);
        if v.is_null() || CFGetTypeID(v) != CFNumberGetTypeID() {
            return None;
        }
        (*(v as *const CFNumber)).as_i64()
    }
}

/// Read a dictionary value as an `f64`, guarding the runtime type.
pub(super) unsafe fn dict_f64(dict: *const c_void, key: &CFString) -> Option<f64> {
    unsafe {
        let v = dict_get(dict, key);
        if v.is_null() || CFGetTypeID(v) != CFNumberGetTypeID() {
            return None;
        }
        (*(v as *const CFNumber)).as_f64()
    }
}

/// Read a `kCGWindowBounds`-style nested dictionary as a `CGRect`.
pub(super) unsafe fn dict_rect(dict: *const c_void, key: &CFString) -> Option<CGRect> {
    unsafe {
        let v = dict_get(dict, key);
        if v.is_null() {
            return None;
        }
        let mut rect = CGRect::default();
        let ok = objc2_core_graphics::CGRectMakeWithDictionaryRepresentation(
            Some(&*(v as *const CFDictionary)),
            &mut rect,
        );
        ok.then_some(rect)
    }
}
