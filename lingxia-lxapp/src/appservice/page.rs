use super::bridge::{
    BRIDGE_CANCELED, BRIDGE_INTERNAL_ERROR, BRIDGE_METHOD_NOT_FOUND, Bridge, JsonPatchOp,
    MessageHandler, MessageTransport, RpcError, ServiceType,
};
use crate::PageServiceEvent;
use crate::error;
use crate::error::LxAppError;
use crate::host::get_host;
use crate::lxapp::LxApp;
use crate::page::Page;
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, error::HostError,
    function::Optional, js_class, js_export, js_method,
};
use rong_event::EventEmitter;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::rc::Rc;
use tokio::sync::Mutex;

#[js_export]
pub struct PageSvc {
    functions: HashMap<String, JSFunc>,
    this: JSObject,

    pub(crate) page: Page,
    bridge: Bridge,

    event_emitter: EventEmitter,

    // state of PageSvc
    state: Rc<Mutex<PageSvcState>>,
}

struct PageSvcState {
    callback: HashMap<String, JSFunc>,
    state_callback: HashMap<u64, JSFunc>,
    state_rev: u64,
    init_data: Option<JSObject>,
}

impl MessageTransport for PageSvc {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError> {
        if let Some(controller) = self.page.webview_controller() {
            controller
                .post_message(message_json)
                .map_err(|e| LxAppError::WebView(e.to_string()))
        } else {
            Err(LxAppError::WebView("WebView not ready".to_string()))
        }
    }
}

impl MessageHandler for PageSvc {
    fn get_service_type(&self, service_name: &str) -> ServiceType {
        if let Some(api_name) = service_name.strip_prefix("host.")
            && let Some(handler) = get_host(api_name)
        {
            return ServiceType::HostAPI(handler);
        }

        if let Some(js_func) = self.functions.get(service_name) {
            return ServiceType::JSFunc(js_func.clone());
        }

        ServiceType::None
    }

    async fn get_state_snapshot(&self, _scope: Option<&str>) -> Result<Value, LxAppError> {
        let data_obj = self
            .this
            .get::<_, JSObject>("data")
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let data_json = data_obj
            .json_stringify()
            .map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let state: Value =
            serde_json::from_str(&data_json).map_err(|e| LxAppError::Bridge(e.to_string()))?;
        let rev = self.state.try_lock().map(|s| s.state_rev).unwrap_or(0);
        Ok(json!({ "rev": rev, "state": state }))
    }

