use super::bridge::{
    Bridge, DispatchMessage, DispatchMessageType, MessageHandler, MessageTransport, ServiceType,
};
use super::lx;
use crate::error::MiniAppError;
use crate::miniapp::MiniApp;
use crate::page::Page;
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, function::Optional,
    js_class, js_export, js_method,
};
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
    fast_api: Rc<lx::FastJSApi>,

    page: Page,
    bridge: Bridge,

    // state of PageSvc
    state: Rc<Mutex<PageSvcState>>,
}

struct PageSvcState {
    // service function for callback type in bridge
    callback: HashMap<String, JSFunc>,
    callbackid: AtomicUsize,
    init_data: Option<JSObject>,
    bridge_rdy: bool,
}

impl MessageTransport for PageSvc {
    fn post_message_to_view(&self, message_json: String) -> Result<(), MiniAppError> {
        self.page.webview_controller().post_message(message_json)
    }
}

impl MessageHandler for PageSvc {
    fn get_service_type(&self, service_name: &str) -> ServiceType {
        // Check regular JS functions first
        if self.functions.contains_key(service_name) {
            return ServiceType::JSFunc;
        }

        // Check FastJSApi (also treated as JSFunc for now)
        if self.fast_api.has_fast_api(service_name) {
            return ServiceType::JSFunc;
        }

        ServiceType::None
    }

