//! One native XComponent node per page, mounted in an ArkTS `ContentSlot`. ArkUI calls
//! back on the main thread: surface lifetime, touch, keys, focus and display frames.
use crate::{accessibility, pages, surface::NativeWindow, sys};
use lingxia_view::interaction::view::{Pointer, PointerPhase};
use std::ffi::{CString, c_void};
use std::sync::OnceLock;

/// Touch slop and the slowest fling, in virtual pixels (ArkUI's pan and fling defaults).
const SLOP_VP: f32 = 8.0;
const MIN_FLING_VP: f32 = 50.0;
/// Velocity looks this far back from the last sample.
const VELOCITY_WINDOW_NS: i64 = 100_000_000;

fn api() -> Result<&'static sys::ArkUI_NativeNodeAPI_1, String> {
    static API: OnceLock<usize> = OnceLock::new();
    let api = *API.get_or_init(|| {
        // SAFETY: queried by its published name; the table lives for the process.
        unsafe {
            sys::OH_ArkUI_QueryModuleInterfaceByName(
                sys::ARKUI_NATIVE_NODE,
                c"ArkUI_NativeNodeAPI_1".as_ptr(),
            ) as usize
        }
    });
    // SAFETY: a non-null result is the `ArkUI_NativeNodeAPI_1` table.
    unsafe { (api as *const sys::ArkUI_NativeNodeAPI_1).as_ref() }
        .ok_or_else(|| "UI_SURFACE: no ArkUI native node API".into())
}

/// Recent touch samples for the release velocity, device pixels and nanoseconds.
#[derive(Default)]
pub(crate) struct Velocity(Vec<(i64, f32, f32)>);

impl Velocity {
    fn add(&mut self, time: i64, x: f32, y: f32) {
        self.0.retain(|(t, ..)| time - t <= VELOCITY_WINDOW_NS);
        self.0.push((time, x, y));
    }

    /// Least-squares slope over the window, pixels per second.
    fn estimate(&self) -> [f32; 2] {
        let n = self.0.len() as f64;
        if n < 2.0 {
            return [0.0; 2];
        }
        let t0 = self.0[0].0;
        let ts: Vec<f64> = self.0.iter().map(|(t, ..)| (t - t0) as f64 / 1e9).collect();
        let mean_t = ts.iter().sum::<f64>() / n;
        let slope = |values: Vec<f64>| {
            let mean_v = values.iter().sum::<f64>() / n;
            let (mut num, mut den) = (0.0, 0.0);
            for (t, v) in ts.iter().zip(&values) {
                num += (t - mean_t) * (v - mean_v);
                den += (t - mean_t).powi(2);
            }
            if den > 0.0 { (num / den) as f32 } else { 0.0 }
        };
        [
            slope(self.0.iter().map(|s| f64::from(s.1)).collect()),
            slope(self.0.iter().map(|s| f64::from(s.2)).collect()),
        ]
    }
}

pub(crate) struct Node {
    token: i64,
    handle: sys::ArkUI_NodeHandle,
    content: sys::ArkUI_NodeContentHandle,
    holder: *mut sys::OH_ArkUI_SurfaceHolder,
    callback: *mut sys::OH_ArkUI_SurfaceCallback,
    pub(crate) provider: *mut sys::ArkUI_AccessibilityProvider,
}

const EVENTS: [i32; 6] = [
    sys::NODE_TOUCH_EVENT,
    sys::NODE_ON_FOCUS,
    sys::NODE_ON_BLUR,
    sys::NODE_ON_KEY_EVENT,
    sys::NODE_ON_HOVER,
    sys::NODE_ON_AXIS,
];

