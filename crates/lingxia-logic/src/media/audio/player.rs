//! `lx.audio.createPlayer`: a JS handle over one coordinator player.

use super::source::{invalid_options, optional_string, resolve_player_source};
use super::{AudioContextService, audio_js_error, bind_abort, interruption_object, route_object};
use futures::channel::oneshot;
use lingxia_media::audio::{
    AudioError, AudioRuntime, Focus, OutputRequest, PlayerCommand, PlayerEvent, PlayerOptions,
    PlayerSnapshot, QueueEntry, Repeat, Reply,
};
use lxapp::LxApp;
use rong::function::Optional;
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue, Promise, PropertyDescriptor};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const EVENTS: &[&str] = &[
    "state",
    "time",
    "item",
    "ended",
    "error",
    "interruption",
    "route",
];

type Pending = oneshot::Sender<Result<Reply, AudioError>>;

struct PlayerCore {
    runtime: &'static AudioRuntime,
    app: Arc<LxApp>,
    id: u64,
    shared: Arc<Mutex<PlayerSnapshot>>,
    queue: RefCell<Vec<JSValue>>,
    listeners: RefCell<HashMap<&'static str, Vec<Option<JSFunc>>>>,
    pending: RefCell<HashMap<u64, Pending>>,
    next_req: Cell<u64>,
    disposed: Cell<bool>,
}

impl PlayerCore {
    fn snapshot(&self) -> PlayerSnapshot {
        self.shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn send(&self, command: PlayerCommand) -> oneshot::Receiver<Result<Reply, AudioError>> {
        let (tx, rx) = oneshot::channel();
        if self.disposed.get() {
            let _ = tx.send(Err(AudioError::disposed()));
            return rx;
        }
        let req = self.next_req.get();
        self.next_req.set(req + 1);
        self.pending.borrow_mut().insert(req, tx);
        self.runtime.player_command(self.id, req, command);
        rx
    }

    /// Fire-and-forget: the coordinator still emits `state`, nobody awaits.
    fn post(&self, command: PlayerCommand) {
        if !self.disposed.get() {
            self.runtime.player_command(self.id, 0, command);
        }
    }

    fn emit(&self, event: &'static str, payload: JSValue) {
        let listeners: Vec<JSFunc> = self
            .listeners
            .borrow()
            .get(event)
            .map(|slots| slots.iter().flatten().cloned().collect())
            .unwrap_or_default();
        for listener in listeners {
            // A throwing listener must not starve the others.
            let _ = listener.call::<_, JSValue>(None, (payload.clone(),));
        }
    }

    fn item(&self, index: Option<usize>) -> Option<JSValue> {
        index.and_then(|index| self.queue.borrow().get(index).cloned())
    }

    fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        self.runtime
            .player_command(self.id, 0, PlayerCommand::Dispose);
    }
}

fn state_object(ctx: &JSContext, core: &PlayerCore, snap: &PlayerSnapshot) -> JSResult<JSObject> {
    let state = JSObject::new(ctx);
    state.set("status", snap.status.as_str())?;
    state.set("buffering", snap.buffering)?;
    state.set("seeking", snap.seeking)?;
    state.set("positionMs", snap.position_now() as f64)?;
    match snap.duration_ms {
        Some(duration) => state.set("durationMs", duration as f64)?,
        None => state.set("durationMs", JSValue::null(ctx))?,
    };
    state.set("bufferedMs", snap.buffered_ms as f64)?;
    match snap.index {
        Some(index) => state.set("index", index as f64)?,
        None => state.set("index", JSValue::null(ctx))?,
    };
    match core.item(snap.index) {
        Some(item) => state.set("item", item)?,
        None => state.set("item", JSValue::null(ctx))?,
    };
    state.set("volume", snap.volume)?;
    state.set("muted", snap.muted)?;
    state.set("rate", snap.rate)?;
    state.set("repeat", snap.repeat.as_str())?;
    state.set("output", snap.output.as_str())?;
    state.set("route", route_object(ctx, &snap.route)?)?;
    match &snap.error {
        Some(error) => {
            let value = JSObject::new(ctx);
            value.set("code", error.code.as_str())?;
            value.set("message", error.message.as_str())?;
            state.set("error", value)?
        }
        None => state.set("error", JSValue::null(ctx))?,
    };
    Ok(state)
}

