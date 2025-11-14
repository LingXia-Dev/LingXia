use super::bridge::{
    Bridge, DispatchMessage, DispatchMessageType, MessageHandler, MessageTransport, ServiceType,
};
use crate::PageServiceEvent;
use crate::error;
use crate::error::LxAppError;
use crate::lx::fastapi::get_fast_api;
use crate::lxapp::LxApp;
use crate::page::Page;
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, function::Optional,
    js_class, js_export, js_method,
};
use rong_modules::event::EventEmitter;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Mutex;

// Page is Send able, but JSFunc is not, we can not let Page hold PageSvc.
#[js_export]
pub(crate) struct PageSvc {
    functions: HashMap<String, JSFunc>,
    this: JSObject,

    pub(crate) page: Page,
    bridge: Bridge,

    event_emitter: EventEmitter,

    // state of PageSvc
    state: Rc<Mutex<PageSvcState>>,
}

struct PageSvcState {
    // service function for callback type in bridge
    callback: HashMap<String, JSFunc>,
    callbackid: AtomicUsize,
    init_data: Option<JSObject>,
}

impl MessageTransport for PageSvc {
    fn post_message_to_view(&self, message_json: String) -> Result<(), LxAppError> {
        if let Some(controller) = self.page.webview_controller() {
            controller
                .post_message(message_json)
                .map_err(|e| LxAppError::WebView(e.to_string()))
        } else {
            Err(LxAppError::WebView(
                "WebView not ready for message posting".to_string(),
            ))
        }
    }
}

impl MessageHandler for PageSvc {
    fn get_service_type(&self, service_name: &str) -> ServiceType {
        // Check if it's a FastAPI call with lx. prefix
        if let Some(api_name) = service_name.strip_prefix("lx.") {
            if let Some(handler) = get_fast_api(api_name) {
                return ServiceType::FastAPI(handler);
            }
        }

        // Check page-specific JS functions
        if let Some(js_func) = self.functions.get(service_name) {
            return ServiceType::JSFunc(js_func.clone());
        }

        ServiceType::None
    }

    async fn handle_message(&self, dispatch_msg: DispatchMessage, service_type: ServiceType) {
        match dispatch_msg.message_type() {
            DispatchMessageType::Call {
                name, payload: _, ..
            }
            | DispatchMessageType::Event { name, payload: _ } => {
                let ctx = self.get_ctx();

                // Extract values before moving into task
                let name_owned = name.clone();
                let page_svc_clone = self.clone();

                // Pre-extract parameters to avoid duplication
                let (func_name, args) = match dispatch_msg.message_type() {
                    DispatchMessageType::Call { name, payload, .. }
                    | DispatchMessageType::Event { name, payload } => {
                        (name.as_str(), payload.as_deref())
                    }
                    _ => unreachable!(), // We're in Call/Event branch
                };

                let _func_name_owned = func_name.to_string();
                let args_owned = args.map(|s| s.to_string());

                // Handle different service types
                match service_type {
                    ServiceType::JSFunc(js_func) => {
                        // Build args object once
                        let args_obj = args_owned
                            .as_deref()
                            .and_then(|json| rong::JSObject::from_json_string(&ctx, json).ok());
                        // Priority & coalescing
                        let (prio, dedup) = match dispatch_msg.message_type() {
                            DispatchMessageType::Event { name, .. } => (
                                rong::JsInvokePriority::Event,
                                Some(format!("page:{}:{}", page_svc_clone.page.path(), name)),
                            ),
                            _ => (rong::JsInvokePriority::Normal, None),
                        };
                        // Enqueue (non-blocking); Bridge for Call already replied success
                        if let Err(e) = rong::enqueue_js_invoke(
                            &ctx,
                            js_func.clone(),
                            Some(page_svc_clone.this.clone()),
                            args_obj,
                            prio,
                            dedup,
                            false,
                        )
                        .await
                        {
                            error!("JS invocation '{}' failed: {}", name_owned, e);
                        }
                    }
                    ServiceType::FastAPI(handler) => {
                        // For FastAPI, handle directly and reply
                        let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap().clone();

                        match handler.call(lxapp, args) {
                            Ok(result) => {
                                // Reply with the result for Call messages
                                if matches!(
                                    dispatch_msg.message_type(),
                                    DispatchMessageType::Call { .. }
                                ) {
                                    let _ = dispatch_msg.reply_success(self, Some(&result));
                                }
                            }
                            Err(e) => {
                                error!("Fast API call '{}' failed: {}", name, e);
                                // Reply with error for Call messages
                                if matches!(
                                    dispatch_msg.message_type(),
                                    DispatchMessageType::Call { .. }
                                ) {
                                    let _ = dispatch_msg.reply_failure(self, &e.to_string());
                                }
                            }
                        }
                    }
                    ServiceType::None => {
                        // This shouldn't happen for Call/Event messages
                    }
                }
            }
            DispatchMessageType::Callback { callback_id } => {
                // Callbacks are always handled the same way regardless of service_type
                let callback_id_owned = callback_id.clone();
                let mut page_svc_clone = self.clone();

                let task = async move {
                    if let Err(e) = page_svc_clone.callback(&callback_id_owned).await {
                        error!("No callback handler: {}, Error: {}", callback_id_owned, e);
                    }
                };
                rong::spawn(task);
            }
        }
    }

