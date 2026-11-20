//! The page's content window as a wgpu surface target: DXGI binds a flip-model swap chain to
//! it, and the render thread presents into it.
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    Win32WindowHandle, WindowHandle, WindowsDisplayHandle,
};
use std::num::NonZeroIsize;

/// A content window that outlives its presenter: the page detaches the presenter before it
/// destroys the window.
pub(crate) struct Hwnd(NonZeroIsize);

impl Hwnd {
    pub(crate) fn new(hwnd: isize) -> Option<Self> {
        NonZeroIsize::new(hwnd).map(Self)
    }
}

impl HasWindowHandle for Hwnd {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::Win32(Win32WindowHandle::new(self.0));
        // SAFETY: the window lives until the presenter bound to it is detached.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for Hwnd {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(unsafe { DisplayHandle::borrow_raw(WindowsDisplayHandle::new().into()) })
    }
}