fn read_bool(options: &JSObject, key: &str) -> Result<Option<bool>, AudioError> {
    match options.get::<_, JSValue>(key) {
        Ok(value) if value.is_undefined() || value.is_null() => Ok(None),
        Ok(value) if value.is_boolean() => Ok(value.to_rust::<bool>().ok()),
        Ok(_) => Err(invalid_options(format!("{key} must be a boolean"))),
        Err(_) => Ok(None),
    }
}

fn read_number(options: &JSObject, key: &str) -> Result<Option<f64>, AudioError> {
    match options.get::<_, JSValue>(key) {
        Ok(value) if value.is_undefined() || value.is_null() => Ok(None),
        Ok(value) if value.is_number() => value
            .to_rust::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(Some)
            .ok_or_else(|| invalid_options(format!("{key} must be a finite number"))),
        Ok(_) => Err(invalid_options(format!("{key} must be a number"))),
        Err(_) => Ok(None),
    }
}

pub(super) fn parse_focus(value: Option<String>, default: Focus) -> Result<Focus, AudioError> {
    match value.as_deref() {
        None => Ok(default),
        Some("exclusive") => Ok(Focus::Exclusive),
        Some("duck") => Ok(Focus::Duck),
        Some("mix") => Ok(Focus::Mix),
        Some(other) => Err(invalid_options(format!(
            "focus must be 'exclusive', 'duck', or 'mix', got '{other}'"
        ))),
    }
}

fn parse_output(value: &str) -> Result<OutputRequest, AudioError> {
    match value {
        "auto" => Ok(OutputRequest::Auto),
        "speaker" => Ok(OutputRequest::Speaker),
        "earpiece" => Ok(OutputRequest::Earpiece),
        other => Err(invalid_options(format!(
            "output must be 'auto', 'speaker', or 'earpiece', got '{other}'"
        ))),
    }
}

fn parse_repeat(value: &str) -> Result<Repeat, AudioError> {
    match value {
        "off" => Ok(Repeat::Off),
        "one" => Ok(Repeat::One),
        "all" => Ok(Repeat::All),
        other => Err(invalid_options(format!(
            "repeat must be 'off', 'one', or 'all', got '{other}'"
        ))),
    }
}

fn check_volume(volume: f64) -> Result<f64, AudioError> {
    if (0.0..=1.0).contains(&volume) {
        Ok(volume)
    } else {
        Err(invalid_options("volume must be between 0 and 1"))
    }
}

fn check_rate(rate: f64) -> Result<f64, AudioError> {
    if (0.5..=2.0).contains(&rate) {
        Ok(rate)
    } else {
        Err(invalid_options("rate must be between 0.5 and 2"))
    }
}

fn parse_options(
    options: Option<&JSObject>,
) -> Result<(PlayerOptions, Option<JSObject>), AudioError> {
    let mut parsed = PlayerOptions::default();
    let Some(options) = options else {
        return Ok((parsed, None));
    };
    if read_bool(options, "background")? == Some(true) {
        return Err(AudioError::new(
            lingxia_media::audio::AudioErrorCode::BackgroundUnavailable,
            "background playback is not available in this runtime",
        ));
    }
    parsed.focus = parse_focus(optional_string(options, "focus")?, Focus::Exclusive)?;
    parsed.obey_silent_switch =
        read_bool(options, "obeySilentSwitch")?.unwrap_or(parsed.focus == Focus::Mix);
    if let Some(output) = optional_string(options, "output")? {
        parsed.output = parse_output(&output)?;
    }
    if parsed.output == OutputRequest::Earpiece && parsed.focus == Focus::Mix {
        return Err(invalid_options("a mix player cannot use the earpiece"));
    }
    parsed.proximity = read_bool(options, "proximity")?.unwrap_or(false);
    parsed.resume_after_interruption =
        read_bool(options, "resumeAfterInterruption")?.unwrap_or(false);
    if let Some(volume) = read_number(options, "volume")? {
        parsed.volume = check_volume(volume)?;
    }
    if let Some(rate) = read_number(options, "rate")? {
        parsed.rate = check_rate(rate)?;
    }
    if let Some(repeat) = optional_string(options, "repeat")? {
        parsed.repeat = parse_repeat(&repeat)?;
    }
    if let Some(interval) = read_number(options, "timeUpdateIntervalMs")? {
        if interval < 0.0 {
            return Err(invalid_options("timeUpdateIntervalMs must be >= 0"));
        }
        parsed.time_update_interval_ms = interval as u32;
    }
    let signal = match options.get::<_, JSValue>("signal") {
        Ok(value) if !value.is_undefined() && !value.is_null() => Some(
            value
                .into_object()
                .ok_or_else(|| invalid_options("signal must be an AbortSignal"))?,
        ),
        _ => None,
    };
    Ok((parsed, signal))
}

