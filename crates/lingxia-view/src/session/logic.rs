//! What the View sends Logic, in process: owned values walked out of the isolate, never text.
use rong::quickjs::{JSObject, JSValue};
use rong::{JSResult, error::HostError};
use serde_json::Value;

/// A bridge failure as a reply carries it: `code` is a string, or a business code number.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct BridgeError {
    pub code: Value,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub data: Option<Value>,
}

/// What the View sends Logic: page actions, streams, channels, and answers to its requests.
#[derive(Debug, PartialEq)]
pub enum ToLogic {
    /// A page action or stream, or one of the page-owned `host.navigation.*` controls.
    /// `sent_us` is the debug-build probe stamp, 0 otherwise.
    Call {
        id: String,
        method: String,
        params: Option<Value>,
        sent_us: u64,
    },
    Cancel {
        id: String,
    },
    ChannelOpen {
        id: String,
        topic: String,
        params: Option<Value>,
    },
    ChannelData {
        id: String,
        payload: Value,
    },
    ChannelClose {
        id: String,
        code: Option<String>,
        reason: Option<String>,
    },
    /// The answer to a Logic request into the View.
    Reply {
        id: String,
        outcome: Result<Value, BridgeError>,
    },
}

fn invalid(what: &str) -> rong::RongJSError {
    HostError::new(
        rong::error::E_INTERNAL,
        format!("UI_BRIDGE: invalid View message ({what})"),
    )
    .into()
}

fn string(object: &JSObject, key: &str) -> JSResult<String> {
    let value: JSValue = object.get(key)?;
    if !value.is_string() {
        return Err(invalid(key));
    }
    value.into_value().try_into()
}

fn optional_string(object: &JSObject, key: &str) -> JSResult<Option<String>> {
    let value: JSValue = object.get(key)?;
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    string(object, key).map(Some)
}

/// An absent or `undefined` field is `None`; anything else is walked.
fn tree(object: &JSObject, key: &str) -> JSResult<Option<Value>> {
    let value: JSValue = object.get(key)?;
    if value.is_undefined() {
        return Ok(None);
    }
    super::js_tree::from_js(value).map(Some)
}

impl ToLogic {
    /// One message the View isolate queued (`__lingxiaTakeViewEvents`).
    pub(crate) fn from_js(message: JSValue) -> JSResult<Self> {
        let object = message
            .into_object()
            .ok_or_else(|| invalid("not an object"))?;
        let id = string(&object, "id")?;
        Ok(match string(&object, "kind")?.as_str() {
            "req" => Self::Call {
                id,
                method: string(&object, "method")?,
                params: tree(&object, "params")?,
                sent_us: super::probe::now_us(),
            },
            "cancel" => Self::Cancel { id },
            "ch.open" => Self::ChannelOpen {
                id,
                topic: string(&object, "topic")?,
                params: tree(&object, "params")?,
            },
            "ch.data" => Self::ChannelData {
                id,
                payload: tree(&object, "payload")?.unwrap_or(Value::Null),
            },
            "ch.close" => Self::ChannelClose {
                id,
                code: optional_string(&object, "code")?,
                reason: optional_string(&object, "reason")?,
            },
            "res" => {
                let ok: JSValue = object.get("ok")?;
                if !ok.is_boolean() {
                    return Err(invalid("ok"));
                }
                let ok: bool = ok.into_value().try_into()?;
                Self::Reply {
                    id,
                    outcome: if ok {
                        Ok(tree(&object, "result")?.unwrap_or(Value::Null))
                    } else {
                        let error = tree(&object, "error")?.ok_or_else(|| invalid("error"))?;
                        Err(serde_json::from_value(error).map_err(|_| invalid("error"))?)
                    },
                }
            }
            other => return Err(invalid(other)),
        })
    }

    /// A reply frame the overlays answered a Logic request with.
    pub(crate) fn from_reply(frame: Value) -> Result<Self, String> {
        let Value::Object(mut frame) = frame else {
            return Err("UI_BRIDGE: invalid View reply".into());
        };
        let id = match frame.remove("id") {
            Some(Value::String(id)) => id,
            _ => return Err("UI_BRIDGE: View reply without id".into()),
        };
        let outcome = match frame.remove("ok") {
            Some(Value::Bool(true)) => Ok(frame.remove("result").unwrap_or(Value::Null)),
            Some(Value::Bool(false)) => Err(frame
                .remove("error")
                .and_then(|error| serde_json::from_value(error).ok())
                .ok_or_else(|| format!("UI_BRIDGE: invalid View reply {id}"))?),
            _ => return Err(format!("UI_BRIDGE: invalid View reply {id}")),
        };
        Ok(Self::Reply { id, outcome })
    }

    /// The bridge frame this message stands for, for tests that read it as JSON.
    #[cfg(test)]
    pub(crate) fn frame(&self) -> Value {
        use serde_json::json;
        let mut frame = match self {
            Self::Call {
                id, method, params, ..
            } => {
                json!({"v":2,"kind":"req","id":id,"method":method,"cap":if method.starts_with("host.") {"host"} else {"page"},"params":params})
            }
            Self::Cancel { id } => json!({"v":2,"kind":"cancel","id":id}),
            Self::ChannelOpen { id, topic, params } => {
                json!({"v":2,"kind":"ch.open","id":id,"topic":topic,"cap":"page","params":params})
            }
            Self::ChannelData { id, payload } => {
                json!({"v":2,"kind":"ch.data","id":id,"payload":payload})
            }
            Self::ChannelClose { id, code, reason } => {
                json!({"v":2,"kind":"ch.close","id":id,"code":code,"reason":reason})
            }
            Self::Reply {
                id,
                outcome: Ok(result),
            } => json!({"v":2,"kind":"res","id":id,"ok":true,"result":result}),
            Self::Reply {
                id,
                outcome: Err(error),
            } => {
                json!({"v":2,"kind":"res","id":id,"ok":false,"error":{"code":error.code,"message":error.message,"data":error.data}})
            }
        };
        if let Some(frame) = frame.as_object_mut() {
            frame.retain(|_, value| !value.is_null());
        }
        frame
    }
}
