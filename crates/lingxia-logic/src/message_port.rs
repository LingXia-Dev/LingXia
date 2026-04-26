use std::cell::RefCell;

use rong::{
    Class, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue,
    function::{Rest, This},
    js_class, js_export, js_method,
};
use rong_event::{Emitter, EmitterExt, EventEmitter, EventKey};

#[js_export]
pub(crate) struct JSMessagePort {
    event_emitter: EventEmitter,
    peer: RefCell<Option<JSObject>>,
}

#[js_class]
impl JSMessagePort {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "MessagePort cannot be directly constructed",
        )
        .into())
    }

    #[js_method(rename = "postMessage")]
    fn post_message(this: This<JSObject>, payload: JSValue) -> JSResult<()> {
        let peer = {
            let port = (*this).borrow::<Self>()?;
            port.peer.borrow().clone()
        };
        let Some(peer) = peer else {
            return Ok(());
        };
        <Self as EmitterExt>::do_emit(
            This(peer),
            EventKey::String("message".to_string()),
            Rest(vec![payload]),
        )?;
        Ok(())
    }

    #[js_method(rename = "onMessage")]
    fn on_message(this: This<JSObject>, handler: JSFunc) -> JSResult<JSFunc> {
        let target = (*this).clone();
        let ctx = target.context();
        let handler_for_off = handler.clone();
        <Self as EmitterExt>::add_event_listener(
            this,
            EventKey::String("message".to_string()),
            handler,
            false,
            false,
        )?;
        JSFunc::new(&ctx, move || {
            <JSMessagePort as EmitterExt>::remove_event_listener(
                This(target.clone()),
                EventKey::String("message".to_string()),
                handler_for_off.clone(),
            )
        })
    }

    #[js_method(gc_mark)]
    fn gc_mark_with<F>(&self, mut mark_fn: F)
    where
        F: FnMut(&JSValue),
    {
        if let Some(peer) = self.peer.borrow().as_ref() {
            mark_fn(peer.as_js_value());
        }
        self.event_emitter.gc_mark_with(mark_fn);
    }
}

impl Emitter for JSMessagePort {
    fn get_event_emitter(&self) -> EventEmitter {
        self.event_emitter.clone()
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    ctx.register_hidden_class::<JSMessagePort>()
}

pub(crate) fn pair(ctx: &JSContext) -> JSResult<(JSObject, JSObject)> {
    let first = Class::lookup::<JSMessagePort>(ctx)?.instance(JSMessagePort {
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
    });
    let second = Class::lookup::<JSMessagePort>(ctx)?.instance(JSMessagePort {
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
    });

    first
        .borrow::<JSMessagePort>()?
        .peer
        .replace(Some(second.clone()));
    second
        .borrow::<JSMessagePort>()?
        .peer
        .replace(Some(first.clone()));

    Ok((first, second))
}
