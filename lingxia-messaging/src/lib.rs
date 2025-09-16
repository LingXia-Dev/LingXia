//! LingXia Callback System
//!
//! Simple callback registry for cross-platform UI interactions.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;

/// Callback result from platform
#[derive(Debug, Clone)]
pub struct CallbackResult {
    pub success: bool,
    pub data: String,
}

struct CallbackEntry {
    sender: oneshot::Sender<CallbackResult>,
}

struct CallbackRegistry {
    callbacks: Mutex<HashMap<u64, CallbackEntry>>,
    next_id: AtomicU64,
}

impl CallbackRegistry {
    fn new() -> Self {
        Self {
            callbacks: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    fn register(&self) -> (u64, oneshot::Receiver<CallbackResult>) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();

        {
            let mut callbacks = self.callbacks.lock().unwrap();
            callbacks.insert(id, CallbackEntry { sender });
        }

        (id, receiver)
    }

    fn unregister(&self, id: u64) -> bool {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks.remove(&id).is_some()
    }

    fn invoke(&self, id: u64, success: bool, data: String) -> bool {
        let mut callbacks = self.callbacks.lock().unwrap();

        if let Some(entry) = callbacks.remove(&id) {
            let _ = entry.sender.send(CallbackResult { success, data });
            true
        } else {
            false
        }
    }
}

static REGISTRY: OnceLock<CallbackRegistry> = OnceLock::new();

fn get_registry() -> &'static CallbackRegistry {
    REGISTRY.get_or_init(|| CallbackRegistry::new())
}

/// Get callback ID and receiver
pub fn get_callback() -> (u64, oneshot::Receiver<CallbackResult>) {
    get_registry().register()
}

/// Remove callback by ID
pub fn remove_callback(id: u64) -> bool {
    get_registry().unregister(id)
}

/// Invoke callback (called from platform code)
pub fn invoke_callback(id: u64, success: bool, data: String) -> bool {
    get_registry().invoke(id, success, data)
}
