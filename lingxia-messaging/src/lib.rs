//! LingXia Messaging System
//!
//! Provides two core functionalities for cross-platform communication:
//! 1. A flexible callback registry that supports both oneshot and stream callbacks.
//! 2. A publish-subscribe system for system-wide events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tokio::sync::{mpsc, oneshot};

/// Callback result from platform
#[derive(Debug, Clone)]
pub struct CallbackResult {
    pub success: bool,
    pub data: String,
}

impl CallbackResult {
    /// Borrow the callback payload as a string slice.
    pub fn as_str(&self) -> &str {
        &self.data
    }

    /// Consume the result and return the underlying string payload.
    pub fn into_string(self) -> String {
        self.data
    }
}

enum CallbackEntry {
    Oneshot(oneshot::Sender<CallbackResult>),
    Stream(mpsc::Sender<CallbackResult>),
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

    fn register_oneshot(&self) -> (u64, oneshot::Receiver<CallbackResult>) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();

        {
            let mut callbacks = self.callbacks.lock().unwrap();
            callbacks.insert(id, CallbackEntry::Oneshot(sender));
        }

        (id, receiver)
    }

    fn register_stream(&self) -> (u64, mpsc::Receiver<CallbackResult>) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel(16); // Buffer size of 16

        {
            let mut callbacks = self.callbacks.lock().unwrap();
            callbacks.insert(id, CallbackEntry::Stream(sender));
        }

        (id, receiver)
    }

    fn unregister(&self, id: u64) -> bool {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks.remove(&id).is_some()
    }

    fn invoke(&self, id: u64, success: bool, data: String) -> bool {
        let mut callbacks = self.callbacks.lock().unwrap();
        let result = CallbackResult { success, data };

        match callbacks.get(&id) {
            Some(CallbackEntry::Oneshot(_)) => {
                // For oneshot, remove the callback after sending
                if let Some(CallbackEntry::Oneshot(sender)) = callbacks.remove(&id) {
                    let _ = sender.send(result);
                    true
                } else {
                    false
                }
            }
            Some(CallbackEntry::Stream(sender)) => {
                // For stream, keep the callback and try to send
                match sender.try_send(result) {
                    Ok(_) => true,
                    Err(mpsc::error::TrySendError::Full(_payload)) => {
                        // Channel is full; report failure so caller can retry
                        false
                    }
                    Err(mpsc::error::TrySendError::Closed(_payload)) => {
                        // Channel is closed, remove the callback
                        callbacks.remove(&id);
                        false
                    }
                }
            }
            None => false,
        }
    }
}

static CALLBACK_REGISTRY: OnceLock<CallbackRegistry> = OnceLock::new();

fn get_callback_registry() -> &'static CallbackRegistry {
    CALLBACK_REGISTRY.get_or_init(CallbackRegistry::new)
}

/// Register a oneshot callback and get its receiver.
pub fn get_callback() -> (u64, oneshot::Receiver<CallbackResult>) {
    get_callback_registry().register_oneshot()
}

/// Register a stream callback and get its receiver.
pub fn get_stream_callback() -> (u64, mpsc::Receiver<CallbackResult>) {
    get_callback_registry().register_stream()
}

/// Remove callback by ID. This is useful for cancellation or timeout scenarios.
pub fn remove_callback(id: u64) -> bool {
    get_callback_registry().unregister(id)
}

/// Invoke callback (called from platform code) to send result back.
/// For oneshot mode, this removes the callback after sending.
/// For stream mode, this keeps the callback active for future messages.
pub fn invoke_callback(id: u64, success: bool, data: impl Into<String>) -> bool {
    get_callback_registry().invoke(id, success, data.into())
}

/// Represents a system-wide event.
#[derive(Debug, Clone)]
pub struct Event {
    pub name: String,
    pub data: String,
}

struct EventRegistry {
    listeners: Mutex<HashMap<String, Vec<mpsc::Sender<Event>>>>,
}

impl EventRegistry {
    fn new() -> Self {
        Self {
            listeners: Mutex::new(HashMap::new()),
        }
    }

    fn subscribe(&self, event_name: String) -> mpsc::Receiver<Event> {
        let (sender, receiver) = mpsc::channel(16); // Channel with a buffer of 16

        let mut listeners = self.listeners.lock().unwrap();
        listeners.entry(event_name).or_default().push(sender);

        receiver
    }

    fn publish(&self, name: &str, data: &str) {
        let mut listeners = self.listeners.lock().unwrap();

        if let Some(senders) = listeners.get_mut(name) {
            let event = Event {
                name: name.to_string(),
                data: data.to_string(),
            };
            // Use retain to keep only the active senders.
            // A sender is considered inactive if its channel is closed.
            senders.retain(|sender| {
                match sender.try_send(event.clone()) {
                    Ok(_) => true,                                      // Sent successfully, keep sender.
                    Err(mpsc::error::TrySendError::Full(_)) => true, // Channel is full, listener is slow. Keep sender.
                    Err(mpsc::error::TrySendError::Closed(_)) => false, // Channel is closed, listener is gone. Remove sender.
                }
            });
        }
    }
}

static EVENT_REGISTRY: OnceLock<EventRegistry> = OnceLock::new();

fn get_event_registry() -> &'static EventRegistry {
    EVENT_REGISTRY.get_or_init(EventRegistry::new)
}

/// Subscribe to a named event.
///
/// Returns a receiver that will get a copy of every event published with that name.
pub fn subscribe(event_name: String) -> mpsc::Receiver<Event> {
    get_event_registry().subscribe(event_name)
}

/// Publish an event to all subscribers.
///
/// This is a synchronous, non-blocking function that is safe to call from any thread,
/// including the main UI thread. It will try to send to all listeners and will
/// automatically clean up any listeners whose channels have been closed.
pub fn publish(name: String, data: String) {
    get_event_registry().publish(&name, &data);
}
