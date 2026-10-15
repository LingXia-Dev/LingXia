//! `lx.audio.loadSound`: preloaded, polyphonic effects.

use super::player::{method, parse_focus};
use super::source::{invalid_options, optional_string, resolve_sound_source};
use super::{AudioContextService, audio_js_error, bind_abort};
use futures::channel::oneshot;
use lingxia_media::audio::{
    AudioError, AudioRuntime, Focus, SoundEvent, SoundOptions, VoiceParams,
};
use rong::function::Optional;
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue, Promise, PropertyDescriptor};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

const DEFAULT_VOICES: usize = 4;
const MAX_VOICES: usize = 16;

struct SoundCore {
    runtime: &'static AudioRuntime,
    id: u64,
    disposed: Cell<bool>,
    voices: RefCell<HashMap<u64, oneshot::Sender<()>>>,
}

impl SoundCore {
    fn dispose(&self) {
        if !self.disposed.replace(true) {
            self.runtime.dispose_sound(self.id);
        }
    }
}

fn number(
    object: &JSObject,
    key: &str,
    default: f64,
    range: (f64, f64),
) -> Result<f64, AudioError> {
    match object.get::<_, JSValue>(key) {
        Ok(value) if value.is_undefined() || value.is_null() => Ok(default),
        Ok(value) => value
            .to_rust::<f64>()
            .ok()
            .filter(|n| n.is_finite() && *n >= range.0 && *n <= range.1)
            .ok_or_else(|| {
                invalid_options(format!("{key} must be between {} and {}", range.0, range.1))
            }),
        Err(_) => Ok(default),
    }
}

fn parse_options(
    options: Option<&JSObject>,
) -> Result<(SoundOptions, Option<JSObject>), AudioError> {
    let mut parsed = SoundOptions {
        focus: Focus::Mix,
        obey_silent_switch: true,
        max_voices: DEFAULT_VOICES,
    };
    let Some(options) = options else {
        return Ok((parsed, None));
    };
    parsed.focus = parse_focus(optional_string(options, "focus")?, Focus::Mix)?;
    if parsed.focus == Focus::Exclusive {
        return Err(invalid_options(
            "a sound mixes or ducks; it cannot take exclusive focus",
        ));
    }
    parsed.obey_silent_switch = match options.get::<_, JSValue>("obeySilentSwitch") {
        Ok(value) if value.is_boolean() => value.to_rust::<bool>().unwrap_or(true),
        Ok(value) if value.is_undefined() || value.is_null() => parsed.focus == Focus::Mix,
        Ok(_) => return Err(invalid_options("obeySilentSwitch must be a boolean")),
        Err(_) => parsed.focus == Focus::Mix,
    };
    let voices = number(
        options,
        "maxVoices",
        DEFAULT_VOICES as f64,
        (1.0, MAX_VOICES as f64),
    )?;
    if voices.fract() != 0.0 {
        return Err(invalid_options("maxVoices must be an integer"));
    }
    parsed.max_voices = voices as usize;
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

pub(super) fn load_sound(
    ctx: JSContext,
    source: JSValue,
    options: Optional<JSObject>,
) -> JSResult<Promise> {
    let service = AudioContextService::get(&ctx)?;
    let prepared = parse_options(options.0.as_ref()).and_then(|(options, signal)| {
        resolve_sound_source(&service.app, &source).map(|path| (options, signal, path))
    });
    let (options, signal, path) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            return Promise::from_future(&ctx, None, async move {
                Err::<(), _>(audio_js_error(&error))
            });
        }
    };
    crate::task_object::validate_abort_signal(signal.as_ref())?;
    let runtime = service.runtime;
    let handle = runtime.load_sound(service.owner, options, path.to_string_lossy().into_owned());
    let core = Rc::new(SoundCore {
        runtime,
        id: handle.id,
        disposed: Cell::new(false),
        voices: RefCell::new(HashMap::new()),
    });
    let abort_core = core.clone();
    bind_abort(&ctx, signal, move || abort_core.dispose())?;

    let mut events = handle.events;
    let pump_core = core.clone();
    let pump = Promise::from_future(&ctx, None, async move {
        while let Some(SoundEvent::VoiceEnded(voice)) = events.recv().await {
            if let Some(done) = pump_core.voices.borrow_mut().remove(&voice) {
                let _ = done.send(());
            }
        }
        for (_, done) in pump_core.voices.borrow_mut().drain() {
            let _ = done.send(());
        }
        Ok::<(), rong::RongJSError>(())
    })?;

    let loaded = handle.loaded;
    let ctx_for_sound = ctx.clone();
    Promise::from_future(&ctx, None, async move {
        let duration = match loaded.await {
            Ok(Ok(duration)) => duration,
            Ok(Err(error)) => return Err(audio_js_error(&error)),
            Err(_) => return Err(audio_js_error(&AudioError::disposed())),
        };
        let ctx = ctx_for_sound;
        let sound = JSObject::new(&ctx);
        sound.set("durationMs", duration as f64)?;
        sound.define_property(
            "__lxSoundPump",
            PropertyDescriptor::from_value(pump.into_object().into_js_value()),
        )?;
        let c = core.clone();
        method!(&ctx, sound, "play", move |ctx: JSContext,
                                           options: Optional<JSObject>|
              -> JSResult<JSObject> {
            play_voice(&ctx, &c, options.0.as_ref())
        });
        let c = core.clone();
        method!(&ctx, sound, "stopAll", move || -> JSResult<()> {
            if !c.disposed.get() {
                c.runtime.stop_all_voices(c.id);
            }
            Ok(())
        });
        let c = core.clone();
        method!(&ctx, sound, "dispose", move || -> JSResult<()> {
            c.dispose();
            Ok(())
        });
        Ok(sound)
    })
}

fn play_voice(
    ctx: &JSContext,
    core: &Rc<SoundCore>,
    options: Option<&JSObject>,
) -> JSResult<JSObject> {
    let params = match options {
        Some(options) => (|| -> Result<VoiceParams, AudioError> {
            Ok(VoiceParams {
                volume: number(options, "volume", 1.0, (0.0, 1.0))?,
                rate: number(options, "rate", 1.0, (0.5, 2.0))?,
                pan: number(options, "pan", 0.0, (-1.0, 1.0))?,
                looping: match options.get::<_, JSValue>("loop") {
                    Ok(value) if value.is_boolean() => value.to_rust::<bool>().unwrap_or(false),
                    Ok(value) if value.is_undefined() || value.is_null() => false,
                    Ok(_) => return Err(invalid_options("loop must be a boolean")),
                    Err(_) => false,
                },
            })
        })()
        .map_err(|e| audio_js_error(&e))?,
        None => VoiceParams {
            volume: 1.0,
            rate: 1.0,
            pan: 0.0,
            looping: false,
        },
    };
    if core.disposed.get() {
        return Err(audio_js_error(&AudioError::disposed()));
    }
    let voice = core.runtime.play_voice(core.id, params);
    let (done, ended) = oneshot::channel::<()>();
    core.voices.borrow_mut().insert(voice, done);
    let object = JSObject::new(ctx);
    object.set(
        "ended",
        Promise::from_future(ctx, None, async move {
            let _ = ended.await;
            Ok::<(), rong::RongJSError>(())
        })?,
    )?;
    let c = core.clone();
    method!(ctx, object, "stop", move || -> JSResult<()> {
        if !c.disposed.get() {
            c.runtime.stop_voice(c.id, voice);
        }
        Ok(())
    });
    Ok(object)
}