    async fn handle_bridge_ready(&self) {
        let mut page_svc_clone = self.clone();
        let _ = page_svc_clone.handle_lxport_ready().await;
    }
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, config: JSObject, path: String) -> JSResult<JSObject> {
        let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

        // Get the page from LxApp
        let page = lxapp
            .get_page(&path)
            .ok_or_else(|| RongJSError::Error(format!("Page not found: {}", path)))?;

        let init_data = JSObject::new(&ctx);

        if let Ok(original_data) = config.get::<_, JSObject>("data") {
            init_data.set("data", original_data)?;
        } else {
            init_data.set("data", JSObject::new(&ctx))?;
        }

        let mut page_svc = PageSvc {
            functions: HashMap::new(),
            this: config.clone(), // will be updated later
            page,
            bridge: Bridge::new(),
            event_emitter: EventEmitter::default(),
            state: Rc::new(Mutex::new(PageSvcState {
                callback: HashMap::new(),
                callbackid: AtomicUsize::new(0),
                init_data: None,
            })),
        };

        // Register all functions
        page_svc.register_functions(&config)?;

        // Store the complete init data
        {
            let mut state = page_svc.state.try_lock().unwrap();
            state.init_data = Some(init_data);
        }

        let class = Class::get::<PageSvc>(&ctx).unwrap();
        let instance = class.instance(page_svc);

        let binding = instance.clone();
        let mut page_svc = binding.borrow_mut::<PageSvc>().unwrap();
        page_svc.this = instance.clone();

        // Register the PageSvc in the JSContext HashMap
        if let Some(page_svc_map) = ctx.get_user_data::<Rc<RefCell<HashMap<String, PageSvc>>>>() {
            page_svc_map.borrow_mut().insert(path, page_svc.clone());
        }

        Ok(instance)
    }

    #[js_method(rename = "_setData")]
    async fn set_data(&mut self, data: String, callback: Optional<JSFunc>) -> JSResult<()> {
        let mut state = self.state.lock().await;

        // Check if bridge is ready
        if !self.bridge.is_ready() {
            return Err(RongJSError::Error(format!(
                "Bridge of {} is not ready to receive data",
                self.page.path()
            )));
        }

        // If we have a callback, register it and get a callback ID
        let callback_id = if let Some(cb) = callback.0 {
            let counter = state.callbackid.fetch_add(1, Ordering::SeqCst);
            let callbackid = format!("setData-{}", counter);
            state.callback.insert(callbackid.clone(), cb);
            Some(callbackid)
        } else {
            None
        };

        // Use the set_data method with optional callback_id
        self.bridge
            .set_data(self, &data, callback_id)
            .await
            .map_err(|e| RongJSError::Error(e.to_string()))?;

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
            mark_fn(func.as_jsvalue());
        }
        mark_fn(self.this.as_jsvalue());

        if let Ok(state) = self.state.try_lock() {
            for (_, func) in state.callback.iter() {
                mark_fn(func.as_jsvalue());
            }
        }
    }
}

