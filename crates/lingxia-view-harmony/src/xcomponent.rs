//! One native XComponent node per page, mounted in an ArkTS `ContentSlot`. ArkUI calls
//! back on the main thread: surface lifetime, touch, keys, focus and display frames.
use crate::{accessibility, pages, surface::NativeWindow, sys};
use lingxia_view::interaction::view::{Key, Pointer, PointerPhase};
use std::ffi::{CString, c_void};
use std::sync::OnceLock;

/// Touch slop and the slowest fling, in virtual pixels (ArkUI's pan and fling defaults).
pub(crate) const SLOP_VP: f32 = 8.0;
pub(crate) const MIN_FLING_VP: f32 = 50.0;
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
    pub(crate) fn add(&mut self, time: i64, x: f32, y: f32) {
        self.0.retain(|(t, ..)| time - t <= VELOCITY_WINDOW_NS);
        self.0.push((time, x, y));
    }

    /// Least-squares slope over the window, pixels per second.
    pub(crate) fn estimate(&self) -> [f32; 2] {
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
    pub(crate) content: sys::ArkUI_NodeContentHandle,
    pub(crate) texture: bool,
    holder: *mut sys::OH_ArkUI_SurfaceHolder,
    callback: *mut sys::OH_ArkUI_SurfaceCallback,
    pub(crate) provider: *mut sys::ArkUI_AccessibilityProvider,
}

const SURFACE_EVENTS: [i32; 7] = [
    sys::NODE_TOUCH_EVENT,
    sys::NODE_ON_FOCUS,
    sys::NODE_ON_BLUR,
    sys::NODE_ON_HOVER,
    sys::NODE_ON_MOUSE,
    sys::NODE_ON_AXIS,
    sys::NODE_ON_KEY_EVENT,
];

const TEXTURE_EVENTS: [i32; 8] = [
    sys::NODE_TOUCH_EVENT,
    sys::NODE_ON_TOUCH_INTERCEPT,
    sys::NODE_ON_FOCUS,
    sys::NODE_ON_BLUR,
    sys::NODE_ON_HOVER,
    sys::NODE_ON_MOUSE,
    sys::NODE_ON_AXIS,
    sys::NODE_ON_KEY_EVENT,
];

impl Node {
    /// Creates the page's XComponent node and adds it to `content`. Pages use `SURFACE` by
    /// default so RenderService scans out without an extra ArkUI composition pass; `texture`
    /// switches to `TEXTURE` only while `<webview>` cutouts sit beneath the page.
    pub(crate) fn mount(
        token: i64,
        content: sys::ArkUI_NodeContentHandle,
        texture: bool,
    ) -> Result<Self, String> {
        let api = api()?;
        let user = token as usize as *mut c_void;
        let id = CString::new(format!("lingxia-gpu-{token}")).expect("digits");
        // SAFETY: main thread; every handle is checked and owned by the returned node.
        unsafe {
            let mut handle = if texture {
                (api.create_node)(sys::ARKUI_NODE_XCOMPONENT_TEXTURE)
            } else {
                std::ptr::null_mut()
            };
            let is_surface = handle.is_null();
            if is_surface {
                handle = (api.create_node)(sys::ARKUI_NODE_XCOMPONENT);
            }
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
            let yes = sys::ArkUI_NumberValue { i32: 1 };
            (api.set_attribute)(handle, sys::NODE_XCOMPONENT_ID, &string(&id));
            if is_surface {
                let surface = sys::ArkUI_NumberValue {
                    i32: sys::ARKUI_XCOMPONENT_TYPE_SURFACE,
                };
                (api.set_attribute)(handle, sys::NODE_XCOMPONENT_TYPE, &number(&surface));
            } else {
                let transparent = sys::ArkUI_NumberValue { u32: 0x0000_0000 };
                (api.set_attribute)(handle, sys::NODE_BACKGROUND_COLOR, &number(&transparent));
            }
            (api.set_attribute)(handle, sys::NODE_WIDTH_PERCENT, &number(&full));
            (api.set_attribute)(handle, sys::NODE_HEIGHT_PERCENT, &number(&full));
            (api.set_attribute)(handle, sys::NODE_FOCUSABLE, &number(&yes));
            // ArkUI only routes hardware keys to a focused node, and touch focuses nothing by default.
            (api.set_attribute)(handle, sys::NODE_FOCUS_ON_TOUCH, &number(&yes));
            // The page runs its own input connection; the XComponent must not raise one.
            sys::OH_ArkUI_XComponent_SetNeedSoftKeyboard(handle, false);
            let events: &[i32] = if is_surface {
                &SURFACE_EVENTS
            } else {
                &TEXTURE_EVENTS
            };
            for &kind in events {
                let code = (api.register_node_event)(handle, kind, 0, user);
                if code != 0 {
                    log::warn!(target: "lingxia_view", "UI_INPUT: node event {kind} refused ({code})");
                }
            }
            let code = (api.add_node_event_receiver)(handle, on_event);
            if code != 0 {
                log::warn!(target: "lingxia_view", "UI_INPUT: node event receiver refused ({code})");
            }

            let holder = sys::OH_ArkUI_SurfaceHolder_Create(handle);
            let callback = sys::OH_ArkUI_SurfaceCallback_Create();
            sys::OH_ArkUI_SurfaceHolder_SetUserData(holder, user);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceCreatedEvent(callback, surface_created);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceChangedEvent(callback, surface_changed);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceDestroyedEvent(callback, surface_destroyed);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceShowEvent(callback, surface_shown);
            sys::OH_ArkUI_SurfaceCallback_SetSurfaceHideEvent(callback, surface_hidden);
            sys::OH_ArkUI_SurfaceHolder_AddSurfaceCallback(holder, callback);

            let provider = sys::OH_ArkUI_AccessibilityProvider_Create(handle);
            if !provider.is_null() {
                accessibility::attach(token, provider)?;
            }
            let code = sys::OH_ArkUI_NodeContent_AddNode(content, handle);
            if code != 0 {
                return Err(format!("UI_SURFACE: ContentSlot refused the node ({code})"));
            }
            Ok(Self {
                token,
                handle,
                content,
                texture: !is_surface,
                holder,
                callback,
                provider,
            })
        }
    }
}

