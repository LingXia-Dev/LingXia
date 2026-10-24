//! JNI codec for the shared lifecycle and interaction core. Handles are owned
//! by Java; the UI thread is the only caller of interaction handles.
use super::guarded;
use crate::interaction::frame::PresentedFrame;
use crate::interaction::view::{
    AccessibilityAction, Effect, Key, Pointer, PointerPhase, ViewInteraction,
};
use crate::session::lifecycle::{ConnectionTicket, Lifecycle, PageTicket, SurfaceTicket};
use jni::{
    JNIEnv,
    objects::{JClass, JString},
    sys::{jboolean, jdouble, jfloat, jint, jlong, jstring},
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn lifecycle<'a>(handle: jlong) -> Result<&'a Lifecycle, String> {
    // SAFETY: Java owns the box from create until release and never uses it afterwards.
    unsafe { (handle as *const Lifecycle).as_ref() }.ok_or_else(|| "UI_LIFECYCLE: released".into())
}

fn interaction<'a>(handle: jlong) -> Result<&'a Mutex<ViewInteraction>, String> {
    // SAFETY: as for lifecycle handles.
    unsafe { (handle as *const Mutex<ViewInteraction>).as_ref() }
        .ok_or_else(|| "UI_INTERACTION: released".into())
}

fn with_interaction<T>(
    handle: jlong,
    work: impl FnOnce(&mut ViewInteraction) -> Result<T, String>,
) -> Result<T, String> {
    let mut interaction = interaction(handle)?
        .lock()
        .map_err(|_| "UI_INTERACTION: poisoned")?;
    work(&mut interaction)
}

fn text(env: &mut JNIEnv, value: impl Into<String>) -> Result<jstring, String> {
    env.new_string(value.into())
        .map(|s| s.into_raw())
        .map_err(|e| e.to_string())
}

fn input(env: &mut JNIEnv, value: &JString) -> Result<String, String> {
    env.get_string(value)
        .map(Into::into)
        .map_err(|e| e.to_string())
}

fn outcome(env: &mut JNIEnv, handled: bool, effects: Vec<Effect>) -> Result<jstring, String> {
    text(
        env,
        json!({"handled": handled, "effects": effects}).to_string(),
    )
}

fn effects(env: &mut JNIEnv, effects: Vec<Effect>) -> Result<jstring, String> {
    text(
        env,
        serde_json::to_string(&effects).map_err(|e| e.to_string())?,
    )
}

/// A frame prepared on the View worker, handed to the UI thread for presentation.
pub(super) fn frame_handle(frame: Arc<PresentedFrame>) -> jlong {
    Box::into_raw(Box::new(frame)) as jlong
}

pub(super) extern "system" fn frame_release(_: JNIEnv, _: JClass, frame: jlong) {
    if frame != 0 {
        // SAFETY: created by `frame_handle` and released exactly once by Java.
        drop(unsafe { Box::from_raw(frame as *mut Arc<PresentedFrame>) });
    }
}

// ---- lifecycle --------------------------------------------------------------

pub(super) extern "system" fn lifecycle_create(_: JNIEnv, _: JClass, logic: jboolean) -> jlong {
    Box::into_raw(Box::new(Lifecycle::new(logic != 0))) as jlong
}

pub(super) extern "system" fn lifecycle_release(_: JNIEnv, _: JClass, handle: jlong) {
    if handle != 0 {
        // SAFETY: created by `lifecycle_create` and released exactly once by Java.
        drop(unsafe { Box::from_raw(handle as *mut Lifecycle) });
    }
}

/// Transitions by opcode keep the Java surface small; each returns a ticket or a flag.
pub(super) extern "system" fn lifecycle_op(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    op: jint,
    value: jlong,
) -> jlong {
    guarded(&mut env, |_| {
        let lifecycle = lifecycle(handle)?;
        let flag = |value: bool| Ok(i64::from(value));
        match op {
            0 => Ok(lifecycle.load().get() as i64),
            1 => flag(lifecycle.loaded(PageTicket::new(value as u64))),
            2 => Ok(lifecycle.park().get() as i64),
            3 => {
                lifecycle.dispose();
                Ok(0)
            }
            4 => flag(lifecycle.connect(ConnectionTicket::new(value as u64))),
            5 => flag(lifecycle.set_bridge_ready(lifecycle.connection(), value != 0)),
            6 => {
                lifecycle.set_visible(value != 0);
                Ok(0)
            }
            7 => Ok(lifecycle.invalidate_surface().get() as i64),
            8 => Ok(lifecycle.page().get() as i64),
            9 => Ok(lifecycle.surface().get() as i64),
            10 => Ok(lifecycle.connection().get() as i64),
            11 => flag(lifecycle.is_current_page(PageTicket::new(value as u64))),
            12 => flag(lifecycle.is_current_surface(SurfaceTicket::new(value as u64))),
            13 => flag(lifecycle.is_current_connection(ConnectionTicket::new(value as u64))),
            14 => Ok(lifecycle.frame().map_or(0, |surface| surface.get() as i64)),
            15 => flag(lifecycle.accepts_input()),
            16 => flag(lifecycle.session_failed()),
            17 => flag(lifecycle.bridge_ready()),
            _ => Err("UI_LIFECYCLE: unknown operation".into()),
        }
    })
}

pub(super) extern "system" fn lifecycle_fail(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    error: JString,
) -> jstring {
    guarded(&mut env, |env| {
        let error = input(env, &error)?;
        let recovery = lifecycle(handle)?.fail(&error);
        text(env, json!({"kind": format!("{:?}", recovery.kind), "retry": format!("{:?}", recovery.retry)}).to_string())
    })
}

// ---- interaction ------------------------------------------------------------

pub(super) extern "system" fn interaction_create(_: JNIEnv, _: JClass) -> jlong {
    Box::into_raw(Box::new(Mutex::new(ViewInteraction::default()))) as jlong
}

pub(super) extern "system" fn interaction_release(_: JNIEnv, _: JClass, handle: jlong) {
    if handle != 0 {
        // SAFETY: created by `interaction_create` and released exactly once by Java.
        drop(unsafe { Box::from_raw(handle as *mut Mutex<ViewInteraction>) });
    }
}

/// Consumes `frame`. Returns `{effects, reach}` for the adapter to execute.
pub(super) extern "system" fn interaction_presented(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    frame: jlong,
    revision: jlong,
    offsets: JString,
    page: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        if frame == 0 {
            return Err("UI_INTERACTION: missing frame".into());
        }
        // SAFETY: created by `frame_handle`; presentation consumes it.
        let frame = *unsafe { Box::from_raw(frame as *mut Arc<PresentedFrame>) };
        let offsets: HashMap<String, f32> =
            serde_json::from_str(&input(env, &offsets)?).map_err(|e| format!("UI_SCROLL: {e}"))?;
        let presented = with_interaction(handle, |interaction| {
            Ok(interaction.presented(
                frame,
                revision as u64,
                &offsets,
                PageTicket::new(page as u64),
            ))
        })?;
        text(
            env,
            json!({"effects": presented.effects, "reach": presented.reach}).to_string(),
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) extern "system" fn interaction_pointer(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    phase: jint,
    x: jfloat,
    y: jfloat,
    slop: jfloat,
    velocity_x: jfloat,
    velocity_y: jfloat,
    min_fling: jfloat,
    page: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        let phase = match phase {
            0 => PointerPhase::Down,
            1 => PointerPhase::Move,
            2 => PointerPhase::Up,
            _ => PointerPhase::Cancel,
        };
        let event = Pointer {
            phase,
            x,
            y,
            slop,
            velocity: [velocity_x, velocity_y],
            min_fling,
        };
        let (handled, result) = with_interaction(handle, |interaction| {
            Ok(interaction.pointer(event, PageTicket::new(page as u64)))
        })?;
        outcome(env, handled, result)
    })
}

