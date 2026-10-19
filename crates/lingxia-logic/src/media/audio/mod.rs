//! `lx.audio`: Logic-owned playback over the host audio coordinator.

mod player;
mod sound;
mod source;

use crate::i18n::js_error_with_reason;
use lingxia_media::audio::{
    AudioError, AudioRuntime, HostEvent, Interruption, OwnerId, Route, RouteReason,
};
use lingxia_platform::traits::audio::{AudioBackend, AudioDeviceCapabilities};
use lxapp::LxApp;
use rong::{
    JSContext, JSContextService, JSFunc, JSObject, JSResult, JSValue, Promise, RongJSError,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

type Listeners = Rc<RefCell<Vec<Option<JSFunc>>>>;

/// One Logic context's registration with the coordinator. Shutting the
/// context down disposes everything it created.
struct AudioContextService {
    handles: AudioHandles,
    route_listeners: Listeners,
    interruption_listeners: Listeners,
    host_pump: RefCell<Option<Promise>>,
}

#[derive(Clone)]
struct AudioHandles {
    runtime: &'static AudioRuntime,
    owner: OwnerId,
    app: Arc<LxApp>,
}

impl AudioContextService {
    fn get(ctx: &JSContext) -> JSResult<AudioHandles> {
        ctx.get_service::<AudioContextService>()
            .map(|service| service.handles.clone())
            .ok_or_else(|| crate::i18n::js_internal_error("lx.audio is not available"))
    }
}

impl JSContextService for AudioContextService {
    fn on_shutdown(&self) {
        self.handles.runtime.shutdown_owner(self.handles.owner);
        self.host_pump.borrow_mut().take();
    }
}

fn runtime_for(app: &Arc<LxApp>) -> Option<&'static AudioRuntime> {
    if !lingxia_media::audio::available() {
        return None;
    }
    let backend: Arc<dyn AudioBackend> = app.runtime.clone();
    lingxia_media::audio::start(backend)
        .inspect_err(|error| log::warn!("lx.audio unavailable: {error}"))
        .ok()
}

/// Output hardware for `lx.supports`, or `None` without a backend.
pub(crate) fn device_capabilities(app: &Arc<LxApp>) -> Option<AudioDeviceCapabilities> {
    runtime_for(app).map(AudioRuntime::capabilities)
}

/// The lxapp's onShow/onHide: foreground players pause with it.
pub(crate) fn on_visibility(ctx: &JSContext, visible: bool) {
    if let Some(service) = ctx.get_service::<AudioContextService>() {
        service
            .handles
            .runtime
            .set_owner_visible(service.handles.owner, visible);
    }
}

pub(super) fn audio_js_error(error: &AudioError) -> RongJSError {
    js_error_with_reason(error.code.biz_code(), error.code.as_str(), &error.message)
}

/// Run `on_abort` once when `signal` aborts (now, if it already has).
fn bind_abort(
    ctx: &JSContext,
    signal: Option<JSObject>,
    on_abort: impl FnOnce() + 'static,
) -> JSResult<()> {
    let Some(signal) = signal else {
        return Ok(());
    };
    if signal.get::<_, bool>("aborted").unwrap_or(false) {
        on_abort();
        return Ok(());
    }
    let callback = JSFunc::new_once(ctx, move || -> JSResult<()> {
        on_abort();
        Ok(())
    })?;
    let options = JSObject::new(ctx);
    options.set("once", true)?;
    let add: JSFunc = signal.get("addEventListener")?;
    add.call::<_, ()>(Some(signal.clone()), ("abort", callback, options))
}

fn route_object(ctx: &JSContext, route: &Route) -> JSResult<JSObject> {
    let object = JSObject::new(ctx);
    object.set("output", route.output.as_str())?;
    if let Some(name) = &route.name {
        object.set("name", name.as_str())?;
    }
    Ok(object)
}

fn route_change_object(ctx: &JSContext, route: &Route, reason: RouteReason) -> JSResult<JSObject> {
    let object = JSObject::new(ctx);
    object.set("route", route_object(ctx, route)?)?;
    object.set("reason", reason.as_str())?;
    Ok(object)
}

fn interruption_object(ctx: &JSContext, interruption: &Interruption) -> JSResult<JSObject> {
    let object = JSObject::new(ctx);
    object.set("phase", interruption.phase.as_str())?;
    object.set("reason", interruption.reason.as_str())?;
    object.set("shouldResume", interruption.should_resume)?;
    object.set("resumed", interruption.resumed)?;
    Ok(object)
}

fn subscribe(ctx: &JSContext, listeners: &Listeners, listener: JSValue) -> JSResult<JSFunc> {
    let listener = listener
        .to_rust::<JSFunc>()
        .map_err(|_| audio_js_error(&source::invalid_options("listener must be a function")))?;
    let slot = {
        let mut slots = listeners.borrow_mut();
        slots.push(Some(listener));
        slots.len() - 1
    };
    let listeners = listeners.clone();
    JSFunc::new(ctx, move || -> JSResult<()> {
        if let Some(entry) = listeners.borrow_mut().get_mut(slot) {
            *entry = None;
        }
        Ok(())
    })
}