impl Node {
    /// Creates the page's XComponent node and adds it to `content`.
    pub(crate) fn mount(token: i64, content: sys::ArkUI_NodeContentHandle) -> Result<Self, String> {
        let api = api()?;
        let user = token as usize as *mut c_void;
        let id = CString::new(format!("lingxia-gpu-{token}")).expect("digits");
        // SAFETY: main thread; every handle is checked and owned by the returned node.
        unsafe {
            let handle = (api.create_node)(sys::ARKUI_NODE_XCOMPONENT);
            if handle.is_null() {
                return Err("UI_SURFACE: XComponent node creation failed".into());
            }
            let string = |s: &CString| sys::ArkUI_AttributeItem {
                value: std::ptr::null(),
                size: 0,
                string: s.as_ptr(),
                object: std::ptr::null_mut(),
            };
            let number = |v: &sys::ArkUI_NumberValue| sys::ArkUI_AttributeItem {
                value: v,
                size: 1,
                string: std::ptr::null(),
                object: std::ptr::null_mut(),
            };
            let full = sys::ArkUI_NumberValue { f32: 1.0 };
            let surface = sys::ArkUI_NumberValue {
                i32: sys::ARKUI_XCOMPONENT_TYPE_SURFACE,
            };
            let yes = sys::ArkUI_NumberValue { i32: 1 };
            (api.set_attribute)(handle, sys::NODE_XCOMPONENT_ID, &string(&id));
            (api.set_attribute)(handle, sys::NODE_XCOMPONENT_TYPE, &number(&surface));
            (api.set_attribute)(handle, sys::NODE_WIDTH_PERCENT, &number(&full));
            (api.set_attribute)(handle, sys::NODE_HEIGHT_PERCENT, &number(&full));
            (api.set_attribute)(handle, sys::NODE_FOCUSABLE, &number(&yes));
            // The page runs its own input connection; the XComponent must not raise one.
            sys::OH_ArkUI_XComponent_SetNeedSoftKeyboard(handle, false);
            for kind in EVENTS {
                (api.register_node_event)(handle, kind, 0, user);
            }
            (api.add_node_event_receiver)(handle, on_event);

            let holder = sys::OH_ArkUI_SurfaceHolder_Create(handle);
            let callback = sys::OH_ArkUI_SurfaceCallback_Create();
            sys::OH_ArkUI_SurfaceHolder_SetUserData(holder, user);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceCreatedEvent(callback, surface_created);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceChangedEvent(callback, surface_changed);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceDestroyedEvent(callback, surface_destroyed);
            sys::OH_ArkUI_SurfaceHolder_AddSurfaceCallback(holder, callback);

            let provider = sys::OH_ArkUI_AccessibilityProvider_Create(handle);
            if !provider.is_null() {
                accessibility::attach(token, provider)?;
            }
            pages::bind_node(handle, token);
            let code = sys::OH_ArkUI_NodeContent_AddNode(content, handle);
            if code != 0 {
                return Err(format!("UI_SURFACE: ContentSlot refused the node ({code})"));
            }
            Ok(Self {
                token,
                handle,
                content,
                holder,
                callback,
                provider,
            })
        }
    }

    /// Ask for one display frame callback; ArkUI keeps calling until unregistered.
    pub(crate) fn request_frame(&self) {
        // SAFETY: main thread, live node.
        unsafe { sys::OH_ArkUI_XComponent_RegisterOnFrameCallback(self.handle, on_frame) };
    }

    pub(crate) fn stop_frames(&self) {
        // SAFETY: main thread, live node.
        unsafe { sys::OH_ArkUI_XComponent_UnregisterOnFrameCallback(self.handle) };
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let Ok(api) = api() else { return };
        pages::unbind_node(self.handle);
        // SAFETY: main thread; releases what `mount` created, in reverse.
        unsafe {
            if !self.provider.is_null() {
                sys::OH_ArkUI_AccessibilityProvider_Dispose(self.provider);
            }
            sys::OH_ArkUI_XComponent_UnregisterOnFrameCallback(self.handle);
            sys::OH_ArkUI_NodeContent_RemoveNode(self.content, self.handle);
            for kind in EVENTS {
                (api.unregister_node_event)(self.handle, kind);
            }
            (api.remove_node_event_receiver)(self.handle, on_event);
            sys::OH_ArkUI_SurfaceHolder_Dispose(self.holder);
            sys::OH_ArkUI_SurfaceCallback_Dispose(self.callback);
            (api.dispose_node)(self.handle);
        }
        let _ = self.token;
    }
}

fn holder_token(holder: *mut sys::OH_ArkUI_SurfaceHolder) -> i64 {
    // SAFETY: user data set in `mount` to the page token.
    unsafe { sys::OH_ArkUI_SurfaceHolder_GetUserData(holder) as usize as i64 }
}

