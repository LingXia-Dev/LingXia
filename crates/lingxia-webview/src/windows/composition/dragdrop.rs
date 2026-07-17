//! OLE drag-and-drop into a composition-hosted WebView2: the windowed
//! controller registers its own drop target, the composition controller
//! expects the host to forward `IDropTarget` calls.

use super::*;
use windows::Win32::Foundation::POINTL;
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Ole::{
    DROPEFFECT, IDropTarget, IDropTarget_Impl, RegisterDragDrop, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::core::{Ref, implement};

#[implement(IDropTarget)]
struct SurfaceDropTarget {
    hwnd: HWND,
    controller: ICoreWebView2CompositionController3,
}

impl SurfaceDropTarget {
    /// Drop points arrive in screen coordinates; WebView2 wants them
    /// relative to the surface.
    fn client_point(&self, pt: &POINTL) -> windows::Win32::Foundation::POINT {
        let mut point = windows::Win32::Foundation::POINT { x: pt.x, y: pt.y };
        unsafe {
            let _ = windows::Win32::Graphics::Gdi::ScreenToClient(self.hwnd, &mut point);
        }
        point
    }
}

impl IDropTarget_Impl for SurfaceDropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Ref<IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        unsafe {
            self.controller.DragEnter(
                pdataobj.as_ref(),
                grfkeystate.0,
                self.client_point(pt),
                pdweffect as *mut u32,
            )
        }
    }

    fn DragOver(
        &self,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        unsafe {
            self.controller
                .DragOver(grfkeystate.0, self.client_point(pt), pdweffect as *mut u32)
        }
    }

    fn DragLeave(&self) -> WinResult<()> {
        unsafe { self.controller.DragLeave() }
    }

    fn Drop(
        &self,
        pdataobj: Ref<IDataObject>,
        grfkeystate: MODIFIERKEYS_FLAGS,
        pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> WinResult<()> {
        unsafe {
            self.controller.Drop(
                pdataobj.as_ref(),
                grfkeystate.0,
                self.client_point(pt),
                pdweffect as *mut u32,
            )
        }
    }
}

/// Registers the surface window as a drop target forwarding to the
/// controller. Requires `OleInitialize` on this thread. Best-effort: an old
/// runtime without CompositionController3 just loses drop-into-page.
pub(crate) fn register_drop_target(hwnd: HWND, controller: &ICoreWebView2CompositionController) {
    let controller3: ICoreWebView2CompositionController3 = match controller.cast() {
        Ok(controller3) => controller3,
        Err(err) => {
            log::warn!("drag-drop unavailable (no CompositionController3): {err}");
            return;
        }
    };
    let target: IDropTarget = SurfaceDropTarget {
        hwnd,
        controller: controller3,
    }
    .into();
    unsafe {
        if let Err(err) = RegisterDragDrop(hwnd, &target) {
            log::warn!("RegisterDragDrop failed: {err}");
        }
    }
}

pub(crate) fn revoke_drop_target(hwnd: HWND) {
    unsafe {
        let _ = RevokeDragDrop(hwnd);
    }
}
