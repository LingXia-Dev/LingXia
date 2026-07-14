//! Pinned lxapps — the sidebar pin grid's lxapp entries (user list + order).
//!
//! Lives in Rust, not platform chrome state: pins are semantic user intent
//! that every desktop skin must render identically, and mutations arrive via
//! shell UI or (future) writer APIs. Stored as a JSON array of appIds in the
//! host data dir; order is pin order.

use crate::warn;
use lingxia_platform::traits::app_runtime::AppRuntime;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

fn store_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    &LOCK
}

fn store_path() -> Option<PathBuf> {
    Some(
        super::runtime_registry::get_platform()?
            .app_data_dir()
            .join("shell-pins.json"),
    )
}

fn load() -> Vec<String> {
    let Some(path) = store_path() else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
}

fn save(pins: &[String]) -> bool {
    let Some(path) = store_path() else {
        return false;
    };
    match serde_json::to_string(pins) {
        Ok(json) => {
            if let Err(err) = fs::write(&path, json) {
                warn!("failed to write shell pins: {err}");
                return false;
            }
            true
        }
        Err(_) => false,
    }
}

/// Pinned lxapp ids in pin order.
pub fn pinned_lxapps() -> Vec<String> {
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    load()
}

pub fn is_lxapp_pinned(appid: &str) -> bool {
    pinned_lxapps().iter().any(|id| id == appid)
}

/// Pin (appends) or unpin an lxapp. Idempotent; returns whether the store
/// now reflects the requested state.
pub fn set_lxapp_pinned(appid: &str, pinned: bool) -> bool {
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    let mut pins = load();
    let present = pins.iter().any(|id| id == appid);
    match (pinned, present) {
        (true, false) => pins.push(appid.to_string()),
        (false, true) => pins.retain(|id| id != appid),
        _ => return true,
    }
    save(&pins)
}
