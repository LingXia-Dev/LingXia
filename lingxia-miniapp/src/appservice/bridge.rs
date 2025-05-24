use crate::error::MiniAppError;
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

/// Defines transport and service discovery for the Bridge.
pub(crate) trait BridgeTransport {
    fn post_message_to_view(&self, message_json: &str) -> Result<(), MiniAppError>;
    fn has_service(&self, service_name: &str) -> bool;
}

/// Bridge for communicating between Logic Layer and View Layer
///
/// This bridge handles the communication protocol between Logic and View layers
/// as defined in the LingXia Bridge Communication Specification. It processes
/// JSON messages, manages call/reply sequences, and routes events.
#[derive(Clone)]
pub(crate) struct Bridge {
    transport: Rc<dyn BridgeTransport>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ErrorPayload {
    pub message: String,
}

/// Represents different types of incoming messages for dispatch
#[derive(Clone)]
pub(crate) struct DispatchMessage {
    bridge: Bridge,
    message_type: DispatchMessageType,
    // Internal msg_id for Call messages, used for replies
    msg_id: Option<String>,
}

#[derive(Clone)]
pub(crate) enum DispatchMessageType {
    /// A function call from the view layer
    Call {
        name: String,
        payload: Option<String>,
    },
    /// An event notification from the view layer
    Event {
        name: String,
        payload: Option<String>,
    },
    /// A callback completion notification
    Callback { callback_id: String },
}

impl DispatchMessage {
    /// Create a new DispatchMessage
    pub(crate) fn new(
        bridge: Bridge,
        message_type: DispatchMessageType,
        msg_id: Option<String>,
    ) -> Self {
        Self {
            bridge,
            message_type,
            msg_id,
        }
    }

    /// Reply with success to a Call message
    /// Only works for Call messages, ignored for Event and Callback
    ///
    /// # Arguments
    /// * `result` - Optional JSON string to include in the reply. Use Some(json_string) for fast operations
    ///  that need to return data immediately, or None for operations that don't return data.
    pub fn reply_success(&self, result: Option<&str>) -> Result<(), MiniAppError> {
        match &self.message_type {
            DispatchMessageType::Call { .. } => {
                if let Some(result_json) = result {
                    let result_value = serde_json::from_str::<Value>(result_json).map_err(|e| {
                        MiniAppError::Bridge(format!("Failed to parse result JSON: {}", e))
                    })?;
                    self.bridge
                        .reply_with_result_internal(self.msg_id.clone(), result_value)
                } else {
                    self.bridge.reply_success_internal(self.msg_id.clone())
                }
            }
            _ => Ok(()), // Events and callbacks don't need replies
        }
    }

    /// Reply with failure to a Call message
    /// Only works for Call messages, ignored for Event and Callback
    pub fn reply_failure(&self, error_message: &str) -> Result<(), MiniAppError> {
        match &self.message_type {
            DispatchMessageType::Call { .. } => self
                .bridge
                .reply_failure_internal(self.msg_id.clone(), error_message),
            _ => Ok(()), // Events and callbacks don't need replies
        }
    }

