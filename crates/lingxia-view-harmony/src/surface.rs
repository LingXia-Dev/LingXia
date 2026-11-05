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
/// is free (the host re-prepares the frame). Any thread.
pub(crate) fn blit(window: &NativeWindow, pixels: &[u8], width: u32, height: u32) -> bool {
    let window = window.as_ptr();
    // SAFETY: a retained window; the buffer is ours between request and flush or abort.
    unsafe {
        sys::OH_NativeWindow_NativeWindowHandleOpt(
            window,
            sys::SET_FORMAT,
            sys::PIXEL_FMT_RGBA_8888,
        );
        sys::OH_NativeWindow_NativeWindowHandleOpt(
            window,
            sys::SET_USAGE,
            sys::USAGE_CPU_READ_WRITE_DMA,
        );
        let mut buffer = std::ptr::null_mut();
        let mut fence = -1;
        if sys::OH_NativeWindow_NativeWindowRequestBuffer(window, &mut buffer, &mut fence) != 0
            || buffer.is_null()
        {
            return false;
        }
        if fence >= 0 {
            let mut poll = libc::pollfd {
                fd: fence,
                events: libc::POLLIN,
                revents: 0,
            };
            libc::poll(&mut poll, 1, 3000);
            libc::close(fence);
        }
        let handle = sys::OH_NativeWindow_GetBufferHandleFromNative(buffer);
        let Some(handle) = handle.as_ref() else {
            sys::OH_NativeWindow_NativeWindowAbortBuffer(window, buffer);
            return false;
        };
        let size = handle.size.max(0) as usize;
        let (base, mapped) = if handle.vir_addr.is_null() {
            let map = libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                handle.fd,
                0,
            );
            if map == libc::MAP_FAILED {
                sys::OH_NativeWindow_NativeWindowAbortBuffer(window, buffer);
                return false;
            }
            (map.cast::<u8>(), true)
        } else {
            (handle.vir_addr.cast::<u8>(), false)
        };
        let stride = handle.stride.max(0) as usize;
        let row = (width as usize * 4).min(stride);
        for y in 0..(height as usize).min(handle.height.max(0) as usize) {
            let from = y * width as usize * 4;
            if from + row > pixels.len() || y * stride + row > size {
                break;
            }
            std::ptr::copy_nonoverlapping(pixels.as_ptr().add(from), base.add(y * stride), row);
        }
        if mapped {
            libc::munmap(base.cast(), size);
        }
        let region = sys::Region {
            rects: std::ptr::null_mut(),
            rect_number: 0,
        };
        sys::OH_NativeWindow_NativeWindowFlushBuffer(window, buffer, -1, region) == 0
    }
}