pub(super) fn create_player(ctx: JSContext, options: Optional<JSObject>) -> JSResult<JSObject> {
    let service = AudioContextService::get(&ctx)?;
    let (options, signal) = parse_options(options.0.as_ref()).map_err(|e| audio_js_error(&e))?;
    crate::task_object::validate_abort_signal(signal.as_ref())?;
    let handle = service.runtime.create_player(service.owner, options);
    let core = Rc::new(PlayerCore {
        runtime: service.runtime,
        app: service.app.clone(),
        id: handle.id,
        shared: handle.shared,
        queue: RefCell::new(Vec::new()),
        listeners: RefCell::new(HashMap::new()),
        pending: RefCell::new(HashMap::new()),
        next_req: Cell::new(1),
        disposed: Cell::new(false),
    });

    let player = JSObject::new(&ctx);
    install_members(&ctx, &player, &core)?;

    let mut events = handle.events;
    let pump_core = core.clone();
    let pump_ctx = ctx.clone();
    let pump = Promise::from_future(&ctx, None, async move {
        while let Some(event) = events.recv().await {
            dispatch(&pump_ctx, &pump_core, event)?;
        }
        // The coordinator dropped the player: nothing is left to answer.
        for (_, tx) in pump_core.pending.borrow_mut().drain() {
            let _ = tx.send(Err(AudioError::disposed()));
        }
        Ok::<(), rong::RongJSError>(())
    })?;
    // An unreferenced Promise is collected and its future cancelled.
    player.define_property(
        "__lxAudioPump",
        PropertyDescriptor::from_value(pump.into_object().into_js_value()),
    )?;

    let abort_core = core.clone();
    bind_abort(&ctx, signal, move || abort_core.dispose())?;
    Ok(player)
}

fn dispatch(ctx: &JSContext, core: &PlayerCore, event: PlayerEvent) -> JSResult<()> {
    match event {
        PlayerEvent::State(snap) => {
            let state = state_object(ctx, core, &snap)?;
            core.emit("state", state.into_js_value());
        }
        PlayerEvent::Time {
            position_ms,
            duration_ms,
        } => {
            let payload = JSObject::new(ctx);
            payload.set("positionMs", position_ms as f64)?;
            match duration_ms {
                Some(duration) => payload.set("durationMs", duration as f64)?,
                None => payload.set("durationMs", JSValue::null(ctx))?,
            };
            core.emit("time", payload.into_js_value());
        }
        PlayerEvent::Item { index, reason } => {
            let payload = JSObject::new(ctx);
            payload.set("index", index as f64)?;
            match core.item(Some(index)) {
                Some(item) => payload.set("item", item)?,
                None => payload.set("item", JSValue::null(ctx))?,
            };
            payload.set("reason", reason.as_str())?;
            core.emit("item", payload.into_js_value());
        }
        PlayerEvent::Ended { index } => {
            let payload = JSObject::new(ctx);
            payload.set("index", index as f64)?;
            core.emit("ended", payload.into_js_value());
        }
        PlayerEvent::Error { error, index } => {
            let payload = JSObject::new(ctx);
            payload.set("code", error.code.as_str())?;
            payload.set("message", error.message.as_str())?;
            match index {
                Some(index) => payload.set("index", index as f64)?,
                None => payload.set("index", JSValue::null(ctx))?,
            };
            core.emit("error", payload.into_js_value());
        }
        PlayerEvent::Interruption(interruption) => {
            let payload = interruption_object(ctx, &interruption)?;
            core.emit("interruption", payload.into_js_value());
        }
        PlayerEvent::Route(change) => {
            let payload = JSObject::new(ctx);
            payload.set("route", route_object(ctx, &change.route)?)?;
            payload.set("reason", change.reason.as_str())?;
            core.emit("route", payload.into_js_value());
        }
        PlayerEvent::Reply { req, result } => {
            if let Some(tx) = core.pending.borrow_mut().remove(&req) {
                let _ = tx.send(result);
            }
        }
    }
    Ok(())
}