    async fn handle_req(
        &self,
        method: &str,
        params_json: Option<&str>,
        service_type: ServiceType,
        mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<Value, RpcError> {
        let ctx = self.get_ctx();

        let build_args_obj = |json: Option<&str>| -> Option<JSObject> {
            let json = json?;
            match serde_json::from_str::<Value>(json) {
                Ok(Value::Object(_)) => rong::JSObject::from_json_string(&ctx, json).ok(),
                Ok(Value::Null) => None,
                Ok(other) => {
                    let wrapped = json!({ "value": other }).to_string();
                    rong::JSObject::from_json_string(&ctx, &wrapped).ok()
                }
                Err(_) => None,
            }
        };

        let js_value_to_json = |v: JSValue| -> Result<Value, RpcError> {
            if v.is_undefined() || v.is_null() {
                return Ok(Value::Null);
            }
            if v.is_boolean() {
                let b: bool = v
                    .into_value()
                    .try_into()
                    .map_err(|e: RongJSError| RpcError {
                        code: BRIDGE_INTERNAL_ERROR.to_string(),
                        message: Some(e.to_string()),
                    })?;
                return Ok(Value::Bool(b));
            }
            if v.is_number() {
                let n: f64 = v
                    .into_value()
                    .try_into()
                    .map_err(|e: RongJSError| RpcError {
                        code: BRIDGE_INTERNAL_ERROR.to_string(),
                        message: Some(e.to_string()),
                    })?;
                return Ok(Value::Number(serde_json::Number::from_f64(n).ok_or_else(
                    || RpcError {
                        code: BRIDGE_INTERNAL_ERROR.to_string(),
                        message: Some("Invalid number".to_string()),
                    },
                )?));
            }
            if v.is_string() {
                let s: String = v
                    .into_value()
                    .try_into()
                    .map_err(|e: RongJSError| RpcError {
                        code: BRIDGE_INTERNAL_ERROR.to_string(),
                        message: Some(e.to_string()),
                    })?;
                return Ok(Value::String(s));
            }
            if let Some(obj) = v.into_object() {
                let s = obj.json_stringify().map_err(|e| RpcError {
                    code: BRIDGE_INTERNAL_ERROR.to_string(),
                    message: Some(e.to_string()),
                })?;
                return serde_json::from_str(&s).map_err(|e| RpcError {
                    code: BRIDGE_INTERNAL_ERROR.to_string(),
                    message: Some(e.to_string()),
                });
            }

            Err(RpcError {
                code: BRIDGE_INTERNAL_ERROR.to_string(),
                message: Some("Unsupported JS return type".to_string()),
            })
        };

        match service_type {
            ServiceType::JSFunc(js_func) => {
                let args_obj = build_args_obj(params_json);
                let fut = async {
                    match args_obj {
                        Some(obj) => {
                            js_func
                                .call_async::<_, JSValue>(Some(self.this.clone()), (obj,))
                                .await
                        }
                        None => {
                            js_func
                                .call_async::<_, JSValue>(Some(self.this.clone()), ())
                                .await
                        }
                    }
                };

                tokio::select! {
                    _ = &mut cancel_rx => {
                        Err(RpcError { code: BRIDGE_CANCELED.to_string(), message: None })
                    }
                    res = fut => {
                        match res {
                            Ok(v) => js_value_to_json(v),
                            Err(e) => Err(RpcError { code: BRIDGE_INTERNAL_ERROR.to_string(), message: Some(e.to_string()) }),
                        }
                    }
                }
            }
            ServiceType::HostAPI(handler) => {
                let lxapp = LxApp::from_ctx(&ctx).map_err(|e| RpcError {
                    code: BRIDGE_INTERNAL_ERROR.to_string(),
                    message: Some(e.to_string()),
                })?;
                let input = params_json.map(|s| s.to_string());

                // Allow immediate cancel response while still forwarding cancellation to Host.
                // NOTE: This runs inside a per-request task (bridge spawns Req handling), so
                // awaiting here will not block the bridge message pump.
                let start = std::time::Instant::now();
                let (host_cancel_tx, host_cancel_rx) = tokio::sync::oneshot::channel();
                let mut host_fut = handler.call(lxapp, input, host_cancel_rx);

                let json_result: Result<String, RpcError> = tokio::select! {
                    biased;
                    res = &mut host_fut => {
                        match res {
                            Ok(json) => Ok(json),
                            Err(e) => {
                                // Host handlers are expected to return a cancel error when they
                                // observe `cancel`. Map that to BRIDGE_CANCELED so view callers
                                // can handle it consistently.
                                if matches!(&e, LxAppError::Bridge(msg) if msg == "Canceled") {
                                    Err(RpcError { code: BRIDGE_CANCELED.to_string(), message: None })
                                } else {
                                    Err(RpcError { code: BRIDGE_INTERNAL_ERROR.to_string(), message: Some(e.to_string()) })
                                }
                            }
                        }
                    }
                    _ = &mut cancel_rx => {
                        let _ = host_cancel_tx.send(());
                        Err(RpcError { code: BRIDGE_CANCELED.to_string(), message: None })
                    }
                };

                let elapsed = start.elapsed();
                if elapsed > std::time::Duration::from_secs(3) {
                    crate::warn!(
                        "[{}] host req '{}' slow: {:?}",
                        self.page.path(),
                        method,
                        elapsed
                    );
                }

                let json = json_result?;
                serde_json::from_str(&json).map_err(|e| RpcError {
                    code: BRIDGE_INTERNAL_ERROR.to_string(),
                    message: Some(e.to_string()),
                })
            }
            ServiceType::None => Err(RpcError {
                code: BRIDGE_METHOD_NOT_FOUND.to_string(),
                message: Some(format!("Method not found: {}", method)),
            }),
        }
    }

    async fn handle_notify(
        &self,
        method: &str,
        params_json: Option<&str>,
        service_type: ServiceType,
    ) {
        let ctx = self.get_ctx();

        let args_obj =
            params_json.and_then(|json| rong::JSObject::from_json_string(&ctx, json).ok());

        match service_type {
            ServiceType::JSFunc(js_func) => {
                let this_obj = self.this.clone();
                let method_name = method.to_string();
                let page_path = self.page.path().to_string();
                let task = async move {
                    let result = match args_obj {
                        Some(obj) => js_func.call_async::<_, ()>(Some(this_obj), (obj,)).await,
                        None => js_func.call_async::<_, ()>(Some(this_obj), ()).await,
                    };
                    if let Err(e) = result {
                        error!("[{}] notify '{}' failed: {}", page_path, method_name, e);
                    }
                };
                rong::spawn(task);
            }
            ServiceType::HostAPI(handler) => {
                let lxapp = match LxApp::from_ctx(&ctx) {
                    Ok(app) => app,
                    Err(e) => {
                        error!("notify '{}' missing LxApp: {}", method, e);
                        return;
                    }
                };
                let input = params_json.map(|s| s.to_string());
                let method_name = method.to_string();

                // Notify calls are fire-and-forget but must not spuriously "cancel" themselves.
                let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
                rong::spawn(async move {
                    let _keep_alive = cancel_tx;
                    if let Err(e) = handler.call(lxapp, input, cancel_rx).await {
                        error!("notify '{}' failed: {}", method_name, e);
                    }
                });
            }
            ServiceType::None => {}
        }
    }

    async fn handle_bridge_ready(&self) {
        let mut page_svc_clone = self.clone();
        let _ = page_svc_clone.handle_bridge_ready_internal().await;
    }

    fn expected_bridge_nonce(&self) -> Option<String> {
        self.page.bridge_nonce()
    }

    fn bridge_page_path(&self) -> Option<String> {
        Some(self.page.path())
    }

    fn is_cap_allowed(&self, _cap: &str) -> bool {
        // Capability control is delegated to host app, not lxapp config.
        // Host app should handle permission checks in its HostHandler implementations.
        true
    }

    async fn handle_state_ack(&self, _scope: Option<String>, rev: u64) {
        let mut state = self.state.lock().await;
        if let Some(cb) = state.state_callback.remove(&rev) {
            drop(state);
            let _ = cb.call::<_, ()>(None, ());
        }
    }
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, config: JSObject, path: String) -> JSResult<JSObject> {
        let lxapp = LxApp::from_ctx(&ctx)?;

        let page = lxapp.get_page(&path).ok_or_else(|| {
            RongJSError::from(HostError::new(
                rong::error::E_NOT_FOUND,
                format!("Page not found: {}", path),
            ))
        })?;

        let init_data = JSObject::new(&ctx);

        if let Ok(original_data) = config.get::<_, JSObject>("data") {
            init_data.set("data", original_data)?;
        } else {
            init_data.set("data", JSObject::new(&ctx))?;
        }

        // Cache capabilities
        let mut page_svc = PageSvc {
            functions: HashMap::new(),
            this: config.clone(),
            page,
            bridge: Bridge::new(),
            event_emitter: EventEmitter::default(),
            state: Rc::new(Mutex::new(PageSvcState {
                callback: HashMap::new(),
                state_callback: HashMap::new(),
                state_rev: 0,
                init_data: None,
            })),
        };

        page_svc.register_functions(&config)?;

        {
            let mut state = page_svc.state.try_lock().unwrap();
            state.init_data = Some(init_data);
        }

        let class = Class::get::<PageSvc>(&ctx).unwrap();
        let instance = class.instance(page_svc);

        let binding = instance.clone();
        let mut page_svc = binding.borrow_mut::<PageSvc>().unwrap();
        page_svc.this = instance.clone();

        super::with_page_svc_map(&ctx, |page_svc_map| {
            page_svc_map.borrow_mut().insert(path, page_svc.clone());
            Ok(())
        })?;

        Ok(instance)
    }

