use crate::PageServiceEvent;
use crate::bridge::{
    BRIDGE_CANCELED, BRIDGE_INTERNAL_ERROR, BRIDGE_METHOD_NOT_FOUND, BRIDGE_TOPIC_NOT_FOUND,
    PageBridge, RpcError, ViewTransport,
};
use crate::error;
use crate::error::LxAppError;
use crate::lxapp::LxApp;
use crate::page::PageInstance;
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSSymbol, JSValue, JsonToJSValue, RongJSError,
    Source, error::HostError, function::Optional, js_class, js_method,
};
use rong_event::EventEmitter;
use serde::Deserialize;
use serde_json::value::RawValue;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use tokio::sync::{Mutex, oneshot};

#[js_class(clone)]
pub struct PageSvc {
    functions: HashMap<String, JSFunc>,
    /// Methods declared as `stream_handlers` in page meta — they receive an
    /// explicit `StreamHandle` JS object as their second argument and are
    /// expected to call `stream.end(result)` (or `stream.error(code, msg)`)
    /// instead of returning an async iterator.
    stream_handlers: HashSet<String>,
    this: JSObject,

    pub(crate) page: PageInstance,
    event_emitter: EventEmitter,

    // state of PageSvc
    state: Rc<Mutex<PageSvcState>>,
}

struct PageSvcState {
    callback: HashMap<String, JSFunc>,
    state_callback: HashMap<u64, JSFunc>,
    state_rev: u64,
    init_data: Option<JSObject>,
    channels: HashMap<String, ChannelState>,
}

struct ChannelState {
    /// Shared with the `ch.on()` JS closure so that listeners registered at
    /// *any* point during the channel's lifetime take effect immediately.
    listeners: Rc<RefCell<ChannelListeners>>,
    outbound_seq: u64,
}

struct ChannelListeners {
    on_data: Option<JSFunc>,
    on_close: Option<JSFunc>,
}

#[derive(Debug, Deserialize, Default)]
struct PageBindingMeta {
    #[serde(default)]
    handlers: Vec<String>,
    /// Methods that receive an explicit `StreamHandle` JS object as their
    /// second argument instead of using the `async function*` generator pattern.
    #[serde(default)]
    stream_handlers: Vec<String>,
}

fn rpc_error_from_lxapp_error(err: &LxAppError) -> RpcError {
    if let LxAppError::RongJSHost {
        code,
        message,
        data,
    } = err
    {
        return RpcError {
            code: code.clone(),
            message: Some(message.clone()),
            data: data.clone(),
        };
    }
    RpcError::new(BRIDGE_INTERNAL_ERROR, Some(err.to_string()))
}

fn rpc_error_from_rong(err: RongJSError) -> RpcError {
    let lxapp_error: LxAppError = err.into();
    rpc_error_from_lxapp_error(&lxapp_error)
}

async fn await_js_call_or_cancel<T>(
    cancel_rx: &mut oneshot::Receiver<()>,
    call: impl std::future::Future<Output = JSResult<T>>,
) -> Result<T, RpcError> {
    tokio::select! {
        _ = cancel_rx => Err(RpcError::new(BRIDGE_CANCELED, None)),
        result = call => result.map_err(rpc_error_from_rong),
    }
}

fn js_value_to_json_str(v: JSValue) -> Result<String, RpcError> {
    if v.is_undefined() || v.is_null() {
        return Ok("null".to_owned());
    }
    if v.is_boolean() {
        let b: bool = v
            .into_value()
            .try_into()
            .map_err(|e: RongJSError| rpc_error_from_rong(e))?;
        return Ok(if b { "true" } else { "false" }.to_owned());
    }
    if v.is_number() {
        let n: f64 = v
            .into_value()
            .try_into()
            .map_err(|e: RongJSError| rpc_error_from_rong(e))?;
        let num = serde_json::Number::from_f64(n).ok_or_else(|| {
            RpcError::new(BRIDGE_INTERNAL_ERROR, Some("Invalid number".to_string()))
        })?;
        return Ok(num.to_string());
    }
    if v.is_string() {
        let s: String = v
            .into_value()
            .try_into()
            .map_err(|e: RongJSError| rpc_error_from_rong(e))?;
        return serde_json::to_string(&s)
            .map_err(|e| RpcError::new(BRIDGE_INTERNAL_ERROR, Some(e.to_string())));
    }
    if let Some(obj) = v.into_object() {
        return obj
            .to_json_string()
            .map_err(|e| RpcError::new(BRIDGE_INTERNAL_ERROR, Some(e.to_string())));
    }

    Err(RpcError::new(
        BRIDGE_INTERNAL_ERROR,
        Some("Unsupported JS return type".to_string()),
    ))
}