fn unit_promise(
    ctx: &JSContext,
    rx: oneshot::Receiver<Result<Reply, AudioError>>,
) -> JSResult<Promise> {
    Promise::from_future(ctx, None, async move {
        match rx.await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(audio_js_error(&error)),
            Err(_) => Err(audio_js_error(&AudioError::disposed())),
        }
    })
}

fn loaded_promise(
    ctx: &JSContext,
    rx: oneshot::Receiver<Result<Reply, AudioError>>,
) -> JSResult<Promise> {
    let ctx_for_result = ctx.clone();
    Promise::from_future(ctx, None, async move {
        match rx.await {
            Ok(Ok(Reply::Loaded { duration_ms, index })) => {
                let info = JSObject::new(&ctx_for_result);
                match duration_ms {
                    Some(duration) => info.set("durationMs", duration as f64)?,
                    None => info.set("durationMs", JSValue::null(&ctx_for_result))?,
                };
                info.set("index", index as f64)?;
                Ok(info)
            }
            Ok(Ok(Reply::Done)) => Ok(JSObject::new(&ctx_for_result)),
            Ok(Err(error)) => Err(audio_js_error(&error)),
            Err(_) => Err(audio_js_error(&AudioError::disposed())),
        }
    })
}

fn rejected(ctx: &JSContext, error: AudioError) -> JSResult<Promise> {
    Promise::from_future(
        ctx,
        None,
        async move { Err::<(), _>(audio_js_error(&error)) },
    )
}

/// `QueueItem` → (`item` kept verbatim for echoes, resolved entry).
fn queue_item(
    app: &LxApp,
    value: JSValue,
    is_item: bool,
) -> Result<(JSValue, QueueEntry), AudioError> {
    if !is_item {
        let entry = resolve_player_source(app, &value, 0)?;
        let ctx = value.context();
        let item = JSObject::new(&ctx);
        item.set("source", value)
            .map_err(|_| invalid_options("source could not be stored"))?;
        return Ok((item.into_js_value(), entry));
    }
    let object = value
        .clone()
        .into_object()
        .ok_or_else(|| invalid_options("queue items must be objects with a source"))?;
    let source = object
        .get::<_, JSValue>("source")
        .map_err(|_| invalid_options("queue item needs a source"))?;
    let start_ms = match read_number(&object, "startMs")? {
        Some(ms) if ms < 0.0 => return Err(invalid_options("startMs must be >= 0")),
        Some(ms) => ms as u64,
        None => 0,
    };
    let entry = resolve_player_source(app, &source, start_ms)?;
    Ok((value, entry))
}

fn looks_like_item(value: &JSValue) -> bool {
    value
        .clone()
        .into_object()
        .is_some_and(|object| object.has_property("source").unwrap_or(false))
}

fn read_autoplay(options: &Optional<JSObject>) -> Result<(bool, Option<f64>), AudioError> {
    match options.0.as_ref() {
        Some(options) => Ok((
            read_bool(options, "autoplay")?.unwrap_or(false),
            read_number(options, "startIndex")?,
        )),
        None => Ok((false, None)),
    }
}

macro_rules! method {
    ($ctx:expr, $object:expr, $name:expr, $f:expr) => {
        $object.set($name, JSFunc::new($ctx, $f)?.name($name)?)?
    };
}
pub(super) use method;