/// Ingress without arguments by opcode: long press, cancel, reset, focus changes.
pub(super) extern "system" fn interaction_signal(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    op: jint,
    page: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        let page = PageTicket::new(page as u64);
        let result = with_interaction(handle, |interaction| {
            Ok(match op {
                0 => interaction.long_press(),
                1 => interaction.cancel_all(),
                2 => interaction.reset(),
                3 => interaction.focus_entered(false, page),
                4 => interaction.focus_entered(true, page),
                5 => interaction.focus_left(),
                6 => {
                    interaction.set_inert(true);
                    interaction.cancel_all()
                }
                7 => {
                    interaction.set_inert(false);
                    Vec::new()
                }
                _ => return Err("UI_INTERACTION: unknown signal".into()),
            })
        })?;
        effects(env, result)
    })
}

pub(super) extern "system" fn interaction_key(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    key: jint,
    shift: jboolean,
    repeat: jboolean,
    page: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        let key = match key {
            0 => Key::Tab,
            1 => Key::Up,
            2 => Key::Down,
            3 => Key::Left,
            4 => Key::Right,
            5 => Key::Home,
            6 => Key::End,
            _ => Key::Activate,
        };
        let (handled, result) = with_interaction(handle, |interaction| {
            Ok(interaction.key(key, shift != 0, repeat != 0, PageTicket::new(page as u64)))
        })?;
        outcome(env, handled, result)
    })
}

