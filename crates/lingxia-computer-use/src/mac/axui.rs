//! Thin, owning wrappers over the macOS Accessibility (`AXUIElement`) C API,
//! which has no objc2 binding. `AxEl` owns a retained `AXUIElementRef` and
//! releases it on drop; `Cf` does the same for any other CoreFoundation value
//! we copy out of an attribute. Attribute/action names are the stable AX
//! constant strings (e.g. "AXRole"), created as CFStrings on demand.

use crate::error::{Error, Result};
use objc2_core_foundation::{CFRetained, CFString, CGPoint, CGSize};
use std::ffi::c_void;

// AXValueType raw values (CoreGraphics/AXValue.h).
const AX_VALUE_CGPOINT: u32 = 1;
const AX_VALUE_CGSIZE: u32 = 2;

// AXError values (HIServices/AXError.h).
const AX_SUCCESS: i32 = 0;
const AX_ERROR_API_DISABLED: i32 = -25211;
const AX_ERROR_ATTRIBUTE_UNSUPPORTED: i32 = -25205;
const AX_ERROR_ACTION_UNSUPPORTED: i32 = -25206;
const AX_ERROR_NOT_IMPLEMENTED: i32 = -25208;
const AX_ERROR_INVALID_ELEMENT: i32 = -25202;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> *mut c_void;
    fn AXUIElementCreateSystemWide() -> *mut c_void;
    fn AXUIElementCopyAttributeValue(
        element: *mut c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: *mut c_void,
        attribute: *const c_void,
        value: *const c_void,
    ) -> i32;
    fn AXUIElementPerformAction(element: *mut c_void, action: *const c_void) -> i32;
    fn AXUIElementGetPid(element: *mut c_void, pid: *mut libc::pid_t) -> i32;
    fn AXUIElementCopyElementAtPosition(
        application: *mut c_void,
        x: f32,
        y: f32,
        element: *mut *mut c_void,
    ) -> i32;
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    fn AXValueGetValue(value: *const c_void, the_type: u32, value_ptr: *mut c_void) -> bool;
    fn AXValueCreate(the_type: u32, value_ptr: *const c_void) -> *const c_void;
    // Private but long-stable: maps an AX window element to its CGWindowID, the
    // only reliable bridge between the CGWindowList and AX views of a window
    // (geometry matching is ambiguous when windows overlap exactly).
    fn _AXUIElementGetWindow(element: *mut c_void, out: *mut u32) -> i32;
}

unsafe extern "C-unwind" {
    fn CFRetain(cf: *const c_void) -> *const c_void;
    fn CFRelease(cf: *const c_void);
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(boolean: *const c_void) -> bool;
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
}

/// True if this process is trusted for the Accessibility API.
pub(super) fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Trigger the Accessibility "add this app" system prompt (and return the
/// current trust state). The user must still toggle the switch — this only
/// surfaces the prompt / adds the app to the list.
pub(super) fn prompt_trusted() -> bool {
    // {kAXTrustedCheckOptionPrompt: true}. The key's constant value is the
    // literal string "AXTrustedCheckOptionPrompt".
    let key = CFString::from_str("AXTrustedCheckOptionPrompt");
    let value = objc2_core_foundation::CFBoolean::new(true);
    let options = objc2_core_foundation::CFDictionary::from_slices(&[&*key], &[value]);
    unsafe {
        AXIsProcessTrustedWithOptions(
            (&*options as *const objc2_core_foundation::CFDictionary<CFString, _>).cast(),
        )
    }
}

/// Map a raw `AXError` to our taxonomy for a resolved target.
fn ax_err(code: i32, what: &str) -> Error {
    match code {
        AX_ERROR_API_DISABLED => Error::Permission(format!(
            "accessibility denied: grant Accessibility to {} in System Settings › Privacy & Security",
            crate::responsible_app()
        )),
        AX_ERROR_ATTRIBUTE_UNSUPPORTED | AX_ERROR_ACTION_UNSUPPORTED | AX_ERROR_NOT_IMPLEMENTED => {
            Error::Unsupported(format!("{what} is not supported by this element"))
        }
        AX_ERROR_INVALID_ELEMENT => Error::Stale(format!("{what}: the element no longer exists")),
        _ => Error::Failed(format!("{what} failed (AXError {code})")),
    }
}

/// Guard entry to any AX operation with a clear permission error.
pub(super) fn require_trusted() -> Result<()> {
    if is_trusted() {
        Ok(())
    } else {
        Err(Error::Permission(format!(
            "accessibility denied: grant Accessibility to {} in System Settings › Privacy & Security",
            crate::responsible_app()
        )))
    }
}

/// An owned CoreFoundation value (released on drop).
pub(super) struct Cf(*const c_void);

impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

/// An owned `AXUIElementRef` (released on drop).
pub(super) struct AxEl(*mut c_void);

impl Drop for AxEl {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0 as *const c_void) };
        }
    }
}