    /// Get the message type
    pub fn message_type(&self) -> &DispatchMessageType {
        &self.message_type
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct IncomingMessage {
    #[serde(rename = "msgId")]
    msg_id: Option<String>,
    #[serde(rename = "type")]
    type_: String,
    name: Option<String>,
    payload: Option<Value>,
    #[serde(rename = "callbackId")]
    callback_id: Option<String>,
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
            "callback" => {
                if message.callback_id.is_none() {
                    return Err(MiniAppError::Bridge(
                        "Callback missing callbackId".to_string(),
                    ));
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

    fn msg_id(&self) -> Option<&str> {
        self.msg_id.as_deref()
    }

    /// Convert to a dispatch message for cleaner handling
    fn to_dispatch_message(&self, bridge: Bridge) -> Result<Option<DispatchMessage>, MiniAppError> {
        match self.type_.as_str() {
            "call" => {
                let name = self.name.as_ref().unwrap().clone();
                let msg_id = self.msg_id.as_ref().unwrap().clone();
                let payload = self.payload_as_opt_string()?;
                Ok(Some(DispatchMessage::new(
                    bridge,
                    DispatchMessageType::Call { name, payload },
                    Some(msg_id),
                )))
            }
            "event" => {
                let name = self.name.as_ref().unwrap().clone();
                let payload = self.payload_as_opt_string()?;
                Ok(Some(DispatchMessage::new(
                    bridge,
                    DispatchMessageType::Event { name, payload },
                    None,
                )))
            }
            "callback" => {
                let callback_id = self.callback_id.as_ref().unwrap().clone();
                Ok(Some(DispatchMessage::new(
                    bridge,
                    DispatchMessageType::Callback { callback_id },
                    None,
                )))
            }
            "reply" => {
                // Reply messages are handled internally, not dispatched
                Ok(None)
            }
            _ => Err(MiniAppError::Bridge(format!(
                "Unknown message type: {}",
                self.type_
            ))),
        }
    }
}

impl std::fmt::Display for ErrorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for ErrorPayload {}

impl Bridge {
    pub fn new(transport: Rc<dyn BridgeTransport>) -> Self {
        Self {
            transport,
            msg_counter: Rc::new(AtomicUsize::new(0)),
            pending_calls: Rc::new(Mutex::new(HashMap::new())),
        }
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
        self.transport.post_message_to_view(&serialized)?;
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

        self.transport
            .post_message_to_view(&serialized)
            .map_err(|e| {
                let mut pending_on_err = self.pending_calls.lock().unwrap();
                pending_on_err.remove(&msg_id);
                e
            })?;

        match timeout(Duration::from_millis(DEFAULT_TIMEOUT_MS), rx).await {
            Ok(Ok(bridge_result)) => bridge_result,
            Ok(Err(_)) => {
                self.pending_calls.lock().unwrap().remove(&msg_id);
                Err(MiniAppError::Bridge(format!(
                    "Reply channel closed for call '{}' (id: {}) before reply",
                    name, msg_id
                )))
            }
            Err(_) => {
                self.pending_calls.lock().unwrap().remove(&msg_id);
                Err(MiniAppError::Bridge(format!(
                    "Call '{}' (id: {}) to view timed out",
                    name, msg_id
                )))
            }
        }
    }

    /// Call a method on the View Layer and expect a result value.
    /// This is a convenience method that's identical to call() but provides clearer semantics
    /// for operations that are expected to return data.
    ///
    /// # Arguments
    /// * `name` - The method name to call
    /// * `payload` - Optional parameters for the method
    ///
    /// # Returns
    /// * `Ok(Value)` - The result data from the View Layer
    /// * `Err(MiniAppError)` - If the call failed or timed out
    pub async fn call_with_result(
        &self,
        name: &str,
        payload: Option<Value>,
    ) -> Result<Value, MiniAppError> {
        self.call(name, payload).await
    }

    /// Send data, optionally with a callback ID.
    /// If a callback ID is provided, the View Layer can use it to notify when it has processed the data.
    ///
    /// # Arguments
    /// * `data_patch_json` - JSON string containing the data patch to send to the View Layer
    /// * `callback_id` - Optional callback ID that will be included in the payload
    pub async fn set_data(
        &self,
        data_patch_json: &str,
        callback_id: Option<String>,
    ) -> Result<(), MiniAppError> {
        let mut data_patch_value = serde_json::from_str::<Value>(data_patch_json)?;

        // If we have a callback ID, we need to structure the payload according to the bridge spec
        if let Some(cb_id) = callback_id {
            // Create a new payload with the data and callbackId
            data_patch_value = json!({
                "data": data_patch_value,
                "callbackId": cb_id
            });
        }

        // Send the call with the prepared payload
        self.call("setData", Some(data_patch_value))
            .await
            .map(|_| ())
    }

    /// Process a raw message string received from the View Layer
    ///
    /// This function parses an incoming JSON message and dispatches it appropriately.
    /// For `reply` messages, it resolves the corresponding pending call.
    /// For `call`, `event`, and `callback` messages, it delegates to the provided `dispatch` function.
    ///
    /// # Arguments
    /// * `message` - The incoming message
    /// * `dispatch` - A closure that handles different types of messages
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
        F: AsyncFnOnce(DispatchMessage),
    {
        // Handle reply messages internally
        if message.type_ == "reply" {
            let msg_id = message.msg_id().unwrap();
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
                            Ok(payload_struct.result.unwrap_or(Value::Null))
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
                            MiniAppError::Bridge("Failed to send reply to waiting task".to_string())
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
            return Ok(());
        }

        // Convert to dispatch message and handle
        if let Some(dispatch_msg) = message.to_dispatch_message(self.clone())? {
            // For Call messages, check if service exists before dispatching
            match &dispatch_msg.message_type {
                DispatchMessageType::Call { name, .. } => {
                    if !self.transport.has_service(name) {
                        let _ = dispatch_msg.reply_failure(&format!("service {} not found", name));
                        return Ok(());
                    }
                    // Service exists, let user handle the reply
                    dispatch(dispatch_msg).await;
                }
                DispatchMessageType::Event { .. } | DispatchMessageType::Callback { .. } => {
                    // Events and callbacks don't need service checking
                    dispatch(dispatch_msg).await;
                }
            }
        }

        Ok(())
    }

    // Internal method for sending success replies
    fn reply_success_internal(&self, msg_id: Option<String>) -> Result<(), MiniAppError> {
        self.reply_internal(msg_id, true, None, None)
    }

    // Internal method for sending failure replies
    fn reply_failure_internal(
        &self,
        msg_id: Option<String>,
        error_message: &str,
    ) -> Result<(), MiniAppError> {
        let error_payload = ErrorPayload {
            message: error_message.to_string(),
        };
        self.reply_internal(msg_id, false, None, Some(error_payload))
    }

    // Internal method for sending result replies
    fn reply_with_result_internal(
        &self,
        msg_id: Option<String>,
        result: Value,
    ) -> Result<(), MiniAppError> {
        self.reply_internal(msg_id, true, Some(result), None)
    }

    // Common internal method for all reply types
    fn reply_internal(
        &self,
        msg_id: Option<String>,
        success: bool,
        result: Option<Value>,
        error: Option<ErrorPayload>,
    ) -> Result<(), MiniAppError> {
        let mut reply_payload = json!({
            "success": success
        });

        if let Some(result_value) = result {
            reply_payload["result"] = result_value;
        }

        if let Some(error_payload) = error {
            reply_payload["error"] = json!({
                "message": error_payload.message
            });
        }

        let reply_message = json!({
            "msgId": msg_id,
            "type": "reply",
            "payload": reply_payload
        });

        let serialized_reply = serde_json::to_string(&reply_message)?;

        self.transport
            .post_message_to_view(&serialized_reply)
            .map_err(|e| MiniAppError::Bridge(format!("Failed to post reply: {}", e)))
    }
}