fn get_async_iterator_symbol(ctx: &JSContext) -> Result<JSSymbol, RpcError> {
    ctx.global()
        .get::<_, JSObject>("Symbol")
        .and_then(|symbol| symbol.get::<_, JSSymbol>("asyncIterator"))
        .map_err(rpc_error_from_rong)
}

fn maybe_get_async_iterator(
    ctx: &JSContext,
    value: &JSValue,
) -> Result<Option<JSObject>, RpcError> {
    let Some(obj) = value.clone().into_object() else {
        return Ok(None);
    };

    let async_iter_symbol = get_async_iterator_symbol(ctx)?;
    if let Ok(async_iter_fn) = obj.get::<_, JSFunc>(async_iter_symbol) {
        let iterator = async_iter_fn
            .call::<_, JSObject>(Some(obj.clone()), ())
            .map_err(rpc_error_from_rong)?;
        return Ok(Some(iterator));
    }

    if obj.get::<_, JSFunc>("next").is_ok() {
        return Ok(Some(obj));
    }

    Ok(None)
}

fn get_optional_property(obj: &JSObject, field: &str, ctx: &JSContext) -> JSValue {
    obj.get::<_, JSValue>(field)
        .unwrap_or_else(|_| JSValue::undefined(ctx))
}

fn read_async_iterator_step(
    step_obj: &JSObject,
    ctx: &JSContext,
) -> Result<(bool, String), RpcError> {
    let done = step_obj
        .get::<_, bool>("done")
        .map_err(rpc_error_from_rong)?;
    let value_json = js_value_to_json_str(get_optional_property(step_obj, "value", ctx))?;
    Ok((done, value_json))
}

impl ViewTransport for PageSvc {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError> {
        if let Some(controller) = self.page.webview_controller() {
            controller
                .post_message(&message_json)
                .map_err(|e| LxAppError::WebView(e.to_string()))
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
        }
    }
}

