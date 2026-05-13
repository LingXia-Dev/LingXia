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
        emit_message(&this, payload)
    }

    #[js_method(rename = "onMessage")]
    fn on_message(this: This<JSObject>, handler: JSFunc) -> JSResult<JSFunc> {
        add_message_listener(&this, handler)
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

    attach_methods(ctx, &first)?;
    attach_methods(ctx, &second)?;

    Ok((first, second))
}

pub(crate) fn emit_message(port: &JSObject, payload: JSValue) -> JSResult<()> {
    let peer = {
        let port = port.borrow::<JSMessagePort>()?;
        port.peer.borrow().clone()
    };
    let Some(peer) = peer else {
        return Ok(());
    };

    <JSMessagePort as EmitterExt>::do_emit(
        This(peer),
        EventKey::String("message".to_string()),
        Rest(vec![payload]),
    )
    .map(|_| ())
}

pub(crate) fn add_message_listener(port: &JSObject, handler: JSFunc) -> JSResult<JSFunc> {
    let target = port.clone();
    let ctx = target.context();
    let handler_for_off = handler.clone();
    <JSMessagePort as EmitterExt>::add_event_listener(
        This(target.clone()),
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

fn attach_methods(ctx: &JSContext, port: &JSObject) -> JSResult<()> {
    let post_port = port.clone();
    port.set(
        "postMessage",
        JSFunc::new(ctx, move |payload: JSValue| {
            emit_message(&post_port, payload)
        })?,
    )?;

    let listen_port = port.clone();
    port.set(
        "onMessage",
        JSFunc::new(ctx, move |handler: JSFunc| {
            add_message_listener(&listen_port, handler)
        })?,
    )?;

    Ok(())
}
