//! The XComponent's `OHNativeWindow` as a presentation target: a wgpu GLES surface
//! over EGL, or CPU frames written straight into its buffers.
use crate::sys;
use std::ptr::NonNull;

/// One reference on a native window, dropped with the value.
pub(crate) struct NativeWindow(NonNull<sys::OHNativeWindow>);

// The window is reference-counted by the system and usable from render threads.
unsafe impl Send for NativeWindow {}
unsafe impl Sync for NativeWindow {}

impl NativeWindow {
    /// Takes a reference on `window`, so it outlives the XComponent's own surface callbacks.
    pub(crate) fn retain(window: *mut sys::OHNativeWindow) -> Option<Self> {
        let window = NonNull::new(window)?;
        // SAFETY: a live window from the surface holder.
        unsafe { sys::OH_NativeWindow_NativeObjectReference(window.as_ptr().cast()) };
        Some(Self(window))
    }

    pub(crate) fn as_ptr(&self) -> *mut sys::OHNativeWindow {
        self.0.as_ptr()
    }

    /// The buffer size the producer allocates; EGL keeps it from surface creation.
    pub(crate) fn set_geometry(&self, width: u32, height: u32) {
        // SAFETY: variadic `SET_BUFFER_GEOMETRY` takes two ints.
        unsafe {
            sys::OH_NativeWindow_NativeWindowHandleOpt(
                self.as_ptr(),
                sys::SET_BUFFER_GEOMETRY,
                width as i32,
                height as i32,
            )
        };
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        // SAFETY: balances the reference taken in `retain`.
        unsafe { sys::OH_NativeWindow_NativeObjectUnreference(self.0.as_ptr().cast()) };
    }
}

mod gpu {
    use super::NativeWindow;
    use raw_window_handle::{
        DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, OhosDisplayHandle,
        OhosNdkWindowHandle, WindowHandle,
    };

    impl HasWindowHandle for NativeWindow {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            // SAFETY: the retained window outlives every borrowed handle and the wgpu surface.
            Ok(unsafe { WindowHandle::borrow_raw(OhosNdkWindowHandle::new(self.0.cast()).into()) })
        }
    }

    impl HasDisplayHandle for NativeWindow {
        fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
            Ok(unsafe { DisplayHandle::borrow_raw(OhosDisplayHandle::new().into()) })
        }
    }
}

/// Writes one premultiplied RGBA frame into the window's next buffer; false when none
/// is free (the host re-prepares the frame).
pub(crate) fn blit(_window: &NativeWindow, _pixels: &[u8], _width: u32, _height: u32) -> bool {
    log::warn!(target: "lingxia_view", "UI_SURFACE: CPU frames are not shown on HarmonyOS yet");
    false
}