impl PageSvc {
    /// Register all functions from page config
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

    // handler for bridge type: call or event from native
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
        Err(RongJSError::Error(format!("No service: {}", func_name)))
    }

    // Typed page event caller using PageServiceEvent
    pub(crate) async fn call_page_event(
        &self,
        ctx: &JSContext,
        event: PageServiceEvent,
        args: Option<&str>,
    ) -> JSResult<()> {
        if let Some(js_func) = self.functions.get(event.as_str()) {
            let args_obj = args.and_then(|json| rong::JSObject::from_json_string(ctx, json).ok());
            // Enqueue as Normal priority, no dedup, fire-and-forget
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
            Err(RongJSError::Error(format!(
                "No page event handler: {}",
                event
            )))
        }
    }

    // handler for bridge type: callback
    async fn callback(&mut self, callbackid: &str) -> JSResult<()> {
        let mut state = self.state.lock().await;
        if let Some(callback) = state.callback.remove(callbackid) {
            // Release the lock before calling the callback to avoid potential deadlocks
            drop(state);
            return callback.call::<_, ()>(None, ());
        }

        Err(RongJSError::Error(format!(
            "No callback handler for {}",
            callbackid
        )))
    }

    // post init data to view
    async fn handle_lxport_ready(&mut self) -> JSResult<()> {
        let mut state = self.state.lock().await;

        // only post one time
        if let Some(data) = state.init_data.take() {
            self.bridge
                .set_data(self, &data.json_stringify()?, None)
                .await
                .map_err(|e| RongJSError::Error(e.to_string()))?;
        }
        // Notify native Page that bridge is ready so it can dispatch onLoad at the right time
        self.page.notify_bridge_ready();
        Ok(())
    }

    pub(crate) fn as_bridge(&self) -> &Bridge {
        &self.bridge
    }

    pub(crate) fn get_ctx(&self) -> JSContext {
        self.this.get_ctx()
    }
}

impl Page {
    pub fn get_event_emitter(&self, ctx: &JSContext) -> JSResult<EventEmitter> {
        // Use JSContext registry as the single source of truth for PageSvc binding
        let registry = ctx
            .get_user_data::<Rc<RefCell<HashMap<String, PageSvc>>>>()
            .ok_or_else(|| RongJSError::Error("Page service registry not available".to_string()))?;

        registry
            .borrow()
            .get(self.path().as_str())
            .map(|svc| svc.get_event_emitter())
            .ok_or_else(|| RongJSError::Error("Page service not found after creation".to_string()))
    }
}

impl LxApp {
    pub fn create_page_with_ctx(&self, ctx: &JSContext, path: &str) -> JSResult<Page> {
        // This method is responsible for JS PageSvc creation only; it should not create native Page.
        // Expect the native Page to have been created by the caller on the native side already.
        let page = self
            .get_page(path)
            .ok_or_else(|| RongJSError::Error(format!("Page not found: {}", path)))?;

        // If PageSvc already exists in JSContext registry, return the page directly (idempotent)
        if let Some(registry) = ctx.get_user_data::<Rc<RefCell<HashMap<String, PageSvc>>>>() {
            if registry.borrow().contains_key(path) {
                return Ok(page);
            }
        }

        let create_page = ctx
            .global()
            .get::<_, JSFunc>("__CREATE_PAGE__")
            .map_err(|e| RongJSError::Error(e.to_string()))?;

        create_page
            .call::<_, ()>(None, (path.to_string(),))
            .map_err(|e| RongJSError::Error(e.to_string()))?;

        Ok(page)
    }
}

fn get_current_pages(ctx: JSContext) -> JSResult<Vec<JSObject>> {
    let registry = ctx.global().get::<_, JSObject>("__PAGE_REGISTRY__")?;
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
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
