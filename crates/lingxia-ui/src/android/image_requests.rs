use super::*;
use std::sync::{LazyLock, Mutex};

#[derive(Default)]
struct Queues {
    next: i64,
    entries: HashMap<i64, crate::ImageRequests>,
}
static QUEUES: LazyLock<Mutex<Queues>> = LazyLock::new(|| Mutex::new(Queues::default()));

pub(super) fn current_source(id: jlong, token: jlong) -> Result<Option<String>, String> {
    let queues = QUEUES
        .lock()
        .map_err(|_| "UI_IMAGE: poisoned queue registry")?;
    Ok(queues
        .entries
        .get(&id)
        .and_then(|queue| queue.current_source(token as u64))
        .map(str::to_owned))
}

/// Decode outside the registry lock; only the final bitmap binding is atomic with invalidation.
pub(super) fn bind_current(
    id: jlong,
    token: jlong,
    bind: impl FnOnce(&str) -> Result<bool, String>,
) -> Result<bool, String> {
    let queues = QUEUES
        .lock()
        .map_err(|_| "UI_IMAGE: poisoned queue registry")?;
    match queues
        .entries
        .get(&id)
        .and_then(|queue| queue.current_source(token as u64))
    {
        Some(source) => bind(source),
        None => Ok(false),
    }
}

fn with_queue<T>(
    id: jlong,
    work: impl FnOnce(&mut crate::ImageRequests) -> Result<T, String>,
) -> Result<T, String> {
    let mut queues = QUEUES
        .lock()
        .map_err(|_| "UI_IMAGE: poisoned queue registry")?;
    work(
        queues
            .entries
            .get_mut(&id)
            .ok_or("UI_IMAGE: closed queue")?,
    )
}

pub(super) extern "system" fn create(mut env: JNIEnv, _: JClass) -> jlong {
    guarded(&mut env, |_| {
        let mut queues = QUEUES
            .lock()
            .map_err(|_| "UI_IMAGE: poisoned queue registry")?;
        queues.next = queues
            .next
            .checked_add(1)
            .ok_or("UI_IMAGE: queue identity exhausted")?;
        let id = queues.next;
        queues.entries.insert(id, crate::ImageRequests::default());
        Ok(id)
    })
}

pub(super) extern "system" fn update(mut env: JNIEnv, _: JClass, id: jlong, sources: JString) {
    guarded(&mut env, |env| {
        let sources: String = env.get_string(&sources).map_err(|e| e.to_string())?.into();
        let sources: Vec<crate::image_requests::ImageSource> =
            serde_json::from_str(&sources).map_err(|e| e.to_string())?;
        with_queue(id, |queue| queue.update_resources(&sources))
    })
}

pub(super) extern "system" fn start(mut env: JNIEnv, _: JClass, id: jlong) -> jstring {
    guarded(&mut env, |env| {
        let requests = with_queue(id, |queue| Ok(queue.start()))?;
        let encoded = serde_json::to_string(&requests).map_err(|e| e.to_string())?;
        env.new_string(encoded)
            .map(|value| value.into_raw())
            .map_err(|e| e.to_string())
    })
}

pub(super) extern "system" fn check(
    mut env: JNIEnv,
    _: JClass,
    id: jlong,
    token: jlong,
    complete: jboolean,
) -> jboolean {
    guarded(&mut env, |_| {
        with_queue(id, |queue| {
            Ok(u8::from(if complete != 0 {
                queue.complete(token as u64)
            } else {
                queue.current(token as u64)
            }))
        })
    })
}

pub(super) extern "system" fn reset(mut env: JNIEnv, _: JClass, id: jlong, close: jboolean) {
    guarded(&mut env, |_| {
        if close != 0 {
            QUEUES
                .lock()
                .map_err(|_| "UI_IMAGE: poisoned queue registry")?
                .entries
                .remove(&id);
            Ok(())
        } else {
            with_queue(id, |queue| {
                queue.clear();
                Ok(())
            })
        }
    })
}
