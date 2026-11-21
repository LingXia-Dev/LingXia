use std::cell::{Cell, RefCell};

use rong::{
    Class, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise,
    function::{Rest, This},
    js_class, js_method,
};
use rong_event::{Emitter, EmitterExt, EventEmitter, EventKey};

#[js_class(clone)]
pub(crate) struct JSMessagePort {
    event_emitter: EventEmitter,
    peer: RefCell<Option<JSObject>>,
    /// Messages sent before this side first subscribed, in order.
    pending: RefCell<Vec<JSValue>>,
    subscribed: Cell<bool>,
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

    #[js_method(rename = "send")]
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
        for payload in self.pending.borrow().iter() {
            mark_fn(payload);
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

fn port() -> JSMessagePort {
    JSMessagePort {
        event_emitter: EventEmitter::default(),
        peer: RefCell::new(None),
        pending: RefCell::new(Vec::new()),
        subscribed: Cell::new(false),
    }
}

pub(crate) fn pair(ctx: &JSContext) -> JSResult<(JSObject, JSObject)> {
    let first = Class::lookup::<JSMessagePort>(ctx)?.instance(port());
    let second = Class::lookup::<JSMessagePort>(ctx)?.instance(port());

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
    {
        let receiver = peer.borrow::<JSMessagePort>()?;
        if !receiver.subscribed.get() {
            receiver.pending.borrow_mut().push(payload);
            return Ok(());
        }
    }

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
    let queued = {
        let port = target.borrow::<JSMessagePort>()?;
        if port.subscribed.replace(true) {
            Vec::new()
        } else {
            port.pending.take()
        }
    };
    if !queued.is_empty() {
        // After onMessage returns, so the handler can already hold its unsubscribe.
        let receiver = target.clone();
        Promise::from_future(&ctx, None, async move {
            for payload in queued {
                <JSMessagePort as EmitterExt>::do_emit(
                    This(receiver.clone()),
                    EventKey::String("message".to_string()),
                    Rest(vec![payload]),
                )?;
            }
            Ok::<(), rong::RongJSError>(())
        })?;
    }
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
        "send",
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

#[cfg(test)]
mod tests {
    use super::*;
    use rong::{JSEngine, RongJS};

    /// A message sent before the other side subscribes waits for it, arrives
    /// after onMessage returns, and keeps its order ahead of later sends.
    #[tokio::test(flavor = "current_thread")]
    async fn messages_wait_for_the_first_subscriber() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let runtime = RongJS::runtime();
                let ctx = runtime.context();
                init(&ctx).unwrap();
                let (opener, page) = pair(&ctx).unwrap();
                let seen = std::rc::Rc::new(RefCell::new(Vec::<i32>::new()));

                emit_message(&opener, JSValue::from_rust(&ctx, 1)).unwrap();
                emit_message(&opener, JSValue::from_rust(&ctx, 2)).unwrap();
                let sink = seen.clone();
                let handler =
                    JSFunc::new(&ctx, move |value: i32| sink.borrow_mut().push(value)).unwrap();
                add_message_listener(&page, handler).unwrap();
                assert!(
                    seen.borrow().is_empty(),
                    "queued messages arrive after onMessage returns"
                );

                tokio::task::yield_now().await;
                assert_eq!(*seen.borrow(), vec![1, 2]);
                emit_message(&opener, JSValue::from_rust(&ctx, 3)).unwrap();
                assert_eq!(*seen.borrow(), vec![1, 2, 3]);
            })
            .await;
    }
}
