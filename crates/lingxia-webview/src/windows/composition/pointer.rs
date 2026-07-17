//! WM_POINTER touch/pen forwarding for composition-hosted WebView2 (the
//! WebView2 Win32 sample's pointer conversion). Mouse-type pointers fall
//! through to the legacy mouse path.

use super::*;
use windows::Win32::Foundation::{LRESULT, POINT};
use windows::Win32::UI::Input::Pointer::{
    GetPointerDeviceRects, GetPointerInfo, GetPointerPenInfo, GetPointerTouchInfo, GetPointerType,
    POINTER_INFO, POINTER_PEN_INFO, POINTER_TOUCH_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{PT_PEN, PT_TOUCH};

/// Forwards a WM_POINTERDOWN/UPDATE/UP for a touch or pen pointer. `None`
/// means "not ours" (mouse pointer or lookup failure) — let `DefWindowProcW`
/// generate the legacy mouse messages instead.
pub(crate) fn forward_pointer_message(
    env3: &ICoreWebView2Environment3,
    controller: &ICoreWebView2CompositionController,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
) -> Option<LRESULT> {
    let pointer_id = (wparam.0 & 0xffff) as u32;
    let mut pointer_type = windows::Win32::UI::WindowsAndMessaging::POINTER_INPUT_TYPE::default();
    unsafe { GetPointerType(pointer_id, &mut pointer_type).ok()? };
    if pointer_type != PT_TOUCH && pointer_type != PT_PEN {
        return None;
    }
    let info = build_pointer_info(env3, hwnd, pointer_id, pointer_type.0 as u32)?;
    let result =
        unsafe { controller.SendPointerInput(COREWEBVIEW2_POINTER_EVENT_KIND(msg as i32), &info) };
    if let Err(err) = result {
        log::debug!("SendPointerInput({msg:#06x}) failed: {err}");
    }
    Some(LRESULT(0))
}

fn build_pointer_info(
    env3: &ICoreWebView2Environment3,
    hwnd: HWND,
    pointer_id: u32,
    pointer_kind: u32,
) -> Option<ICoreWebView2PointerInfo> {
    let mut info = POINTER_INFO::default();
    unsafe { GetPointerInfo(pointer_id, &mut info).ok()? };
    let out = unsafe { env3.CreateCoreWebView2PointerInfo().ok()? };

    let mut origin = POINT::default();
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut origin);
    }
    let to_client = |pt: POINT| POINT {
        x: pt.x - origin.x,
        y: pt.y - origin.y,
    };

    let mut device_rect = RECT::default();
    let mut display_rect = RECT::default();
    let have_rects = unsafe {
        GetPointerDeviceRects(info.sourceDevice, &mut device_rect, &mut display_rect).is_ok()
    };
    // Himetric locations shift by the client origin scaled into pointer-
    // device space, so WebView2 maps them back onto the same client point.
    let scale_x = (device_rect.right - device_rect.left) as f32
        / ((display_rect.right - display_rect.left).max(1)) as f32;
    let scale_y = (device_rect.bottom - device_rect.top) as f32
        / ((display_rect.bottom - display_rect.top).max(1)) as f32;
    let to_himetric_client = |pt: POINT| POINT {
        x: pt.x - ((origin.x - display_rect.left) as f32 * scale_x) as i32,
        y: pt.y - ((origin.y - display_rect.top) as f32 * scale_y) as i32,
    };

    unsafe {
        let _ = out.SetPointerKind(pointer_kind);
        let _ = out.SetPointerId(pointer_id);
        let _ = out.SetFrameId(info.frameId);
        let _ = out.SetPointerFlags(info.pointerFlags.0);
        if have_rects {
            let _ = out.SetPointerDeviceRect(device_rect);
            let _ = out.SetDisplayRect(display_rect);
            let _ = out.SetHimetricLocation(to_himetric_client(info.ptHimetricLocation));
            let _ = out.SetHimetricLocationRaw(to_himetric_client(info.ptHimetricLocationRaw));
        }
        let _ = out.SetPixelLocation(to_client(info.ptPixelLocation));
        let _ = out.SetPixelLocationRaw(to_client(info.ptPixelLocationRaw));
        let _ = out.SetTime(info.dwTime);
        let _ = out.SetHistoryCount(info.historyCount);
        let _ = out.SetInputData(info.InputData);
        let _ = out.SetKeyStates(info.dwKeyStates);
        let _ = out.SetPerformanceCount(info.PerformanceCount);
        let _ = out.SetButtonChangeKind(info.ButtonChangeType.0);
    }

    let to_client_rect = |rect: RECT| RECT {
        left: rect.left - origin.x,
        top: rect.top - origin.y,
        right: rect.right - origin.x,
        bottom: rect.bottom - origin.y,
    };
    if pointer_kind == PT_TOUCH.0 as u32 {
        let mut touch = POINTER_TOUCH_INFO::default();
        if unsafe { GetPointerTouchInfo(pointer_id, &mut touch) }.is_ok() {
            unsafe {
                let _ = out.SetTouchFlags(touch.touchFlags);
                let _ = out.SetTouchMask(touch.touchMask);
                let _ = out.SetTouchContact(to_client_rect(touch.rcContact));
                let _ = out.SetTouchContactRaw(to_client_rect(touch.rcContactRaw));
                let _ = out.SetTouchOrientation(touch.orientation);
                let _ = out.SetTouchPressure(touch.pressure);
            }
        }
    } else {
        let mut pen = POINTER_PEN_INFO::default();
        if unsafe { GetPointerPenInfo(pointer_id, &mut pen) }.is_ok() {
            unsafe {
                let _ = out.SetPenFlags(pen.penFlags);
                let _ = out.SetPenMask(pen.penMask);
                let _ = out.SetPenPressure(pen.pressure);
                let _ = out.SetPenRotation(pen.rotation);
                let _ = out.SetPenTiltX(pen.tiltX);
                let _ = out.SetPenTiltY(pen.tiltY);
            }
        }
    }
    Some(out)
}