unsafe extern "C" fn surface_created(holder: *mut sys::OH_ArkUI_SurfaceHolder) {
    // SAFETY: ArkUI's live holder.
    let window = NativeWindow::retain(unsafe { sys::OH_ArkUI_XComponent_GetNativeWindow(holder) });
    let token = holder_token(holder);
    pages::with(token, |host| match window {
        Some(window) => host.adapter().surface_created(host, window),
        None => log::warn!(target: "lingxia_view", "UI_SURFACE: page {token} has no native window"),
    });
}

unsafe extern "C" fn surface_changed(
    holder: *mut sys::OH_ArkUI_SurfaceHolder,
    width: u64,
    height: u64,
) {
    pages::with(holder_token(holder), |host| {
        host.adapter()
            .surface_resized(host, width as u32, height as u32)
    });
}

unsafe extern "C" fn surface_destroyed(holder: *mut sys::OH_ArkUI_SurfaceHolder) {
    pages::with(holder_token(holder), |host| {
        host.adapter().surface_destroyed(host)
    });
}

unsafe extern "C" fn on_frame(node: sys::ArkUI_NodeHandle, _timestamp: u64, _target: u64) {
    let Some(token) = pages::node_token(node) else {
        return;
    };
    pages::with(token, |host| host.adapter().display_frame(host));
}

unsafe extern "C" fn on_event(event: *mut sys::ArkUI_NodeEvent) {
    // SAFETY: ArkUI's live event for this call.
    let (kind, token, input) = unsafe {
        (
            sys::OH_ArkUI_NodeEvent_GetEventType(event),
            sys::OH_ArkUI_NodeEvent_GetUserData(event) as usize as i64,
            sys::OH_ArkUI_NodeEvent_GetInputEvent(event),
        )
    };
    pages::with(token, |host| match kind {
        sys::NODE_TOUCH_EVENT if !input.is_null() => touch(host, input),
        sys::NODE_ON_FOCUS => host.focus_entered(false),
        sys::NODE_ON_BLUR => host.focus_left(),
        _ => {}
    });
}

fn touch(host: &pages::Host, input: *mut sys::ArkUI_UIInputEvent) {
    let adapter = host.adapter();
    // SAFETY: the live input event of this callback.
    let (action, x, y, time) = unsafe {
        (
            sys::OH_ArkUI_UIInputEvent_GetAction(input),
            sys::OH_ArkUI_PointerEvent_GetX(input),
            sys::OH_ArkUI_PointerEvent_GetY(input),
            sys::OH_ArkUI_UIInputEvent_GetEventTime(input),
        )
    };
    let phase = match action {
        sys::UI_TOUCH_EVENT_ACTION_DOWN => PointerPhase::Down,
        sys::UI_TOUCH_EVENT_ACTION_MOVE => PointerPhase::Move,
        sys::UI_TOUCH_EVENT_ACTION_UP => PointerPhase::Up,
        _ => PointerPhase::Cancel,
    };
    if matches!(phase, PointerPhase::Down) {
        adapter.begin_gesture();
    }
    let velocity = {
        let mut tracker = adapter.velocity();
        // SAFETY: history indices below the reported size.
        unsafe {
            for i in 0..sys::OH_ArkUI_PointerEvent_GetHistorySize(input) {
                tracker.add(
                    sys::OH_ArkUI_PointerEvent_GetHistoryEventTime(input, i),
                    sys::OH_ArkUI_PointerEvent_GetHistoryX(input, 0, i),
                    sys::OH_ArkUI_PointerEvent_GetHistoryY(input, 0, i),
                );
            }
        }
        tracker.add(time, x, y);
        if matches!(phase, PointerPhase::Up) {
            tracker.estimate()
        } else {
            [0.0; 2]
        }
    };
    let scale = adapter.placement().0;
    let consumed = host.pointer(Pointer {
        phase,
        x,
        y,
        slop: SLOP_VP * scale,
        velocity,
        min_fling: MIN_FLING_VP * scale,
    });
    if consumed && adapter.claimed() {
        // SAFETY: the live input event of this callback.
        unsafe { sys::OH_ArkUI_PointerEvent_SetStopPropagation(input, true) };
    }
}