/// Ask for one display frame for page `token`; any thread (the host asks from its worker).
pub(crate) fn request_frame(token: i64) {
    vsync::request(token);
}

/// One display-synchronized callback per frame for every page that asked. The vsync
/// callback runs on its own thread, so it posts the frame to the main thread.
mod vsync {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    static WANTED: Mutex<Vec<i64>> = Mutex::new(Vec::new());
    static PENDING: AtomicBool = AtomicBool::new(false);

    fn source() -> Option<*mut sys::OH_NativeVSync> {
        static VSYNC: OnceLock<usize> = OnceLock::new();
        let vsync = *VSYNC.get_or_init(|| {
            let name = c"lingxia-view";
            // SAFETY: a NUL-terminated name and its length.
            unsafe { sys::OH_NativeVSync_Create(name.as_ptr(), name.count_bytes() as u32) as usize }
        });
        (vsync != 0).then_some(vsync as *mut sys::OH_NativeVSync)
    }

    unsafe extern "C" fn fired(timestamp: i64, _data: *mut c_void) {
        PENDING.store(false, Ordering::Release);
        let mut period = 0i64;
        // SAFETY: the process-lifetime source and a live out-pointer.
        let known = source()
            .is_some_and(|vsync| unsafe { sys::OH_NativeVSync_GetPeriod(vsync, &mut period) == 0 });
        // The frame drawn for this vsync shows on the next one.
        let display = (known && period > 0).then_some((timestamp + period, period));
        crate::dispatch::post_main(Box::new(move || {
            let tokens = std::mem::take(&mut *WANTED.lock().unwrap_or_else(|p| p.into_inner()));
            for token in tokens {
                pages::with(token, |host| host.adapter().display_frame(host, display));
            }
        }));
    }