impl PageSvc {
    pub(crate) async fn get_state_snapshot(
        &self,
        _scope: Option<&str>,
    ) -> Result<String, LxAppError> {
        let data_obj = self
            .this
            .get::<_, JSObject>("data")
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let data_json = data_obj
            .to_json_string()
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let rev = self.state.lock().await.state_rev;
        Ok(format!(r#"{{"rev":{},"state":{}}}"#, rev, data_json))
    }

    pub(crate) async fn handle_req(
        &self,
        req_id: &str,
        method: &str,
        params_json: Option<&str>,
        mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<String, RpcError> {
        let ctx = self.get_ctx();

        let build_call_arg = |json: Option<&str>| -> Option<JSValue> {
            let json = json?;
            if json == "null" {
                return None;
            }
            json.json_to_js_value(&ctx).ok()
        };

        let Some(js_func) = self.get_js_func(method) else {
            return Err(RpcError::new(
                BRIDGE_METHOD_NOT_FOUND,
                Some(format!("Method not found: {}", method)),
            ));
        };

        let call_arg = build_call_arg(params_json);

        // Explicit stream handle path — function declared in `stream_handlers`.
        // The handler receives `(params, streamHandle)` and is expected to call
        // `streamHandle.end(result)` or `streamHandle.error(code, msg)`.
        if self.stream_handlers.contains(method) {
            let (stream_handle, mut end_rx) = self.create_stream_handle(req_id)?;
            let call = async {
                match call_arg {
                    Some(val) => {
                        js_func
                            .call_async::<_, JSValue>(Some(self.this.clone()), (val, stream_handle))
                            .await
                    }
                    None => {
                        js_func
                            .call_async::<_, JSValue>(
                                Some(self.this.clone()),
                                (JSObject::new(&ctx), stream_handle),
                            )
                            .await
                    }
                }
            };
            await_js_call_or_cancel(&mut cancel_rx, call).await?;
            return tokio::select! {
                _ = &mut cancel_rx => Err(RpcError::new(BRIDGE_CANCELED, None)),
                result = &mut end_rx => match result {
                    Ok(r) => r,
                    Err(_) => Err(RpcError::new(
                        BRIDGE_INTERNAL_ERROR,
                        Some("Stream handle dropped without end/error".to_string()),
                    )),
                }
            };
        }

        // Generator / unary path.
        let fut = async {
            match call_arg {
                Some(val) => {
                    js_func
                        .call_async::<_, JSValue>(Some(self.this.clone()), (val,))
                        .await
                }
                None => {
                    js_func
                        .call_async::<_, JSValue>(Some(self.this.clone()), ())
                        .await
                }
            }
        };

        let value = tokio::select! {
            _ = &mut cancel_rx => {
                return Err(RpcError::new(BRIDGE_CANCELED, None));
            }
            res = fut => {
                match res {
                    Ok(v) => v,
                    Err(e) => return Err(rpc_error_from_rong(e)),
                }
            }
        };

        if let Some(iterator) = maybe_get_async_iterator(&ctx, &value)? {
            return self
                .consume_async_iterator(req_id, iterator, &mut cancel_rx)
                .await;
        }

        js_value_to_json_str(value)
    }

    pub(crate) async fn handle_notify(&self, method: &str, params_json: Option<&str>) {
        let Some(js_func) = self.get_js_func(method) else {
            return;
        };

        let ctx = self.get_ctx();
        let call_arg = params_json.and_then(|json| {
            if json == "null" {
                return None;
            }
            json.json_to_js_value(&ctx).ok()
        });

        let this_obj = self.this.clone();
        let method_name = method.to_string();
        let page_path = self.page.path().to_string();
        let task = async move {
            let result = match call_arg {
                Some(val) => js_func.call_async::<_, ()>(Some(this_obj), (val,)).await,
                None => js_func.call_async::<_, ()>(Some(this_obj), ()).await,
            };
            if let Err(e) = result {
                error!("[{}] notify '{}' failed: {}", page_path, method_name, e);
            }
        };
        rong::spawn_local(task);
    }

    pub(crate) async fn handle_ch_open(
        &self,
        id: &str,
        topic: &str,
        params_json: Option<&str>,
    ) -> Result<(), RpcError> {
        let Some(js_func) = self.get_js_func(topic) else {
            return Err(RpcError::new(
                BRIDGE_TOPIC_NOT_FOUND,
                Some(format!("Topic not found: {}", topic)),
            ));
        };

        let ctx = self.get_ctx();
        let call_arg = params_json.and_then(|json| {
            if json == "null" {
                return None;
            }
            json.json_to_js_value(&ctx).ok()
        });

        let listeners = Rc::new(RefCell::new(ChannelListeners {
            on_data: None,
            on_close: None,
        }));
        let channel_ctx = self.create_channel_context(id, listeners.clone())?;
        {
            let mut state = self.state.lock().await;
            state.channels.insert(
                id.to_string(),
                ChannelState {
                    listeners,
                    outbound_seq: 0,
                },
            );
        }
        let result = match call_arg {
            Some(val) => {
                js_func
                    .call_async::<_, JSValue>(Some(self.this.clone()), (val, channel_ctx))
                    .await
            }
            None => {
                js_func
                    .call_async::<_, JSValue>(
                        Some(self.this.clone()),
                        (JSObject::new(&ctx), channel_ctx),
                    )
                    .await
            }
        };
        if let Err(err) = result {
            let mut state = self.state.lock().await;
            state.channels.remove(id);
            return Err(rpc_error_from_rong(err));
        }
        Ok(())
    }

    pub(crate) async fn handle_ch_data(
        &self,
        id: &str,
        payload_json: &str,
    ) -> Result<(), RpcError> {
        let on_data = {
            let state = self.state.lock().await;
            state
                .channels
                .get(id)
                .and_then(|channel| channel.listeners.borrow().on_data.clone())
        };
        let Some(on_data) = on_data else {
            return Ok(());
        };
        let payload = payload_json
            .json_to_js_value(&self.get_ctx())
            .map_err(rpc_error_from_rong)?;
        on_data
            .call_async::<_, ()>(None, (payload,))
            .await
            .map_err(rpc_error_from_rong)
    }

    pub(crate) async fn handle_ch_close(&self, id: &str, code: Option<&str>, reason: Option<&str>) {
        let on_close = {
            let mut state = self.state.lock().await;
            state
                .channels
                .remove(id)
                .and_then(|channel| channel.listeners.borrow_mut().on_close.take())
        };
        let Some(on_close) = on_close else {
            return;
        };
        let info = JSObject::new(&self.get_ctx());
        let _ = info.set("code", code.unwrap_or_default().to_string());
        let _ = info.set("reason", reason.unwrap_or_default().to_string());
        let _ = on_close.call_async::<_, ()>(None, (info,)).await;
    }

    pub(crate) async fn handle_bridge_ready(&self) {
        let mut page_svc_clone = self.clone();
        let _ = page_svc_clone.handle_bridge_ready_internal().await;
    }

    pub(crate) async fn handle_state_ack(&self, _scope: Option<String>, rev: u64) {
        let mut state = self.state.lock().await;
        if let Some(cb) = state.state_callback.remove(&rev) {
            drop(state);
            let _ = cb.call::<_, ()>(None, ());
        }
    }

    fn get_js_func(&self, service_name: &str) -> Option<JSFunc> {
        self.functions.get(service_name).cloned()
    }
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new(
        ctx: JSContext,
        config: JSObject,
        path: String,
        meta_json: Optional<String>,
        page_instance_id: Optional<String>,
    ) -> JSResult<JSObject> {
        let lxapp = LxApp::from_ctx(&ctx)?;

        let scoped_page_instance_id = page_instance_id
            .0
            .as_deref()
            .filter(|id| !id.trim().is_empty());
        let page = match scoped_page_instance_id {
            Some(id) => lxapp.get_page_by_instance_id_str(id).ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_NOT_FOUND,
                    format!("PageInstance not found: {}", id),
                ))
            })?,
            None => lxapp.get_page(&path).ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_NOT_FOUND,
                    format!("PageInstance not found: {}", path),
                ))
            })?,
        };

        let init_data = JSObject::new(&ctx);

        if let Ok(original_data) = config.get::<_, JSObject>("data") {
            init_data.set("data", original_data)?;
        } else {
            init_data.set("data", JSObject::new(&ctx))?;
        }

        // Cache capabilities
        let mut page_svc = PageSvc {
            functions: HashMap::new(),
            stream_handlers: HashSet::new(),
            this: config.clone(),
            page,
            event_emitter: EventEmitter::default(),
            state: Rc::new(Mutex::new(PageSvcState {
                callback: HashMap::new(),
                state_callback: HashMap::new(),
                state_rev: 0,
                init_data: None,
                channels: HashMap::new(),
            })),
        };

        page_svc.register_functions(&config, meta_json.0.as_deref())?;

        {
            let mut state = page_svc.state.try_lock().unwrap();
            state.init_data = Some(init_data);
        }

        let class = Class::lookup::<PageSvc>(&ctx).unwrap();
        let instance = class.instance(page_svc);

        let binding = instance.clone();
        let mut page_svc = binding.borrow_mut::<PageSvc>().unwrap();
        page_svc.this = instance.clone();
        let page_instance_id = page_svc.page.instance_id_string();
        super::with_page_svc_map(&ctx, |page_svc_map| {
            let mut page_svc_map = page_svc_map.borrow_mut();
            if scoped_page_instance_id.is_none() {
                page_svc_map.insert(path, page_svc.clone());
            }
            page_svc_map.insert(page_instance_id, page_svc.clone());
            Ok(())
        })?;

        Ok(instance)
    }

    #[js_method(rename = "_setData")]
    async fn set_data(&self, ops_json: String, callback: Optional<JSFunc>) -> JSResult<()> {
        let mut state = self.state.lock().await;
        let bridge = self.bridge();

        if !bridge.is_ready() {
            return Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Bridge of {} is not ready", self.page.path()),
            )));
        }

        let base_rev = state.state_rev;
        let new_rev = base_rev + 1;
        state.state_rev = new_rev;

        let ops = RawValue::from_string(ops_json).map_err(|e| {
            RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
        })?;
        serde_json::from_str::<Vec<crate::bridge::JsonPatchOp>>(ops.get()).map_err(|e| {
            RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
        })?;

        let ack = if let Some(cb) = callback.0 {
            state.state_callback.insert(new_rev, cb);
            Some(true)
        } else {
            None
        };

        drop(state);

        bridge
            .send_state_patch(self, None, base_rev, new_rev, ops, ack)
            .map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;

        Ok(())
    }

    pub fn get_event_emitter(&self) -> EventEmitter {
        self.event_emitter.clone()
    }

    #[js_method(gc_mark)]
    pub fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        for (_, func) in self.functions.iter() {
            mark_fn(func.as_js_value());
        }
        mark_fn(self.this.as_js_value());

        if let Ok(state) = self.state.try_lock() {
            for (_, func) in state.callback.iter() {
                mark_fn(func.as_js_value());
            }
            for (_, func) in state.state_callback.iter() {
                mark_fn(func.as_js_value());
            }
            for channel in state.channels.values() {
                let ls = channel.listeners.borrow();
                if let Some(f) = &ls.on_data {
                    mark_fn(f.as_js_value());
                }
                if let Some(f) = &ls.on_close {
                    mark_fn(f.as_js_value());
                }
            }
        }
    }
}

