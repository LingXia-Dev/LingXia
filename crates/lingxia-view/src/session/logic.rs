//! What the View and Logic send each other, in process: owned values, never text.
use rong::quickjs::{JSContext, JSEngineValue, JSObject, JSValue};
use rong::{IntoJSValue, JSResult, error::HostError};
use serde_json::Value;

/// A bridge failure as a reply carries it: `{ code, message, reason?, data? }`.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct BridgeError {
    pub code: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    /// Boxed so the error stays small beside the values a `Result` returns.
    #[serde(default)]
    pub data: Option<Box<Value>>,
}

/// What the View sends Logic: page actions, streams, channels, and answers to its requests.
#[derive(Debug, PartialEq)]
pub enum ToLogic {
    /// The View took the connection: its state is empty and its actions go to Logic.
    Connected,
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
    /// `error` is set when the channel ended in failure.
    ChannelClose {
        id: String,
        error: Option<BridgeError>,
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

/// An absent or `undefined` field is `None`; anything else is walked.
fn tree(object: &JSObject, key: &str) -> JSResult<Option<Value>> {
    let Some(value) = object.get_opt::<_, JSValue>(key)? else {
        return Ok(None);
    };
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
                error: match tree(&object, "error")? {
                    None | Some(Value::Null) => None,
                    Some(error) => {
                        Some(serde_json::from_value(error).map_err(|_| invalid("error"))?)
                    }
                },
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

    /// The bridge frame this message stands for, for tests that read it as JSON.
    #[cfg(test)]
    pub(crate) fn frame(&self) -> Value {
        use serde_json::json;
        let mut frame = match self {
            Self::Connected => json!({"v":2,"kind":"connected"}),
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
            Self::ChannelClose { id, error } => {
                let error = error.as_ref().map(|error| json!({"code":error.code,"message":error.message,"reason":error.reason,"data":error.data}));
                json!({"v":2,"kind":"ch.close","id":id,"error":error})
            }
            Self::Reply {
                id,
                outcome: Ok(result),
            } => json!({"v":2,"kind":"res","id":id,"ok":true,"result":result}),
            Self::Reply {
                id,
                outcome: Err(error),
            } => {
                json!({"v":2,"kind":"res","id":id,"ok":false,"error":{"code":error.code,"message":error.message,"reason":error.reason,"data":error.data}})
            }
        };
        if let Some(frame) = frame.as_object_mut() {
            frame.retain(|_, value| !value.is_null());
        }
        frame
    }
}

/// What Logic sends the View besides state.
#[derive(Debug, PartialEq)]
pub enum ToView {
    Reply {
        id: String,
        outcome: Result<Value, BridgeError>,
    },
    StreamEvent {
        id: String,
        seq: u64,
        payload: Value,
    },
    ChannelAck {
        id: String,
        outcome: Result<(), BridgeError>,
    },
    ChannelData {
        id: String,
        seq: u64,
        payload: Value,
    },
    /// `error` is set when the channel ended in failure.
    ChannelClose {
        id: String,
        error: Option<BridgeError>,
    },
    /// Answered by [`ToLogic::Reply`] under the same id.
    Request { id: String, request: ViewRequest },
}

/// Logic requests the View serves, one variant per family; replies route by request id.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewRequest {
    /// `page.video(player)`: answered by the View's players, not its runtime.
    Video {
        player: String,
        command: VideoCommand,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum VideoCommand {
    Play,
    Pause,
    /// Pause, then back to the start.
    Stop,
    Seek(f64),
    RequestFullScreen,
    ExitFullScreen,
    SetStreamSource,
    SetQuality(String),
    SetPlaybackRate(f64),
    SetMuted(bool),
    SetVolume(f64),
}

/// Builds plain JS objects field by field.
struct Build<'a> {
    ctx: &'a JSContext,
    object: JSObject,
}

impl<'a> Build<'a> {
    fn new(ctx: &'a JSContext) -> Self {
        Self {
            ctx,
            object: JSObject::new(ctx),
        }
    }
    fn set(self, key: &str, value: impl IntoJSValue<JSEngineValue>) -> JSResult<Self> {
        let value = value.into_js_value(self.ctx);
        self.object.set(key, value)?;
        Ok(self)
    }
    fn tree(self, key: &str, value: &Value) -> JSResult<Self> {
        let value = super::js_tree::to_js(self.ctx, value)?;
        self.object.set(key, value)?;
        Ok(self)
    }
    fn some<T: IntoJSValue<JSEngineValue>>(self, key: &str, value: Option<T>) -> JSResult<Self> {
        match value {
            Some(value) => self.set(key, value),
            None => Ok(self),
        }
    }
    fn error(self, error: &BridgeError) -> JSResult<Self> {
        let mut built = Build::new(self.ctx).set("code", error.code.as_str())?;
        built = built.some("message", error.message.as_deref())?;
        built = built.some("reason", error.reason.as_deref())?;
        if let Some(data) = &error.data {
            built = built.tree("data", data)?;
        }
        let error = built.object;
        self.object.set("error", error)?;
        Ok(self)
    }
    fn done(self) -> JSValue {
        self.object.into_js_value()
    }
}

impl ToView {
    /// The frame object the View runtime takes for a reply, stream or channel message.
    pub(crate) fn to_js(&self, ctx: &JSContext) -> JSResult<JSValue> {
        let frame = Build::new(ctx).set("v", 2)?;
        Ok(match self {
            Self::Reply { id, outcome } => {
                let frame = frame.set("kind", "res")?.set("id", id.as_str())?;
                match outcome {
                    Ok(result) => frame.set("ok", true)?.tree("result", result)?,
                    Err(error) => frame.set("ok", false)?.error(error)?,
                }
            }
            Self::StreamEvent { id, seq, payload } => frame
                .set("kind", "event")?
                .set("id", id.as_str())?
                .set("seq", *seq as f64)?
                .tree("payload", payload)?,
            Self::ChannelAck { id, outcome } => {
                let frame = frame.set("kind", "ch.ack")?.set("id", id.as_str())?;
                match outcome {
                    Ok(()) => frame.set("ok", true)?,
                    Err(error) => frame.set("ok", false)?.error(error)?,
                }
            }
            Self::ChannelData { id, seq, payload } => frame
                .set("kind", "ch.data")?
                .set("id", id.as_str())?
                .set("seq", *seq as f64)?
                .tree("payload", payload)?,
            Self::ChannelClose { id, error } => {
                let frame = frame.set("kind", "ch.close")?.set("id", id.as_str())?;
                match error {
                    Some(error) => frame.error(error)?,
                    None => frame,
                }
            }
            Self::Request { .. } => return Err(invalid("request is not a frame")),
        }
        .done())
    }
}

#[cfg(test)]
impl ToView {
    /// The typed message a bridge frame stands for, for tests written as frames.
    pub(crate) fn from_frame(frame: &Value) -> Self {
        let id = frame["id"].as_str().unwrap_or_default().to_owned();
        let error = || BridgeError {
            code: frame["error"]["code"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            message: frame["error"]["message"].as_str().map(str::to_owned),
            reason: frame["error"]["reason"].as_str().map(str::to_owned),
            data: frame["error"].get("data").cloned().map(Box::new),
        };
        match frame["kind"].as_str().unwrap_or_default() {
            "res" if frame["ok"] == true => Self::Reply {
                id,
                outcome: Ok(frame["result"].clone()),
            },
            "res" => Self::Reply {
                id,
                outcome: Err(error()),
            },
            "event" => Self::StreamEvent {
                id,
                seq: frame["seq"].as_u64().unwrap_or_default(),
                payload: frame["payload"].clone(),
            },
            "ch.ack" => Self::ChannelAck {
                id,
                outcome: if frame["ok"] == true {
                    Ok(())
                } else {
                    Err(error())
                },
            },
            "ch.data" => Self::ChannelData {
                id,
                seq: frame["seq"].as_u64().unwrap_or_default(),
                payload: frame["payload"].clone(),
            },
            "ch.close" => Self::ChannelClose {
                id,
                error: frame
                    .get("error")
                    .filter(|error| !error.is_null())
                    .map(|_| error()),
            },
            other => panic!("not a Logic message: {other}"),
        }
    }
}