/// A CFString for an AX attribute/action name, kept alive for the FFI call.
fn name(s: &str) -> CFRetained<CFString> {
    CFString::from_str(s)
}

fn name_ptr(s: &CFRetained<CFString>) -> *const c_void {
    (&**s) as *const CFString as *const c_void
}

impl AxEl {
    fn targets_self(&self) -> bool {
        let mut pid: libc::pid_t = 0;
        (unsafe { AXUIElementGetPid(self.0, &mut pid) }) == AX_SUCCESS
            && pid == std::process::id() as libc::pid_t
    }

    /// AppKit services self-targeted AX mutations on the calling thread. Route
    /// those writes through the main queue; foreign-process AX calls remain
    /// safe and synchronous on the caller.
    fn mutate<F>(&self, operation: F) -> Result<i32>
    where
        F: FnOnce(*mut c_void) -> Result<i32> + Send,
    {
        if !self.targets_self() || unsafe { libc::pthread_main_np() } != 0 {
            return operation(self.0);
        }

        let element = self.0 as usize;
        let mut result = Err(Error::Failed("main-queue AX mutation did not run".into()));
        dispatch2::DispatchQueue::main().exec_sync(|| {
            result = operation(element as *mut c_void);
        });
        result
    }

    /// The AX element for an application by pid.
    pub(super) fn for_app(pid: i32) -> Result<AxEl> {
        let raw = unsafe { AXUIElementCreateApplication(pid) };
        if raw.is_null() {
            return Err(Error::Failed(format!("no AX element for pid {pid}")));
        }
        Ok(AxEl(raw))
    }

    /// The system-wide AX element (used for hit-testing).
    pub(super) fn system_wide() -> Result<AxEl> {
        let raw = unsafe { AXUIElementCreateSystemWide() };
        if raw.is_null() {
            return Err(Error::Failed("no system-wide AX element".into()));
        }
        Ok(AxEl(raw))
    }

    /// Deepest element at a screen point (global points).
    pub(super) fn element_at(&self, x: f32, y: f32) -> Result<AxEl> {
        let mut out: *mut c_void = std::ptr::null_mut();
        let rc = unsafe { AXUIElementCopyElementAtPosition(self.0, x, y, &mut out) };
        if rc != AX_SUCCESS || out.is_null() {
            return Err(Error::NotFound(format!("no accessible element at {x},{y}")));
        }
        Ok(AxEl(out))
    }

    /// Copy an attribute as an owned CF value.
    fn copy(&self, attr: &str) -> Option<Cf> {
        let (rc, value) = self.copy_raw(attr);
        if rc != AX_SUCCESS || value.is_null() {
            None
        } else {
            Some(Cf(value))
        }
    }

    fn copy_raw(&self, attr: &str) -> (i32, *const c_void) {
        Self::copy_element(self.0, attr)
    }