impl PageSvc {
    fn register_functions(&mut self, obj: &JSObject, meta_json: Option<&str>) -> JSResult<()> {
        let meta: PageBindingMeta = meta_json
            .map(serde_json::from_str)
            .transpose()
            .map_err(|e| HostError::new(rong::error::E_INTERNAL, e.to_string()))?
            .unwrap_or_default();

        for function_name in meta.handlers {
            if function_name.starts_with('_') {
                continue;
            }
            if let Ok(func) = obj.get::<_, JSFunc>(function_name.as_str()) {
                self.functions.insert(function_name, func);
            }
        }

        for function_name in meta.stream_handlers {
            if function_name.starts_with('_') {
                continue;
            }
            if let Ok(func) = obj.get::<_, JSFunc>(function_name.as_str()) {
                self.functions.insert(function_name.clone(), func);
                self.stream_handlers.insert(function_name);
            }
        }

        // Metadata is intentionally conservative; fall back to runtime
        // reflection so spreads and aliased handlers still register.
        for key_value in obj.keys()? {
            let Ok(function_name) = key_value.to_rust::<String>() else {
                continue;
            };
            if function_name.starts_with('_') {
                continue;
            }
            if let Ok(func) = obj.get::<_, JSFunc>(function_name.as_str()) {
                self.functions.insert(function_name, func);
            }
        }
        Ok(())
    }