    pub(super) fn request(token: i64) {
        {
            let mut wanted = WANTED.lock().unwrap_or_else(|p| p.into_inner());
            if !wanted.contains(&token) {
                wanted.push(token);
            }
        }
        if PENDING.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(vsync) = source() else {
            PENDING.store(false, Ordering::Release);
            return log::warn!(target: "lingxia_view", "UI_VSYNC: no display sync");
        };
        // SAFETY: the vsync source lives for the process; the callback ignores its data.
        let code = unsafe { sys::OH_NativeVSync_RequestFrame(vsync, fired, std::ptr::null_mut()) };
        if code != 0 {
            PENDING.store(false, Ordering::Release);
            log::warn!(target: "lingxia_view", "UI_VSYNC: frame request refused ({code})");
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        focus::lost(self.token);
        let Ok(api) = api() else { return };
        // SAFETY: main thread; releases what `mount` created, in reverse.
        unsafe {
            if !self.provider.is_null() {
                sys::OH_ArkUI_AccessibilityProvider_Dispose(self.provider);
            }
            sys::OH_ArkUI_NodeContent_RemoveNode(self.content, self.handle);
            let events: &[i32] = if self.texture {
                &TEXTURE_EVENTS
            } else {
                &SURFACE_EVENTS
            };
            for &kind in events {
                (api.unregister_node_event)(self.handle, kind);
            }
            (api.remove_node_event_receiver)(self.handle, on_event);
            sys::OH_ArkUI_SurfaceHolder_Dispose(self.holder);
            sys::OH_ArkUI_SurfaceCallback_Dispose(self.callback);
            (api.dispose_node)(self.handle);
        }
    }
}

fn holder_token(holder: *mut sys::OH_ArkUI_SurfaceHolder) -> i64 {
    // SAFETY: user data set in `mount` to the page token.
    unsafe { sys::OH_ArkUI_SurfaceHolder_GetUserData(holder) as usize as i64 }
}

unsafe extern "C" fn surface_created(holder: *mut sys::OH_ArkUI_SurfaceHolder) {
    let token = holder_token(holder);
    let transparent = pages::with(token, |host| host.adapter().wants_texture()).unwrap_or(false);
    // SAFETY: ArkUI's live holder.
    let window = NativeWindow::retain(
        unsafe { sys::OH_ArkUI_XComponent_GetNativeWindow(holder) },
        transparent,
    );
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

unsafe extern "C" fn surface_shown(holder: *mut sys::OH_ArkUI_SurfaceHolder) {
    pages::with(holder_token(holder), |host| {
        host.adapter().surface_shown(host)
    });
}

unsafe extern "C" fn surface_hidden(holder: *mut sys::OH_ArkUI_SurfaceHolder) {
    pages::with(holder_token(holder), |host| {
        host.adapter().surface_hidden(host)
    });
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
        sys::NODE_ON_TOUCH_INTERCEPT if !input.is_null() => touch_intercept(host, token, input),
        sys::NODE_TOUCH_EVENT if !input.is_null() => touch(host, input),
        sys::NODE_ON_FOCUS => focus::gained(host, token),
        sys::NODE_ON_BLUR => {
            focus::lost(token);
            host.focus_left()
        }
        sys::NODE_ON_KEY_EVENT if !input.is_null() => key(host, token, input),
        sys::NODE_ON_MOUSE if !input.is_null() => mouse(host, input),
        sys::NODE_ON_HOVER if !hovering(event) => host.hover(None),
        _ => {}
    });
}

/// ArkUI focuses the node on a touch, on a page or tab switch and when the window returns, as
/// well as by Tab. Only a key walks focus into the page, and only the shell can tell which it was.
pub(crate) mod focus {
    use super::pages;
    use std::cell::Cell;

    thread_local! {
        /// The page whose node holds ArkUI focus; 0 when none does. Main thread only.
        static HOLDER: Cell<i64> = const { Cell::new(0) };
    }

    pub(super) fn gained(host: &pages::Host, token: i64) {
        HOLDER.set(token);
        host.adapter().shell.focus_gained(token);
    }

    pub(super) fn lost(token: i64) {
        if HOLDER.get() == token {
            HOLDER.set(0);
        }
    }

    /// The shell's answer to `Shell::focus_gained`: a key moved focus to the page, which still
    /// holds it.
    pub(crate) fn by_key(token: i64) {
        if HOLDER.get() == token {
            pages::with(token, |host| host.focus_entered(false));
        }
    }
}

/// Passes touches that land inside an uncovered `<webview>` hole directly to the sibling
/// `LingXiaWebView` underneath `ContentSlot`, while keeping View overlays and dialogs opaque.
fn touch_intercept(host: &pages::Host, token: i64, input: *mut sys::ArkUI_UIInputEvent) {
    // SAFETY: the live input event of this callback.
    let (x, y) = unsafe {
        (
            sys::OH_ArkUI_PointerEvent_GetX(input),
            sys::OH_ArkUI_PointerEvent_GetY(input),
        )
    };
    let scale = host.adapter().placement().0;
    let mode = if host.webview_at([x * scale, y * scale]).is_some() {
        crate::ime::disconnect(token);
        host.hover(None);
        sys::HTM_NONE
    } else {
        sys::HTM_DEFAULT
    };
    // SAFETY: as above.
    unsafe {
        sys::OH_ArkUI_PointerEvent_SetInterceptHitTestMode(input, mode);
    }
}

/// A mouse over the page drives `hover:`; presses still arrive as touches.
fn mouse(host: &pages::Host, input: *mut sys::ArkUI_UIInputEvent) {
    // SAFETY: the live mouse event of this callback.
    let (action, x, y) = unsafe {
        (
            sys::OH_ArkUI_MouseEvent_GetMouseAction(input),
            sys::OH_ArkUI_PointerEvent_GetX(input),
            sys::OH_ArkUI_PointerEvent_GetY(input),
        )
    };
    if action == sys::UI_MOUSE_EVENT_ACTION_CANCEL {
        host.hover(None);
        return;
    }
    let scale = host.adapter().placement().0;
    host.hover(Some([x * scale, y * scale]));
}