    #[js_method(rename = "_setData")]
    async fn set_data(&self, ops_json: String, callback: Optional<JSFunc>) -> JSResult<()> {
        let mut state = self.state.lock().await;

        if !self.bridge.is_ready() {
            return Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("Bridge of {} is not ready", self.page.path()),
            )));
        }

        let base_rev = state.state_rev;
        let new_rev = base_rev + 1;
        state.state_rev = new_rev;

        // Parse { ops: [...] } format from Page.js
        #[derive(Deserialize)]
        struct OpsWrapper {
            ops: Vec<JsonPatchOp>,
        }
        let wrapper: OpsWrapper = serde_json::from_str(&ops_json).map_err(|e| {
            RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
        })?;
        let ops = wrapper.ops;

        let ack = if let Some(cb) = callback.0 {
            state.state_callback.insert(new_rev, cb);
            Some(true)
        } else {
            None
        };

        drop(state);

        self.bridge
            .send_state_patch(self, None, base_rev, new_rev, ops, ack)
            .map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;

        Ok(())
    }

    #[js_method(rename = "getEventEmitter")]
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
        }
    }
}

impl PageSvc {
    fn register_functions(&mut self, obj: &JSObject) -> JSResult<()> {
        for key_value in obj.keys()? {
            if let Ok(function_name) = key_value.try_into::<String>() {
                if function_name.starts_with('_') {
                    continue;
                }
                if let Ok(func) = obj.get::<_, JSFunc>(function_name.as_str()) {
                    self.functions.insert(function_name.clone(), func);
                }
            }
        }
        Ok(())
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
            Err(RongJSError::from(HostError::new(
                rong::error::E_INTERNAL,
                format!("No page event handler: {}", event),
            )))
        }
    }

    async fn handle_bridge_ready_internal(&mut self) -> JSResult<()> {
        let mut state = self.state.lock().await;

        if let Some(init_data) = state.init_data.take() {
            // Extract the "data" field - this is the actual page data
            let page_data = init_data
                .get::<_, JSObject>("data")
                .unwrap_or_else(|_| JSObject::new(&self.this.get_ctx()));
            let data_json = page_data.json_stringify()?;
            let data_value: Value = serde_json::from_str(&data_json).map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;

            state.state_rev = 1;
            drop(state);

            self.bridge
                .send_state_snapshot(self, None, 1, data_value)
                .map_err(|e| {
                    RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
                })?;
        } else {
            drop(state);
        }

        self.page
            .dispatch_lifecycle_event(crate::PageLifecycleEvent::OnLoad);
        Ok(())
    }

    pub(crate) fn as_bridge(&self) -> &Bridge {
        &self.bridge
    }

    pub(crate) fn get_ctx(&self) -> JSContext {
        self.this.get_ctx()
    }
}

impl PageSvc {
    pub async fn create_in_ctx(ctx: &JSContext, path: &str) -> JSResult<()> {
        super::plugin::ensure_plugin_logic_loaded_for_page_path(ctx, path).await?;

        let create_page = ctx
            .global()
            .get::<_, JSFunc>("__CREATE_PAGE__")
            .map_err(|e| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })?;

        create_page
            .call::<_, ()>(None, (path.to_string(),))
            .map_err(|e: RongJSError| {
                RongJSError::from(HostError::new(rong::error::E_INTERNAL, e.to_string()))
            })
    }

    pub fn get_page(&self) -> Page {
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
                        "Page service not found",
                    ))
                })
        })
    }
}

fn get_current_pages(ctx: JSContext) -> JSResult<Vec<JSObject>> {
    let registry = ctx.global().get::<_, JSObject>("__PAGE_REGISTRY__")?;
    let lxapp = LxApp::from_ctx(&ctx)?;
    let paths = lxapp.get_page_stack();
    let mut pages = Vec::new();
    for p in paths {
        let obj = registry.get::<_, JSObject>(p.as_str())?;
        pages.push(obj);
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