    async fn consume_async_iterator(
        &self,
        stream_id: &str,
        iterator: JSObject,
        cancel_rx: &mut tokio::sync::oneshot::Receiver<()>,
    ) -> Result<String, RpcError> {
        let ctx = self.get_ctx();
        let next_fn = iterator
            .get::<_, JSFunc>("next")
            .map_err(rpc_error_from_rong)?;
        let return_fn = iterator.get::<_, JSFunc>("return").ok();
        let mut seq = 0u64;

        loop {
            let step_obj = tokio::select! {
                _ = &mut *cancel_rx => {
                    if let Some(return_fn) = return_fn.clone() {
                        let _ = return_fn.call_async::<_, JSObject>(Some(iterator.clone()), ()).await;
                    }
                    return Err(RpcError::new(BRIDGE_CANCELED, None));
                }
                step = next_fn.call_async::<_, JSObject>(Some(iterator.clone()), ()) => {
                    step.map_err(rpc_error_from_rong)?
                }
            };

            let (done, value_json) = read_async_iterator_step(&step_obj, &ctx)?;

            if done {
                return Ok(value_json);
            }

            self.bridge()
                .send_event(self, stream_id.to_string(), seq, value_json)
                .map_err(|e| RpcError::new(BRIDGE_INTERNAL_ERROR, Some(e.to_string())))?;
            seq += 1;
        }
    }