    fn copy_element(element: *mut c_void, attr: &str) -> (i32, *const c_void) {
        let key = name(attr);
        let mut value: *const c_void = std::ptr::null();
        let rc = unsafe { AXUIElementCopyAttributeValue(element, name_ptr(&key), &mut value) };
        (rc, value)
    }

    fn copy_self_app_attribute(&self, attr: &str) -> Option<Cf> {
        if !self.targets_self() || unsafe { libc::pthread_main_np() } != 0 {
            return self.copy(attr);
        }
        // AppKit services the current application's AXWindows attribute on its
        // main thread. Once the window element exists, its regular AX tree can
        // stay on the caller; routing every node attribute through the main
        // queue turns a bounded tree walk into thousands of synchronous hops.
        let element = self.0 as usize;
        let attr = attr.to_string();
        let mut result = (AX_ERROR_INVALID_ELEMENT, 0usize);
        dispatch2::DispatchQueue::main().exec_sync(|| {
            let (rc, value) = Self::copy_element(element as *mut c_void, &attr);
            result = (rc, value as usize);
        });
        let value = result.1 as *const c_void;
        (result.0 == AX_SUCCESS && !value.is_null()).then_some(Cf(value))
    }

    pub(super) fn attr_string(&self, attr: &str) -> Option<String> {
        let v = self.copy(attr)?;
        unsafe {
            if CFGetTypeID(v.0) != CFStringGetTypeID() {
                return None;
            }
            Some((*(v.0 as *const CFString)).to_string())
        }
    }

    pub(super) fn attr_bool(&self, attr: &str) -> Option<bool> {
        let v = self.copy(attr)?;
        unsafe {
            if CFGetTypeID(v.0) == CFBooleanGetTypeID() {
                Some(CFBooleanGetValue(v.0))
            } else {
                None
            }
        }
    }

    pub(super) fn attr_point(&self, attr: &str) -> Option<CGPoint> {
        let v = self.copy(attr)?;
        let mut p = CGPoint::new(0.0, 0.0);
        let ok = unsafe { AXValueGetValue(v.0, AX_VALUE_CGPOINT, &mut p as *mut _ as *mut c_void) };
        ok.then_some(p)
    }

    pub(super) fn attr_size(&self, attr: &str) -> Option<CGSize> {
        let v = self.copy(attr)?;
        let mut s = CGSize::new(0.0, 0.0);
        let ok = unsafe { AXValueGetValue(v.0, AX_VALUE_CGSIZE, &mut s as *mut _ as *mut c_void) };
        ok.then_some(s)
    }

    /// An attribute whose value is itself an AXUIElement (e.g. the close button).
    pub(super) fn attr_element(&self, attr: &str) -> Option<AxEl> {
        let v = self.copy(attr)?;
        let raw = v.0 as *mut c_void;
        if raw.is_null() {
            return None;
        }
        // Retain so the AxEl owns its own +1 independent of `v`'s drop.
        unsafe { CFRetain(v.0) };
        Some(AxEl(raw))
    }

    /// Child AX elements (`AXChildren`).
    pub(super) fn children(&self) -> Vec<AxEl> {
        let Some(v) = self.copy("AXChildren") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        unsafe {
            let count = CFArrayGetCount(v.0).max(0);
            for i in 0..count {
                let el = CFArrayGetValueAtIndex(v.0, i);
                if !el.is_null() {
                    CFRetain(el);
                    out.push(AxEl(el as *mut c_void));
                }
            }
        }
        out
    }

    /// The windows of an application element (`AXWindows`).
    pub(super) fn windows(&self) -> Vec<AxEl> {
        let Some(v) = self.copy_self_app_attribute("AXWindows") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        unsafe {
            let count = CFArrayGetCount(v.0).max(0);
            for i in 0..count {
                let el = CFArrayGetValueAtIndex(v.0, i);
                if !el.is_null() {
                    CFRetain(el);
                    out.push(AxEl(el as *mut c_void));
                }
            }
        }
        out
    }

