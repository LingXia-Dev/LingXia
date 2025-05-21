use super::bridge::{Bridge, BridgeTransport};
use crate::error::MiniAppError;
use crate::page::{Page, WebViewController};
use rong::{
    Class, JSContext, JSFunc, JSObject, JSResult, JSValue, RongJSError, Source, function::Optional,
    js_class, js_export, js_method,
};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Mutex;

// Page is Send able, but JSFunc is not, we can not let Page hold PageSvc.
#[js_export]
pub(crate) struct PageSvc {
    functions: HashMap<String, JSFunc>,
    this: JSObject,

    // use Option for late initialization
    page: Option<Page>,
    bridge: Option<Bridge>,

    // state of PageSvc
    state: Rc<Mutex<PageSvcState>>,
}

struct PageSvcState {
    // service function for callback type in bridge
    callback: HashMap<String, JSFunc>,
    callbackid: AtomicUsize,
    init_data: Option<JSObject>,
}

impl BridgeTransport for PageSvc {
    fn post_message_to_view(&self, message_json: &str) -> Result<(), MiniAppError> {
        self.page.as_ref().unwrap().post_message(message_json)
    }

    fn has_service(&self, service_name: &str) -> bool {
        self.functions.contains_key(service_name)
    }
}

#[js_class]
impl PageSvc {
    #[js_method(constructor)]
    fn _new(ctx: JSContext, obj: JSObject) -> JSResult<JSObject> {
        // Get the PageSvc class
        let page_class = Class::get::<PageSvc>(&ctx)?;

        let init_data = obj.get::<_, JSObject>("data").ok();

        let mut page_svc = PageSvc {
            functions: HashMap::new(),
            this: obj.clone(),
            page: None,
            bridge: None,
            state: Rc::new(Mutex::new(PageSvcState {
                callback: HashMap::new(),
                callbackid: AtomicUsize::new(0),
                init_data,
            })),
        };

        // Extract all functions from the object
        page_svc.assign_funcs(&obj)?;

        let page = page_class.instance(page_svc);

        // bind the object to member of 'this'
        page.borrow_mut::<PageSvc>().unwrap().this = page.clone();
        Ok(page)
    }

    #[js_method(rename = "_setData")]
    async fn set_data(&mut self, data: String, callback: Optional<JSFunc>) -> JSResult<()> {
        self.as_bridge()
            .set_data(&data)
            .await
            .map_err(|e| RongJSError::Error(e.to_string()))?;

        // Call the callback if provided
        if let Some(cb) = callback.0 {
            let mut state = self.state.lock().await;
            let counter = state.callbackid.fetch_add(1, Ordering::SeqCst);
            let callbackid = format!("setData-{}", counter);
            state.callback.insert(callbackid, cb);
        }

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

    // handler for bridge type: call or event
    pub async fn call_or_event(
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
    pub async fn callback(&mut self, callbackid: &str) -> JSResult<()> {
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
    pub async fn post_init_data(&mut self) -> JSResult<()> {
        let mut state = self.state.lock().await;

        // only post one time
        if let Some(data) = state.init_data.take() {
            drop(state);
            self.as_bridge()
                .set_data(&data.json_stringify()?)
                .await
                .map_err(|e| RongJSError::Error(e.to_string()))?;
        }
        Ok(())
    }

    // attach Page to PageSvc and init its bridge
    pub fn attach_page(&mut self, page: Page) {
        self.page = Some(page);
        let bridge = Bridge::new(Rc::new(self.clone()));
        self.bridge = Some(bridge);
    }

    pub fn as_bridge(&self) -> &Bridge {
        self.bridge.as_ref().unwrap()
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_class::<PageSvc>()?;

    let page_js = Source::from_bytes(include_str!("scripts/Page.js"));
    ctx.eval::<()>(page_js)?;

    use log;
    ctx.global().set(
        "print",
        JSFunc::new(ctx, |msg: String| log::info!("{}", msg)),
    )?;

    ctx.eval::<()>(Source::from_bytes(
        r#"
                    const console={
                        log: function(...args){
                            print(args.join(' '))
                        },
                        error: function(...args){
                            print(args.join(' '))
                        }
                    }
                "#,
    ))?;

    Ok(())
}