    /// Create an explicit stream handle JS object for Logic-layer functions
    /// that prefer an imperative push API over the generator pattern.
    ///
    /// The returned object exposes `send(data)`, `end(result)`, `error(code, msg)`.
    /// The caller awaits `end_rx` to receive the final result (or error) once
    /// the JS function has finished pushing events.
    fn create_stream_handle(
        &self,
        req_id: &str,
    ) -> Result<(JSObject, oneshot::Receiver<Result<String, RpcError>>), RpcError> {
        let ctx = self.get_ctx();
        let handle = JSObject::new(&ctx);

        handle
            .set("id", req_id.to_string())
            .map_err(rpc_error_from_rong)?;

        // Shared seq counter for outbound events.
        let seq = Rc::new(Cell::new(0u64));

        // Shared oneshot sender — whichever of end / error fires first wins.
        let (end_tx, end_rx) = oneshot::channel::<Result<String, RpcError>>();
        let end_tx_cell = Rc::new(RefCell::new(Some(end_tx)));

        // stream.send(data) — emits a stream event to the View.
        let page_send = self.clone();
        let id_send = req_id.to_string();
        let seq_send = seq.clone();
        let send_fn = JSFunc::new(&ctx, move |payload: JSValue| {
            let page = page_send.clone();
            let id = id_send.clone();
            let s = seq_send.get();
            seq_send.set(s + 1);
            async move {
                let payload_json = js_value_to_json_str(payload).map_err(|e: RpcError| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        e.message.unwrap_or(e.code),
                    ))
                })?;
                page.bridge()
                    .send_event(&page, id, s, payload_json)
                    .map_err(|e| {
                        RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
                    })?;
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        handle.set("send", send_fn).map_err(rpc_error_from_rong)?;

        // stream.end(result) — finalises the stream with a return value.
        let tx_end = end_tx_cell.clone();
        let end_fn = JSFunc::new(&ctx, move |result: JSValue| {
            let tx = tx_end.clone();
            async move {
                let result_json = js_value_to_json_str(result).map_err(|e: RpcError| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        e.message.unwrap_or(e.code),
                    ))
                })?;
                if let Some(sender) = tx.borrow_mut().take() {
                    let _ = sender.send(Ok(result_json));
                }
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        handle.set("end", end_fn).map_err(rpc_error_from_rong)?;

        // stream.error(code, message) — finalises the stream with an error.
        let tx_err = end_tx_cell.clone();
        let error_fn = JSFunc::new(&ctx, move |code: String, message: Optional<String>| {
            let tx = tx_err.clone();
            async move {
                if let Some(sender) = tx.borrow_mut().take() {
                    let _ = sender.send(Err(RpcError::new(code, message.0)));
                }
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        handle.set("error", error_fn).map_err(rpc_error_from_rong)?;

        Ok((handle, end_rx))
    }

    fn create_channel_context(
        &self,
        id: &str,
        listeners: Rc<RefCell<ChannelListeners>>,
    ) -> Result<JSObject, RpcError> {
        let ctx = self.get_ctx();
        let channel_ctx = JSObject::new(&ctx);
        channel_ctx
            .set("id", id.to_string())
            .map_err(rpc_error_from_rong)?;

        // ch.send(payload)
        let channel_id = id.to_string();
        let page_svc_send = self.clone();
        let send_fn = JSFunc::new(&ctx, move |payload: JSValue| {
            let page_svc = page_svc_send.clone();
            let channel_id = channel_id.clone();
            async move {
                let payload_json = js_value_to_json_str(payload).map_err(|e| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        e.message.unwrap_or(e.code),
                    ))
                })?;
                let seq = {
                    let mut state = page_svc.state.lock().await;
                    let channel = state.channels.get_mut(&channel_id).ok_or_else(|| {
                        RongJSError::from(HostError::new(
                            rong::error::E_INTERNAL,
                            format!("Channel closed: {}", channel_id),
                        ))
                    })?;
                    let seq = channel.outbound_seq;
                    channel.outbound_seq += 1;
                    seq
                };
                page_svc
                    .bridge()
                    .send_ch_data(&page_svc, channel_id.clone(), seq, payload_json)
                    .map_err(|e| {
                        RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
                    })?;
                Ok(())
            }
        })
        .map_err(rpc_error_from_rong)?;
        channel_ctx
            .set("send", send_fn)
            .map_err(rpc_error_from_rong)?;

        // ch.close(code?, reason?)
        let channel_id = id.to_string();
        let page_svc_close = self.clone();
        let close_fn = JSFunc::new(
            &ctx,
            move |code: Optional<String>, reason: Optional<String>| {
                let page_svc = page_svc_close.clone();
                let channel_id = channel_id.clone();
                async move {
                    let on_close = {
                        let mut state = page_svc.state.lock().await;
                        state
                            .channels
                            .remove(&channel_id)
                            .and_then(|channel| channel.listeners.borrow_mut().on_close.take())
                    };
                    if let Some(on_close) = on_close {
                        let info = JSObject::new(&page_svc.get_ctx());
                        let _ = info.set("code", code.0.clone().unwrap_or_default());
                        let _ = info.set("reason", reason.0.clone().unwrap_or_default());
                        let _ = on_close.call_async::<_, ()>(None, (info,)).await;
                    }
                    page_svc
                        .bridge()
                        .send_ch_close(&page_svc, channel_id, code.0, reason.0)
                        .map_err(|e| {
                            RongJSError::from(HostError::new(
                                rong::error::E_INTERNAL,
                                e.to_string(),
                            ))
                        })?;
                    Ok(())
                }
            },
        )
        .map_err(rpc_error_from_rong)?;
        channel_ctx
            .set("close", close_fn)
            .map_err(rpc_error_from_rong)?;

        // ch.on(event, handler)
        let on_fn = JSFunc::new(&ctx, move |event: String, handler: JSFunc| {
            let mut ls = listeners.borrow_mut();
            match event.as_str() {
                "data" => ls.on_data = Some(handler),
                "close" => ls.on_close = Some(handler),
                _ => {}
            }
            Ok(())
        })
        .map_err(rpc_error_from_rong)?;
        channel_ctx.set("on", on_fn).map_err(rpc_error_from_rong)?;

        Ok(channel_ctx)
    }