    /// The `CGWindowID` of an AX window element, if it is a window.
    pub(super) fn window_id(&self) -> Option<u32> {
        let mut id: u32 = 0;
        let rc = unsafe { _AXUIElementGetWindow(self.0, &mut id) };
        (rc == AX_SUCCESS && id != 0).then_some(id)
    }

    /// A new owned handle to the same element (retains +1).
    pub(super) fn clone_ref(&self) -> AxEl {
        unsafe { CFRetain(self.0 as *const c_void) };
        AxEl(self.0)
    }

    pub(super) fn perform(&self, action: &str) -> Result<()> {
        let what = action.to_string();
        let action = what.clone();
        let rc = self.mutate(move |element| {
            let action = name(&action);
            Ok(unsafe { AXUIElementPerformAction(element, name_ptr(&action)) })
        })?;
        if rc == AX_SUCCESS {
            Ok(())
        } else {
            Err(ax_err(rc, &what))
        }
    }

    pub(super) fn set_bool(&self, attr: &str, value: bool) -> Result<()> {
        let what = attr.to_string();
        let attr = what.clone();
        let rc = self.mutate(move |element| {
            // Use a CFBoolean via objc2's CFBoolean singleton.
            let value = objc2_core_foundation::CFBoolean::new(value);
            let key = name(&attr);
            Ok(unsafe {
                AXUIElementSetAttributeValue(
                    element,
                    name_ptr(&key),
                    (value as *const objc2_core_foundation::CFBoolean).cast(),
                )
            })
        })?;
        if rc == AX_SUCCESS {
            Ok(())
        } else {
            Err(ax_err(rc, &what))
        }
    }

    pub(super) fn set_string(&self, attr: &str, value: &str) -> Result<()> {
        let what = attr.to_string();
        let attr = what.clone();
        let value = value.to_string();
        let rc = self.mutate(move |element| {
            let value = CFString::from_str(&value);
            let key = name(&attr);
            Ok(unsafe {
                AXUIElementSetAttributeValue(
                    element,
                    name_ptr(&key),
                    (&*value as *const CFString).cast(),
                )
            })
        })?;
        if rc == AX_SUCCESS {
            Ok(())
        } else {
            Err(ax_err(rc, &what))
        }
    }

    pub(super) fn set_point(&self, attr: &str, x: f64, y: f64) -> Result<()> {
        let what = attr.to_string();
        let attr = what.clone();
        let rc = self.mutate(move |element| {
            let point = CGPoint::new(x, y);
            let value = unsafe {
                AXValueCreate(AX_VALUE_CGPOINT, &point as *const CGPoint as *const c_void)
            };
            if value.is_null() {
                return Err(Error::Failed("could not create AXValue point".into()));
            }
            let key = name(&attr);
            let result = unsafe { AXUIElementSetAttributeValue(element, name_ptr(&key), value) };
            unsafe { CFRelease(value) };
            Ok(result)
        })?;
        if rc == AX_SUCCESS {
            Ok(())
        } else {
            Err(ax_err(rc, &what))
        }
    }

    pub(super) fn set_size(&self, attr: &str, w: f64, h: f64) -> Result<()> {
        let what = attr.to_string();
        let attr = what.clone();
        let rc = self.mutate(move |element| {
            let size = CGSize::new(w, h);
            let value =
                unsafe { AXValueCreate(AX_VALUE_CGSIZE, &size as *const CGSize as *const c_void) };
            if value.is_null() {
                return Err(Error::Failed("could not create AXValue size".into()));
            }
            let key = name(&attr);
            let result = unsafe { AXUIElementSetAttributeValue(element, name_ptr(&key), value) };
            unsafe { CFRelease(value) };
            Ok(result)
        })?;
        if rc == AX_SUCCESS {
            Ok(())
        } else {
            Err(ax_err(rc, &what))
        }
    }
}