/// Whether a hover event says the pointer is over the node rather than leaving it.
fn hovering(event: *mut sys::ArkUI_NodeEvent) -> bool {
    // SAFETY: ArkUI's live event; a hover event carries a component event.
    unsafe {
        let component = sys::OH_ArkUI_NodeEvent_GetNodeComponentEvent(event);
        !component.is_null() && (*component).data[0].i32 != 0
    }
}

/// A hardware key the View uses; an unmapped key, Back among them, stays with ArkUI.
fn key(host: &pages::Host, page: i64, input: *mut sys::ArkUI_UIInputEvent) {
    let mut modifiers = 0u64;
    // SAFETY: the live key event of this callback.
    let (kind, code, unicode) = unsafe {
        sys::OH_ArkUI_UIInputEvent_GetModifierKeyStates(input, &mut modifiers);
        (
            sys::OH_ArkUI_KeyEvent_GetType(input),
            sys::OH_ArkUI_KeyEvent_GetKeyCode(input),
            sys::OH_ArkUI_KeyEvent_GetUnicode(input),
        )
    };
    let repeat = kind == sys::ARKUI_KEY_EVENT_LONG_PRESS;
    if kind != sys::ARKUI_KEY_EVENT_DOWN && !repeat {
        return;
    }
    let shift = modifiers & sys::ARKUI_MODIFIER_KEY_SHIFT != 0;
    let Some(view_key) = view_key(code, unicode, shift) else {
        return;
    };
    // An open editor takes typing through the input method; Tab still moves focus. A key its
    // page listed in `onKeyDown` goes to the page first; Space is text there, not Enter.
    if view_key != Key::Tab && crate::ime::editing(page) {
        if !shift && code != 2050 && host.editor_key(view_key) {
            // SAFETY: as above.
            unsafe { sys::OH_ArkUI_KeyEvent_SetConsumed(input, true) };
        }
        return;
    }
    if host.key(view_key, shift, repeat) {
        // SAFETY: as above.
        unsafe { sys::OH_ArkUI_KeyEvent_SetConsumed(input, true) };
    }
}

/// ArkUI key codes to the View's keys. A key event can carry no character, so letters and
/// digits also map from their codes.
fn view_key(code: i32, unicode: u32, shift: bool) -> Option<Key> {
    Some(match code {
        2049 => Key::Tab,
        2012 => Key::Up,
        2013 => Key::Down,
        2014 => Key::Left,
        2015 => Key::Right,
        2081 => Key::Home,
        2082 => Key::End,
        2016 | 2050 | 2054 | 2119 => Key::Activate,
        2070 => Key::Escape,
        2068 => Key::PageUp,
        2069 => Key::PageDown,
        _ => Key::Char(match char::from_u32(unicode).filter(|c| !c.is_control()) {
            Some(c) => c,
            None => match code {
                2000..=2009 => char::from(b'0' + (code - 2000) as u8),
                2017..=2042 => {
                    let c = char::from(b'a' + (code - 2017) as u8);
                    if shift { c.to_ascii_uppercase() } else { c }
                }
                _ => return None,
            },
        }),
    })
}

/// The clock vsync timestamps and input event times use.
pub(crate) fn monotonic_ns() -> i64 {
    let mut now = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a live out-pointer.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) };
    now.tv_sec * 1_000_000_000 + now.tv_nsec
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
    // Node events report virtual pixels; the host works in device pixels.
    let scale = adapter.placement().0;
    let (x, y) = (x * scale, y * scale);
    let phase = match action {
        sys::UI_TOUCH_EVENT_ACTION_DOWN => PointerPhase::Down,
        sys::UI_TOUCH_EVENT_ACTION_MOVE => PointerPhase::Move,
        sys::UI_TOUCH_EVENT_ACTION_UP => PointerPhase::Up,
        _ => PointerPhase::Cancel,
    };
    if matches!(phase, PointerPhase::Down) {
        adapter.begin_gesture();
        // A finger on a device that also has a mouse leaves no hover behind.
        // SAFETY: as above.
        if unsafe { sys::OH_ArkUI_UIInputEvent_GetToolType(input) }
            == sys::UI_INPUT_EVENT_TOOL_TYPE_FINGER
        {
            host.hover(None);
        }
    }
    let velocity = {
        let mut tracker = adapter.velocity();
        // SAFETY: history indices below the reported size.
        unsafe {
            for i in 0..sys::OH_ArkUI_PointerEvent_GetHistorySize(input) {
                tracker.add(
                    sys::OH_ArkUI_PointerEvent_GetHistoryEventTime(input, i),
                    sys::OH_ArkUI_PointerEvent_GetHistoryX(input, 0, i) * scale,
                    sys::OH_ArkUI_PointerEvent_GetHistoryY(input, 0, i) * scale,
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
    host.input_age(u64::try_from((monotonic_ns() - time) / 1000).unwrap_or(0));
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