    pub(crate) async fn call_or_event_from_native(
        &self,
        ctx: &JSContext,
        func_name: &str,
        args: Option<&str>,
    ) -> JSResult<()> {
        if let Some(func) = self.functions.get(func_name) {
            let args_obj = args.and_then(|json| rong::JSObject::from_json_string(ctx, json).ok());
            return match args_obj {
                Some(obj) => {
                    func.call_async::<_, ()>(Some(self.this.clone()), (obj,))
                        .await
                }
                None => func.call_async::<_, ()>(Some(self.this.clone()), ()).await,
            };
        }
        Err(RongJSError::from(HostError::new(
            rong::error::E_INTERNAL,
            format!("No service: {}", func_name),
        )))
    }

    pub(crate) async fn call_page_event(
        &self,
        ctx: &JSContext,
        event: PageServiceEvent,
        args: Option<&str>,
    ) -> JSResult<()> {
        if let Some(js_func) = self.functions.get(event.as_str()) {
            let args_obj = args.and_then(|json| rong::JSObject::from_json_string(ctx, json).ok());
            rong::enqueue_js_invoke(
                ctx,
                js_func.clone(),
                Some(self.this.clone()),
                args_obj,
                rong::JsInvokePriority::Normal,
                None,
                false,
            )
            .await
        } else {
            // PageInstance lifecycle handlers are optional by design.
            Ok(())
        }
    }

    async fn handle_bridge_ready_internal(&mut self) -> JSResult<()> {
        let mut state = self.state.lock().await;

        if let Some(init_data) = state.init_data.take() {
            // Extract the "data" field - this is the actual page data
            let page_data = init_data
                .get::<_, JSObject>("data")
                .unwrap_or_else(|_| JSObject::new(&self.this.context()));
            let data_json = page_data.to_json_string()?;

            state.state_rev = 1;
            drop(state);

            self.bridge()
                .send_state_snapshot(self, None, 1, data_json)
                .map_err(|e| {
                    RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
                })?;
        } else {
            drop(state);
        }

        self.page.notify_bridge_ready();
        Ok(())
    }

    pub(crate) fn bridge(&self) -> PageBridge {
        self.page.bridge()
    }

