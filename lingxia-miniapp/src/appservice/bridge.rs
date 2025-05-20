use crate::error::MiniAppError;
use crate::page::{Page, WebViewController};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

/// Bridge for communicating between Logic Layer and View Layer
///
/// This bridge handles the communication protocol between Logic and View layers
/// as defined in the LingXia Bridge Communication Specification. It processes
/// JSON messages, manages call/reply sequences, and routes events.
#[derive(Clone)]
pub(crate) struct Bridge {
    page: Option<Page>,

    // Use Rc because the Bridge type only lives within a single JavaScript runtime thread.
    msg_counter: Rc<AtomicUsize>,
    pending_calls: Rc<Mutex<PendingCallsMap>>,
}

/// Type alias for the pending calls map to simplify the complex type.
type PendingCallsMap = HashMap<String, oneshot::Sender<Result<Value, MiniAppError>>>;

// Set a more reasonable default timeout for message calls (5 seconds)
#[allow(dead_code)]
const DEFAULT_TIMEOUT_MS: u64 = 5000;

#[derive(Serialize, Deserialize, Debug)]
struct ReplyPayload {
    success: bool,
    error: Option<ErrorPayload>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorPayload {
    pub message: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct IncomingMessage {
    #[serde(rename = "msgId")]
    msg_id: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub name: Option<String>,
    payload: Option<Value>,
}

impl IncomingMessage {
    pub fn from_json_str(json_str: &str) -> Result<Self, MiniAppError> {
        let message: Self = serde_json::from_str(json_str)?;

        match message.type_.as_str() {
            "reply" => {
                if message.msg_id.is_none() {
                    return Err(MiniAppError::Bridge("Reply missing msgId".to_string()));
                }
            }
            "call" | "event" => {
                if message.name.is_none() {
                    return Err(MiniAppError::Bridge(format!(
                        "Message type '{}' missing 'name' field",
                        message.type_
                    )));
                }
            }
            unknown_type => {
                return Err(MiniAppError::Bridge(format!(
                    "Unknown message type: {}",
                    unknown_type
                )));
            }
        }
        Ok(message)
    }

    fn payload_as_opt_string(&self) -> Result<Option<String>, MiniAppError> {
        self.payload
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|e| MiniAppError::Bridge(format!("Payload serialization failed: {}", e)))
    }

    pub fn msg_id_str(&self) -> Option<&str> {
        self.msg_id.as_deref()
    }

    pub fn reply_to_call(
        &self,
        page: &Page,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<(), MiniAppError> {
        let reply_payload = if success {
            json!({
                "success": true
            })
        } else {
            json!({
                "success": false,
                "error": {
                    "message": error_message.unwrap_or("Unknown error")
                }
            })
        };

        let reply_message = json!({
            "msgId": self.msg_id,
            "type": "reply",
            "payload": reply_payload
        });

        let serialized_reply = serde_json::to_string(&reply_message)?;

        page.post_message(&serialized_reply)
            .map_err(|e| MiniAppError::Bridge(format!("Failed to post reply: {}", e)))
    }
}

impl std::fmt::Display for ErrorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for ErrorPayload {}

impl Bridge {
    pub fn new() -> Self {
        Self {
            page: None,
            msg_counter: Rc::new(AtomicUsize::new(0)),
            pending_calls: Rc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Sets the page for the bridge
    pub fn set_page(&mut self, page: Page) {
        self.page = Some(page);
    }

    fn generate_msg_id(&self) -> String {
        let count = self.msg_counter.fetch_add(1, Ordering::Relaxed);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("logic-{}-{}", timestamp, count)
    }

    /// Sends an event to the View Layer
    ///
    /// Events are fire-and-forget messages that don't expect a reply.
    ///
    /// # Arguments
    /// * `name` - The event name
    /// * `payload` - Optional data associated with the event
    pub fn send_event(&self, name: &str, payload: Option<Value>) -> Result<(), MiniAppError> {
        let event_message = json!({
            "msgId": Value::Null,
            "type": "event",
            "name": name,
            "payload": payload
        });

        let serialized = serde_json::to_string(&event_message)?;
        self.page.as_ref().unwrap().post_message(&serialized)?;
        Ok(())
    }

    /// Call a method on the View Layer and wait for a reply with timeout.
    ///
    /// This function sends a message to the View Layer and waits for a response.
    /// If the View Layer doesn't respond within the timeout period, an error is returned
    /// and the pending call is cleaned up.
    async fn call(&self, name: &str, payload: Option<Value>) -> Result<Value, MiniAppError> {
        let msg_id = self.generate_msg_id();
        let call_message = json!({
            "msgId": msg_id.clone(),
            "type": "call",
            "name": name,
            "payload": payload
        });
        let serialized = serde_json::to_string(&call_message)?;

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_calls.lock().unwrap();
            pending.insert(msg_id.clone(), tx);
        }

        if let Err(e) = self.page.as_ref().unwrap().post_message(&serialized) {
            let mut pending = self.pending_calls.lock().unwrap();
            pending.remove(&msg_id);
            return Err(MiniAppError::Bridge(format!(
                "Posting call '{}' failed: {}",
                name, e
            )));
        }

        match timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS), rx).await {
            Ok(Ok(bridge_result)) => bridge_result,
            Ok(Err(_recv_error)) => {
                let mut pending = self.pending_calls.lock().unwrap();
                pending.remove(&msg_id);
                Err(MiniAppError::Bridge(format!(
                    "Channel closed for call '{}' (id: {})",
                    name, msg_id
                )))
            }
            Err(_elapsed) => {
                let mut pending = self.pending_calls.lock().unwrap();
                pending.remove(&msg_id);
                Err(MiniAppError::Bridge(format!(
                    "Call '{}' (id: {}) timed out",
                    name, msg_id
                )))
            }
        }
    }

    /// Update data in the View Layer with timeout.
    pub async fn set_data(&self, data_patch_json: &str) -> Result<(), MiniAppError> {
        let data_patch_value = serde_json::from_str::<Value>(data_patch_json)?;
        self.call("setData", Some(data_patch_value))
            .await
            .map(|_| ())
    }

    /// Process a raw message string received from the View Layer
    ///
    /// This function parses an incoming JSON message and dispatches it appropriately.
    /// For `reply` messages, it resolves the corresponding pending call.
    /// For `call` and `event` messages, it delegates to the provided `dispatch` function.
    ///
    /// # Arguments
    /// * `message` - The incoming message
    /// * `dispatch` - A closure that handles `call` and `event` messages.
    ///   It receives three arguments:
    ///   * `msg_type`: The message type ("call" or "event")
    ///   * `name`: The handler name or event name
    ///   * `payload`: Optional JSON payload as a string, or None if no payload is present
    ///
    /// # Returns
    /// * `Ok(())` if the message was processed successfully
    /// * `Err(MiniAppError)` if there was an error processing the message
    pub async fn process_incoming_message<F>(
        &self,
        message: Arc<IncomingMessage>,
        dispatch: F,
    ) -> Result<(), MiniAppError>
    where
        F: AsyncFnOnce(String, String, Option<String>),
    {
        match message.type_.as_str() {
            "reply" => {
                let msg_id = message.msg_id_str().unwrap();
                let sender = {
                    let mut pending = self.pending_calls.lock().unwrap();
                    pending.remove(msg_id)
                };

                if let Some(tx) = sender {
                    let reply_payload_value =
                        message.payload.as_ref().map_or(Value::Null, |v| v.clone());
                    let payload_struct_result: Result<ReplyPayload, serde_json::Error> =
                        serde_json::from_value(reply_payload_value);

                    match payload_struct_result {
                        Ok(payload_struct) => {
                            let result = if payload_struct.success {
                                Ok(Value::Null)
                            } else {
                                Err(MiniAppError::Bridge(
                                    payload_struct
                                        .error
                                        .unwrap_or_else(|| ErrorPayload {
                                            message: "Unknown view error".to_string(),
                                        })
                                        .to_string(),
                                ))
                            };
                            let _ = tx.send(result).map_err(|_e| {
                                MiniAppError::Bridge(
                                    "Failed to send reply to waiting task".to_string(),
                                )
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(Err(MiniAppError::from(e))).map_err(|_send_error| {
                                MiniAppError::Bridge(
                                    "Failed to send reply deserialization error to waiting task"
                                        .to_string(),
                                )
                            });
                        }
                    }
                }
            }
            "call" | "event" => {
                let name = message.name.clone().unwrap();
                let payload_s = message.payload_as_opt_string()?;
                dispatch(message.type_.clone(), name, payload_s).await;
            }
            _ => {
                return Err(MiniAppError::Bridge(format!(
                    "Unexpected message type '{}' in process_incoming_message",
                    message.type_
                )));
            }
        }
        Ok(())
    }
}