fn notify(listeners: &Listeners, payload: &JSObject) {
    let snapshot: Vec<JSFunc> = listeners.borrow().iter().flatten().cloned().collect();
    for listener in snapshot {
        let _ = listener.call::<_, JSValue>(None, (payload.clone(),));
    }
}

fn service_listeners(ctx: &JSContext) -> JSResult<(Listeners, Listeners, &'static AudioRuntime)> {
    let service = ctx
        .get_service::<AudioContextService>()
        .ok_or_else(|| crate::i18n::js_internal_error("lx.audio is not available"))?;
    Ok((
        service.route_listeners.clone(),
        service.interruption_listeners.clone(),
        service.handles.runtime,
    ))
}

/// Where audio plays right now.
fn route_get(ctx: JSContext) -> JSResult<JSObject> {
    let (_, _, runtime) = service_listeners(&ctx)?;
    route_object(&ctx, &runtime.route())
}

/// Called with the current route before this returns, then on each change.
fn route_watch(ctx: JSContext, listener: JSValue) -> JSResult<JSFunc> {
    let (routes, _, runtime) = service_listeners(&ctx)?;
    let unsubscribe = subscribe(&ctx, &routes, listener.clone())?;
    let first = route_change_object(&ctx, &runtime.route(), RouteReason::Other)?;
    if let Ok(listener) = listener.to_rust::<JSFunc>() {
        listener.call::<_, JSValue>(None, (first,))?;
    }
    Ok(unsubscribe)
}

/// External interruptions (calls, alarms, other apps) for the whole host.
fn on_interruption(ctx: JSContext, listener: JSValue) -> JSResult<JSFunc> {
    let (_, interruptions, _) = service_listeners(&ctx)?;
    subscribe(&ctx, &interruptions, listener)
}

/// A long-form player: one stream at a time over a native queue. Foreground
/// only: it pauses when the lxapp hides.
fn create_player(
    ctx: JSContext,
    options: rong::function::Optional<JSObject>,
) -> JSResult<JSObject> {
    player::create_player(ctx, options)
}

/// Preload a short effect for low-latency, polyphonic playback.
fn load_sound(
    ctx: JSContext,
    source: JSValue,
    options: rong::function::Optional<JSObject>,
) -> JSResult<Promise> {
    sound::load_sound(ctx, source, options)
}

fn audio_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    if let Ok(audio) = lx.get::<_, JSObject>("audio") {
        return Ok(audio);
    }
    let audio = JSObject::new(ctx);
    lx.set("audio", audio.clone())?;
    Ok(audio)
}

fn route_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let audio = audio_namespace(ctx)?;
    if let Ok(route) = audio.get::<_, JSObject>("route") {
        return Ok(route);
    }
    let route = JSObject::new(ctx);
    audio.set("route", route.clone())?;
    Ok(route)
}

/// The namespace objects are plain; `AudioApi` / `AudioRouteApi` in the
/// public types are their only declaration.
fn register_api(ctx: &JSContext) -> JSResult<()> {
    let audio = audio_namespace(ctx)?;
    audio.set(
        "createPlayer",
        JSFunc::new(ctx, create_player)?.name("createPlayer")?,
    )?;
    audio.set(
        "loadSound",
        JSFunc::new(ctx, load_sound)?.name("loadSound")?,
    )?;
    audio.set(
        "onInterruption",
        JSFunc::new(ctx, on_interruption)?.name("onInterruption")?,
    )?;
    let route = route_namespace(ctx)?;
    route.set("get", JSFunc::new(ctx, route_get)?.name("get")?)?;
    route.set("watch", JSFunc::new(ctx, route_watch)?.name("watch")?)?;
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    if !crate::capability::exposes(ctx, "audio") {
        return Ok(());
    }
    let app = LxApp::from_ctx(ctx)?;
    let Some(runtime) = runtime_for(&app) else {
        return Ok(());
    };
    let (owner, mut host_events) = runtime.register_owner();
    let route_listeners: Listeners = Rc::new(RefCell::new(Vec::new()));
    let interruption_listeners: Listeners = Rc::new(RefCell::new(Vec::new()));

    let routes = route_listeners.clone();
    let interruptions = interruption_listeners.clone();
    let pump_ctx = ctx.clone();
    let host_pump = Promise::from_future(ctx, None, async move {
        while let Some(event) = host_events.recv().await {
            match event {
                HostEvent::Route(change) => {
                    let payload = route_change_object(&pump_ctx, &change.route, change.reason)?;
                    notify(&routes, &payload);
                }
                HostEvent::Interruption(interruption) => {
                    let payload = interruption_object(&pump_ctx, &interruption)?;
                    payload.set("scope", "host")?;
                    notify(&interruptions, &payload);
                }
            }
        }
        Ok::<(), RongJSError>(())
    })?;
    ctx.set_service(AudioContextService {
        handles: AudioHandles {
            runtime,
            owner,
            app,
        },
        route_listeners,
        interruption_listeners,
        host_pump: RefCell::new(Some(host_pump)),
    });
    register_api(ctx)
}