/// Queries by opcode returning JSON: scroll state, dispatch queue, numbers, semantics.
pub(super) extern "system" fn interaction_query(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    op: jint,
    value: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        let result = with_interaction(handle, |interaction| {
            Ok(match op {
                0 => {
                    json!({"revision": interaction.scroll().revision(), "offsets": interaction.scroll().snapshot()})
                }
                1 => {
                    serde_json::to_value(interaction.take_dispatch()).map_err(|e| e.to_string())?
                }
                2 => match interaction.take_number() {
                    Some((commit, handle, value, proposal)) => {
                        json!({"commit": commit, "handle": handle, "value": value, "page": proposal.page})
                    }
                    None => serde_json::Value::Null,
                },
                3 => {
                    interaction.number_done();
                    serde_json::Value::Null
                }
                4 => match interaction.semantics() {
                    Some(semantics) => json!({
                        "roots": semantics.roots(),
                        "inputFocus": semantics.input_focus(),
                        "accessibilityFocus": interaction.accessibility_focus(),
                    }),
                    None => serde_json::Value::Null,
                },
                5 => interaction
                    .semantics()
                    .and_then(|semantics| semantics.node(value as u32))
                    .map(|node| serde_json::to_value(node).unwrap_or_default())
                    .unwrap_or_default(),
                6 => json!(interaction.can_scroll_vertically(value as i32)),
                7 => json!(interaction.keyboard_focus()),
                _ => return Err("UI_INTERACTION: unknown query".into()),
            })
        })?;
        text(env, result.to_string())
    })
}

pub(super) extern "system" fn interaction_scroll_by(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    region: JString,
    delta: jfloat,
) -> jboolean {
    guarded(&mut env, |env| {
        let region = input(env, &region)?;
        with_interaction(handle, |interaction| {
            Ok(u8::from(interaction.scroll_by(&region, delta)))
        })
    })
}

pub(super) extern "system" fn interaction_explore(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    active: jboolean,
    x: jfloat,
    y: jfloat,
) -> jstring {
    guarded(&mut env, |env| {
        let point = (active != 0).then_some([x, y]);
        let (handled, result) =
            with_interaction(handle, |interaction| Ok(interaction.explore(point)))?;
        outcome(env, handled, result)
    })
}

pub(super) extern "system" fn interaction_accessibility(
    mut env: JNIEnv,
    _: JClass,
    handle: jlong,
    id: jint,
    action: jint,
    number: jdouble,
    argument: JString,
    page: jlong,
) -> jstring {
    guarded(&mut env, |env| {
        let action = match action {
            0 => AccessibilityAction::Focus,
            1 => AccessibilityAction::ClearFocus,
            2 => AccessibilityAction::Click,
            3 => AccessibilityAction::ShowOnScreen,
            4 => AccessibilityAction::ScrollForward,
            5 => AccessibilityAction::ScrollBackward,
            6 => AccessibilityAction::SetProgress(number),
            7 => AccessibilityAction::SetText(input(env, &argument)?),
            8 => AccessibilityAction::SetSelection(
                number as i32,
                input(env, &argument)?.parse().unwrap_or(-1),
            ),
            9 => AccessibilityAction::ClearSelection,
            10 => AccessibilityAction::SelectItem(number as usize),
            _ => return Err("UI_ACCESSIBILITY: unknown action".into()),
        };
        let (handled, result) = with_interaction(handle, |interaction| {
            Ok(interaction.accessibility(id as u32, action, PageTicket::new(page as u64)))
        })?;
        outcome(env, handled, result)
    })
}