    async fn handle_message(&self, dispatch_msg: DispatchMessage, service_type: ServiceType) {
        match dispatch_msg.message_type() {
            DispatchMessageType::Call {
                name, payload: _, ..
            }
            | DispatchMessageType::Event { name, payload: _ } => {
                // handle LXPortRdy event without spawn_local task
                if name == "LXPortRdy" {
                    let mut page_svc_clone = self.clone();
                    let _ = page_svc_clone.handle_lxport_ready().await;
                    return;
                }

                let ctx = self.get_ctx();

                // Extract values before moving into task
                let name_owned = name.clone();
                let dispatch_msg_clone = dispatch_msg.clone();
                let page_svc_clone = self.clone();

                // Handle different service types
                match service_type {
                    ServiceType::JSFunc => {
                        // For JS functions, execute in JS context
                        let task = async move {
                            if let Err(e) = page_svc_clone
                                .call_or_event_from_view(&ctx, &dispatch_msg_clone)
                                .await
                            {
                                crate::error!(
                                    "JS function call/event '{}' failed: {}",
                                    name_owned,
                                    e
                                );
                            }
                        };
                        tokio::task::spawn_local(task);
                    }
                    ServiceType::FastAPI => {
                        todo!();
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
                        crate::error!("No callback handler: {}, Error: {}", callback_id_owned, e);
                    }
                };
                tokio::task::spawn_local(task);
            }
        }
    }
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, config: JSObject, path: String) -> JSResult<JSObject> {
        let init_data = config.get::<_, JSObject>("data").ok();
        let fast_api = ctx
            .get_user_data::<Rc<lx::FastJSApi>>()
            .ok_or_else(|| RongJSError::Error("FastJSApi not found in context".to_string()))?
            .clone();

        let miniapp = ctx.get_user_data::<Arc<MiniApp>>().unwrap();

        // Get the page from MiniApp
        let page = miniapp
            .get_page(&path)
            .ok_or_else(|| RongJSError::Error(format!("Page not found: {}", path)))?;

        let mut page_svc = PageSvc {
            functions: HashMap::new(),
            this: config.clone(), // will be updated later
            fast_api,
            page,
            bridge: Bridge::new(),
            state: Rc::new(Mutex::new(PageSvcState {
                callback: HashMap::new(),
                callbackid: AtomicUsize::new(0),
                init_data,
                bridge_rdy: false,
            })),
        };

        // Extract functions from page config
        page_svc.assign_funcs(&config)?;

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
        if !state.bridge_rdy {
            return Err(RongJSError::Error(
                "View Bridge is not ready to receive data".to_string(),
            ));
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
    fn assign_funcs(&mut self, obj: &JSObject) -> JSResult<()> {
        for key_value in obj.keys()? {
            // obj.keys() returns JSValue, not String
            if let Ok(key_string) = key_value.try_into::<String>() {
                if let Ok(func) = obj.get::<_, JSFunc>(key_string.as_str()) {
                    self.functions.insert(key_string, func);
                }
            }
        }
        Ok(())
    }

    // handler for bridge type: call or event from view
    pub(crate) async fn call_or_event_from_view(
        &self,
        ctx: &JSContext,
        dispatch_msg: &DispatchMessage,
    ) -> JSResult<()> {
        // Extract function name and args from dispatch message
        let (func_name, args) = match dispatch_msg.message_type() {
            DispatchMessageType::Call { name, payload, .. }
            | DispatchMessageType::Event { name, payload } => (name.as_str(), payload.as_deref()),
            DispatchMessageType::Callback { .. } => {
                // This should never happen as callbacks are handled separately in appservice.rs
                return Ok(());
            }
        };

        // For Call messages, reply success first
        if matches!(
            dispatch_msg.message_type(),
            DispatchMessageType::Call { .. }
        ) {
            if self.fast_api.has_fast_api(func_name) {
                // Handle fast API call
                match self
                    .fast_api
                    .call_fast_api(ctx, func_name, args, self.this.clone())
                    .await
                {
                    Ok(js_value) => {
                        if js_value.is_null() || js_value.is_undefined() {
                            let _ = dispatch_msg.reply_success(self, None);
                        } else if let Some(js_object) = js_value.into_object() {
                            match js_object.json_stringify() {
                                Ok(json_string) => {
                                    let _ = dispatch_msg.reply_success(self, Some(&json_string));
                                }
                                Err(_) => {
                                    let _ = dispatch_msg
                                        .reply_failure(self, "Failed to stringify object");
                                }
                            }
                        } else {
                            let _ = dispatch_msg.reply_failure(self, "Invalid result type");
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        let _ = dispatch_msg.reply_failure(self, &e.to_string());
                        return Ok(());
                    }
                }
            }
        }

        self.call_function_internal(ctx, func_name, args).await
    }

    // handler for bridge type: call or event from native
    pub(crate) async fn call_or_event_from_native(
        &self,
        ctx: &JSContext,
        func_name: &str,
        args: Option<&str>,
    ) -> JSResult<()> {
        self.call_function_internal(ctx, func_name, args).await
    }

    // Internal method to actually call the function
    async fn call_function_internal(
        &self,
        ctx: &JSContext,
        func_name: &str,
        args: Option<&str>,
    ) -> JSResult<()> {
        if let Some(func) = self.functions.get(func_name) {
            let args = args.and_then(|json| JSObject::from_json_string(ctx, json).ok());
            match args {
                Some(obj) => {
                    func.call_async::<_, ()>(Some(self.this.clone()), (obj,))
                        .await?;
                }
                None => {
                    func.call_async::<_, ()>(Some(self.this.clone()), ())
                        .await?;
                }
            };
            return Ok(());
        }
        Err(RongJSError::Error(format!("No service: {}", func_name)))
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
            state.bridge_rdy = true;

            drop(state);
            self.bridge
                .set_data(self, &data.json_stringify()?, None)
                .await
                .map_err(|e| RongJSError::Error(e.to_string()))?;
        } else {
            state.bridge_rdy = true;
        }
        Ok(())
    }

    pub(crate) fn as_bridge(&self) -> &Bridge {
        &self.bridge
    }

    pub fn get_ctx(&self) -> JSContext {
        self.this.get_ctx()
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<PageSvc>()?;

    let page_js = Source::from_bytes(include_str!("scripts/Page.js"));
    ctx.eval::<()>(page_js)?;

    Ok(())
}
