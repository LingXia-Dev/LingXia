//! The page view as a wgpu surface target. The view's backing layer is a
//! CAMetalLayer, so wgpu presents into it without touching the view hierarchy.
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;

pub(crate) struct AppleView(NonNull<c_void>);

// The Swift registry keeps the view alive until its page detaches the presenter.
unsafe impl Send for AppleView {}
unsafe impl Sync for AppleView {}

impl AppleView {
    pub(crate) fn new(view: usize) -> Option<Self> {
        NonNull::new(view as *mut c_void).map(Self)
    }
}

impl HasWindowHandle for AppleView {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        #[cfg(target_os = "macos")]
        let raw = RawWindowHandle::AppKit(raw_window_handle::AppKitWindowHandle::new(self.0));
        #[cfg(not(target_os = "macos"))]
        let raw = RawWindowHandle::UiKit(raw_window_handle::UiKitWindowHandle::new(self.0));
        // SAFETY: the view outlives every borrowed handle and the wgpu surface.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for AppleView {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        #[cfg(target_os = "macos")]
        let raw = RawDisplayHandle::AppKit(raw_window_handle::AppKitDisplayHandle::new());
        #[cfg(not(target_os = "macos"))]
        let raw = RawDisplayHandle::UiKit(raw_window_handle::UiKitDisplayHandle::new());
        Ok(unsafe { DisplayHandle::borrow_raw(raw) })
    }
}
