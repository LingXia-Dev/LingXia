//! The page view's CAMetalLayer, taken from the view on the main thread: wgpu binds a view only
//! there, while the render thread may bind and present into its layer.
use lingxia_view::renderer::gpu::MetalLayer;
use std::ffi::{c_char, c_void};
use std::ptr::NonNull;

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_msgSend();
    fn sel_registerName(name: *const c_char) -> *const c_void;
    fn objc_getClass(name: *const c_char) -> *const c_void;
    fn objc_retain(object: *mut c_void) -> *mut c_void;
    fn objc_release(object: *mut c_void);
}

/// A retained CAMetalLayer.
pub(crate) struct AppleLayer(NonNull<c_void>);

// CAMetalLayer may be configured and draw drawables from any thread.
unsafe impl Send for AppleLayer {}
unsafe impl Sync for AppleLayer {}

impl AppleLayer {
    /// The layer backing `view` (an NSView or UIView), when it is a CAMetalLayer. Main thread.
    pub(crate) fn of_view(view: usize) -> Option<Self> {
        let view = NonNull::new(view as *mut c_void)?;
        // SAFETY: `view` is a live view handed over by Swift on the main thread; the selectors
        // take no arguments but the class, and return objects or BOOL as declared here.
        unsafe {
            let send: unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let layer = NonNull::new(send(view.as_ptr(), sel_registerName(c"layer".as_ptr())))?;
            let is_kind: unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void) -> bool =
                std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
            let metal = objc_getClass(c"CAMetalLayer".as_ptr());
            if metal.is_null()
                || !is_kind(
                    layer.as_ptr(),
                    sel_registerName(c"isKindOfClass:".as_ptr()),
                    metal,
                )
            {
                return None;
            }
            NonNull::new(objc_retain(layer.as_ptr())).map(Self)
        }
    }
}

impl MetalLayer for AppleLayer {
    fn layer(&self) -> NonNull<c_void> {
        self.0
    }
}

impl Drop for AppleLayer {
    fn drop(&mut self) {
        // SAFETY: retained in `of_view`, released once.
        unsafe { objc_release(self.0.as_ptr()) }
    }
}