fn install_members(ctx: &JSContext, player: &JSObject, core: &Rc<PlayerCore>) -> JSResult<()> {
    let c = core.clone();
    let getter = JSFunc::new(ctx, move |ctx: JSContext| -> JSResult<JSObject> {
        state_object(&ctx, &c, &c.snapshot())
    })?;
    player.define_property(
        "state",
        PropertyDescriptor::from_getter(getter).enumerable(),
    )?;

    let c = core.clone();
    let getter = JSFunc::new(ctx, move |ctx: JSContext| -> JSResult<rong::JSArray> {
        let array = rong::JSArray::new(&ctx)?;
        for item in c.queue.borrow().iter() {
            array.push(item.clone())?;
        }
        Ok(array)
    })?;
    player.define_property(
        "queue",
        PropertyDescriptor::from_getter(getter).enumerable(),
    )?;

    let c = core.clone();
    method!(ctx, player, "load", move |ctx: JSContext,
                                       source: JSValue,
                                       options: Optional<JSObject>|
          -> JSResult<Promise> {
        let prepared = read_autoplay(&options).and_then(|(autoplay, _)| {
            let is_item = looks_like_item(&source);
            queue_item(&c.app, source, is_item).map(|item| (autoplay, item))
        });
        let (autoplay, (item, entry)) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return rejected(&ctx, error),
        };
        if c.disposed.get() {
            return rejected(&ctx, AudioError::disposed());
        }
        *c.queue.borrow_mut() = vec![item];
        let rx = c.send(PlayerCommand::SetQueue {
            items: vec![entry],
            start_index: 0,
            autoplay,
        });
        loaded_promise(&ctx, rx)
    });

    let c = core.clone();
    method!(ctx, player, "setQueue", move |ctx: JSContext,
                                           items: JSValue,
                                           options: Optional<
        JSObject,
    >|
          -> JSResult<Promise> {
        let prepared = (|| -> Result<_, AudioError> {
            let (autoplay, start) = read_autoplay(&options)?;
            let array = items
                .into_object()
                .filter(|o| o.get::<_, f64>("length").is_ok())
                .ok_or_else(|| invalid_options("setQueue takes an array of queue items"))?;
            let length = array.get::<_, f64>("length").unwrap_or(0.0) as usize;
            if length == 0 {
                return Err(invalid_options("the queue cannot be empty"));
            }
            let mut kept = Vec::with_capacity(length);
            let mut entries = Vec::with_capacity(length);
            for index in 0..length {
                let value = array
                    .get::<_, JSValue>(index as u32)
                    .map_err(|_| invalid_options("queue item is missing"))?;
                let (item, entry) = queue_item(&c.app, value, true)?;
                kept.push(item);
                entries.push(entry);
            }
            let start = match start {
                Some(start) if start < 0.0 || start.fract() != 0.0 || start as usize >= length => {
                    return Err(invalid_options("startIndex is outside the queue"));
                }
                Some(start) => start as usize,
                None => 0,
            };
            Ok((kept, entries, start, autoplay))
        })();
        let (kept, entries, start_index, autoplay) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return rejected(&ctx, error),
        };
        if c.disposed.get() {
            return rejected(&ctx, AudioError::disposed());
        }
        *c.queue.borrow_mut() = kept;
        let rx = c.send(PlayerCommand::SetQueue {
            items: entries,
            start_index,
            autoplay,
        });
        loaded_promise(&ctx, rx)
    });

    for (name, command) in [
        ("play", PlayerCommand::Play),
        ("pause", PlayerCommand::Pause),
        ("stop", PlayerCommand::Stop),
        ("next", PlayerCommand::Next),
        ("previous", PlayerCommand::Previous),
    ] {
        let c = core.clone();
        method!(
            ctx,
            player,
            name,
            move |ctx: JSContext| -> JSResult<Promise> {
                unit_promise(&ctx, c.send(command.clone()))
            }
        );
    }

    let c = core.clone();
    method!(ctx, player, "seek", move |ctx: JSContext,
                                       position: JSValue|
          -> JSResult<Promise> {
        let position = position
            .to_rust::<f64>()
            .ok()
            .filter(|p| p.is_finite() && *p >= 0.0);
        match position {
            Some(position) => unit_promise(&ctx, c.send(PlayerCommand::Seek(position as u64))),
            None => rejected(&ctx, invalid_options("seek takes a position in ms >= 0")),
        }
    });

    let c = core.clone();
    method!(ctx, player, "skipTo", move |ctx: JSContext,
                                         index: JSValue|
          -> JSResult<Promise> {
        let index = index
            .to_rust::<f64>()
            .ok()
            .filter(|i| i.is_finite() && *i >= 0.0 && i.fract() == 0.0);
        match index {
            Some(index) => unit_promise(&ctx, c.send(PlayerCommand::SkipTo(index as usize))),
            None => rejected(&ctx, invalid_options("skipTo takes a queue index")),
        }
    });

    let c = core.clone();
    method!(
        ctx,
        player,
        "setVolume",
        move |volume: JSValue| -> JSResult<()> {
            let volume = volume
                .to_rust::<f64>()
                .ok()
                .ok_or_else(|| invalid_options("volume must be a number"))
                .and_then(check_volume)
                .map_err(|e| audio_js_error(&e))?;
            c.shared.lock().unwrap_or_else(|e| e.into_inner()).volume = volume;
            c.post(PlayerCommand::ApplySettings);
            Ok(())
        }
    );

    let c = core.clone();
    method!(
        ctx,
        player,
        "setMuted",
        move |muted: JSValue| -> JSResult<()> {
            let muted = muted
                .is_boolean()
                .then(|| muted.to_rust::<bool>().ok())
                .flatten()
                .ok_or_else(|| audio_js_error(&invalid_options("muted must be a boolean")))?;
            c.shared.lock().unwrap_or_else(|e| e.into_inner()).muted = muted;
            c.post(PlayerCommand::ApplySettings);
            Ok(())
        }
    );

    let c = core.clone();
    method!(
        ctx,
        player,
        "setRate",
        move |rate: JSValue| -> JSResult<()> {
            let rate = rate
                .to_rust::<f64>()
                .ok()
                .ok_or_else(|| invalid_options("rate must be a number"))
                .and_then(check_rate)
                .map_err(|e| audio_js_error(&e))?;
            c.shared.lock().unwrap_or_else(|e| e.into_inner()).rate = rate;
            c.post(PlayerCommand::ApplySettings);
            Ok(())
        }
    );

    let c = core.clone();
    method!(
        ctx,
        player,
        "setRepeat",
        move |repeat: JSValue| -> JSResult<()> {
            let repeat = repeat
                .to_rust::<String>()
                .ok()
                .ok_or_else(|| invalid_options("repeat must be a string"))
                .and_then(|r| parse_repeat(&r))
                .map_err(|e| audio_js_error(&e))?;
            c.shared.lock().unwrap_or_else(|e| e.into_inner()).repeat = repeat;
            c.post(PlayerCommand::ApplySettings);
            Ok(())
        }
    );

    let c = core.clone();
    method!(ctx, player, "setOutput", move |ctx: JSContext,
                                            output: JSValue|
          -> JSResult<Promise> {
        let output = output
            .to_rust::<String>()
            .ok()
            .ok_or_else(|| invalid_options("output must be a string"))
            .and_then(|o| parse_output(&o));
        match output {
            Ok(output) => unit_promise(&ctx, c.send(PlayerCommand::SetOutput(output))),
            Err(error) => rejected(&ctx, error),
        }
    });

    let c = core.clone();
    method!(ctx, player, "on", move |ctx: JSContext,
                                     event: JSValue,
                                     listener: JSValue|
          -> JSResult<JSFunc> {
        let name = event.to_rust::<String>().ok();
        let Some(event) = name.and_then(|name| EVENTS.iter().copied().find(|e| *e == name)) else {
            return Err(audio_js_error(&invalid_options(format!(
                "event must be one of {}",
                EVENTS.join(", ")
            ))));
        };
        let listener = listener
            .to_rust::<JSFunc>()
            .map_err(|_| audio_js_error(&invalid_options("listener must be a function")))?;
        let slot = {
            let mut listeners = c.listeners.borrow_mut();
            let slots = listeners.entry(event).or_default();
            slots.push(Some(listener));
            slots.len() - 1
        };
        let c = c.clone();
        JSFunc::new(&ctx, move || -> JSResult<()> {
            if let Some(slot) = c
                .listeners
                .borrow_mut()
                .get_mut(event)
                .and_then(|slots| slots.get_mut(slot))
            {
                *slot = None;
            }
            Ok(())
        })
    });

    let c = core.clone();
    method!(ctx, player, "dispose", move || -> JSResult<()> {
        c.dispose();
        Ok(())
    });
    Ok(())
}