    pub(crate) fn get_ctx(&self) -> JSContext {
        self.this.context()
    }

    pub fn bind_surface(&self, surface: JSObject) -> JSResult<()> {
        self.this.set("surface", surface)?;
        Ok(())
    }

    pub fn clear_surface(&self) -> JSResult<()> {
        self.this.delete("surface")?;
        Ok(())
    }

    pub fn bind_opener(&self, opener: JSObject) -> JSResult<()> {
        self.this.set("opener", opener)?;
        Ok(())
    }

    pub fn clear_opener(&self) -> JSResult<()> {
        self.this.delete("opener")?;
        Ok(())
    }
}

impl PageSvc {
    pub async fn create_in_ctx(
        ctx: &JSContext,
        path: &str,
        page_instance_id: Option<&str>,
    ) -> JSResult<()> {
        super::plugin::ensure_plugin_logic_loaded_for_page_path(ctx, path).await?;
        let lxapp = LxApp::from_ctx(ctx)?;
        let definition_path =
            crate::resolve_page_path(&lxapp, path).unwrap_or_else(|| path.to_string());

        let create_page = ctx
            .global()
            .get::<_, JSFunc>("__LX_CREATE_PAGE__")
            .map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;

        create_page
            .call::<_, ()>(
                None,
                (
                    path.to_string(),
                    definition_path,
                    page_instance_id.unwrap_or_default().to_string(),
                ),
            )
            .map_err(|e: RongJSError| e.into_host_in(ctx))
    }

    pub fn get_page(&self) -> PageInstance {
        self.page.clone()
    }
}

impl LxApp {
    pub async fn get_or_create_page_in_ctx(&self, ctx: &JSContext, url: &str) -> JSResult<PageSvc> {
        let page = self.get_or_create_page(url);

        page.wait_webview_ready()
            .await
            .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e)))?;

        let path = page.path();

        super::with_page_svc_map(ctx, |page_svc_map| {
            page_svc_map
                .borrow()
                .get(path.as_str())
                .cloned()
                .ok_or_else(|| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        "PageInstance service not found",
                    ))
                })
        })
    }

    pub async fn get_page_in_ctx_by_instance_id(
        &self,
        ctx: &JSContext,
        page_instance_id: &str,
    ) -> JSResult<PageSvc> {
        let page = self
            .get_page_by_instance_id_str(page_instance_id)
            .ok_or_else(|| {
                RongJSError::from(HostError::new(
                    rong::error::E_NOT_FOUND,
                    format!("PageInstance not found: {page_instance_id}"),
                ))
            })?;

        page.wait_webview_ready()
            .await
            .map_err(|e| RongJSError::from(HostError::new(rong::error::E_INTERNAL, e)))?;

        super::with_page_svc_map(ctx, |page_svc_map| {
            page_svc_map
                .borrow()
                .get(page_instance_id)
                .cloned()
                .ok_or_else(|| {
                    RongJSError::from(HostError::new(
                        rong::error::E_INTERNAL,
                        "PageInstance service not found",
                    ))
                })
        })
    }
}

fn get_current_pages(ctx: JSContext) -> JSResult<Vec<JSObject>> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let paths = lxapp.get_page_stack();
    let mut pages = Vec::new();
    for p in paths {
        if let Some(page_obj) = super::with_page_svc_map(&ctx, |page_svc_map| {
            Ok(page_svc_map
                .borrow()
                .get(&p)
                .map(|page_svc| page_svc.this.clone()))
        })? {
            pages.push(page_obj);
        }
    }
    Ok(pages)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<PageSvc>()?;

    let page_js = Source::from_bytes(include_str!("scripts/Page.js"));
    ctx.eval::<()>(page_js)?;

    let get_current_pages = rong::JSFunc::new(ctx, get_current_pages)?;
    ctx.global().set("getCurrentPages", get_current_pages)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pending_js_call_is_interrupted_by_request_cancellation() {
        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        cancel_tx.send(()).unwrap();

        let result =
            await_js_call_or_cancel(&mut cancel_rx, std::future::pending::<JSResult<()>>()).await;

        assert_eq!(result.unwrap_err().code, BRIDGE_CANCELED);
    }
}
