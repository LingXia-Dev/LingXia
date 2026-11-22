use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use rong::{
    Class, HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, Promise,
    function::Optional,
    function::{Rest, This},
    js_class, js_method,
};
use rong_event::{Emitter, EmitterExt, EventEmitter, EventKey};

/// Messages held for a side that has not subscribed yet; past this the newest are dropped.
const HELD_LIMIT: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Delivery {
    /// No subscriber yet: messages wait in `pending`.
    Hold,
    /// Subscribed, held messages not all delivered: new ones queue behind them.
    Drain,
    Direct,
}

#[js_class(clone)]
pub(crate) struct JSMessagePort {
    event_emitter: EventEmitter,
    peer: RefCell<Option<JSObject>>,
    pending: RefCell<VecDeque<JSValue>>,
    delivery: Cell<Delivery>,
    warned_overflow: Cell<bool>,
}

#[js_class]
impl JSMessagePort {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_NOT_SUPPORTED,
            "MessagePort cannot be directly constructed",
        )
        .with_name("TypeError")
        .into())
    }

    #[js_method(rename = "send")]
    fn post_message(this: This<JSObject>, payload: JSValue) -> JSResult<()> {
        emit_message(&this, payload)
    }

    #[js_method(rename = "onMessage")]
    fn on_message(
        this: This<JSObject>,
        handler: JSFunc,
        options: Optional<JSValue>,
    ) -> JSResult<JSFunc> {
        lxapp::lx::subscribe(&this.context(), options.0, || {
            add_message_listener(&this, handler)
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
        pending: RefCell::new(VecDeque::new()),
        delivery: Cell::new(Delivery::Hold),
        warned_overflow: Cell::new(false),
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
        if receiver.delivery.get() != Delivery::Direct {
            let mut pending = receiver.pending.borrow_mut();
            if pending.len() >= HELD_LIMIT {
                if !receiver.warned_overflow.replace(true) {
                    log::warn!(
                        "a page message port holds {HELD_LIMIT} messages nobody has subscribed to; dropping newer ones"
                    );
                }
                return Ok(());
            }
            pending.push_back(payload);
            return Ok(());
        }
    }

    <JSMessagePort as EmitterExt>::do_emit(This(peer), message_key(), Rest(vec![payload]))
        .map(|_| ())
}

pub(crate) fn add_message_listener(port: &JSObject, handler: JSFunc) -> JSResult<JSFunc> {
    let target = port.clone();
    let ctx = target.context();
    let handler_for_off = handler.clone();
    <JSMessagePort as EmitterExt>::add_event_listener(
        This(target.clone()),
        message_key(),
        handler,
        false,
        false,
    )?;
    let drain = {
        let port = target.borrow::<JSMessagePort>()?;
        match port.delivery.get() {
            Delivery::Hold if port.pending.borrow().is_empty() => {
                port.delivery.set(Delivery::Direct);
                false
            }
            Delivery::Hold => {
                port.delivery.set(Delivery::Drain);
                true
            }
            Delivery::Drain | Delivery::Direct => false,
        }
    };
    if drain {
        // After onMessage returns, so the handler can already hold its unsubscribe.
        let receiver = target.clone();
        Promise::from_future(&ctx, None, async move { drain_held(&receiver) })?;
    }
    JSFunc::new(&ctx, move || {
        <JSMessagePort as EmitterExt>::remove_event_listener(
            This(target.clone()),
            message_key(),
            handler_for_off.clone(),
        )
    })
}

/// Delivers held messages one at a time, so a message sent while draining
/// (a reply to one of them) queues behind the rest. With nobody left to
/// receive them, the rest wait for the next subscriber.
fn drain_held(receiver: &JSObject) -> JSResult<()> {
    loop {
        let payload = {
            let port = receiver.borrow::<JSMessagePort>()?;
            if !port.event_emitter.has_listeners(&message_key()) {
                port.delivery.set(Delivery::Hold);
                return Ok(());
            }
            let next = port.pending.borrow_mut().pop_front();
            match next {
                Some(payload) => payload,
                None => {
                    port.delivery.set(Delivery::Direct);
                    return Ok(());
                }
            }
        };
        <JSMessagePort as EmitterExt>::do_emit(
            This(receiver.clone()),
            message_key(),
            Rest(vec![payload]),
        )?;
    }
}

fn message_key() -> EventKey {
    EventKey::String("message".to_string())
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
        JSFunc::new(ctx, move |handler: JSFunc, options: Optional<JSValue>| {
            lxapp::lx::subscribe(&listen_port.context(), options.0, || {
                add_message_listener(&listen_port, handler)
            })
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

    /// A message sent after subscribing but before the held ones are out
    /// (a reply in the same turn) arrives after them.
    #[tokio::test(flavor = "current_thread")]
    async fn a_send_during_the_drain_keeps_its_place() {
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
                emit_message(&opener, JSValue::from_rust(&ctx, 3)).unwrap();

                tokio::task::yield_now().await;
                assert_eq!(*seen.borrow(), vec![1, 2, 3]);
            })
            .await;
    }

    /// Held messages survive a subscriber that leaves before they arrive.
    #[tokio::test(flavor = "current_thread")]
    async fn held_messages_wait_for_a_subscriber_that_stays() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let runtime = RongJS::runtime();
                let ctx = runtime.context();
                init(&ctx).unwrap();
                let (opener, page) = pair(&ctx).unwrap();
                let seen = std::rc::Rc::new(RefCell::new(Vec::<i32>::new()));

                emit_message(&opener, JSValue::from_rust(&ctx, 1)).unwrap();
                let first = JSFunc::new(&ctx, |_: i32| {}).unwrap();
                let off = add_message_listener(&page, first).unwrap();
                off.call::<_, ()>(None, ()).unwrap();
                tokio::task::yield_now().await;

                let sink = seen.clone();
                let handler =
                    JSFunc::new(&ctx, move |value: i32| sink.borrow_mut().push(value)).unwrap();
                add_message_listener(&page, handler).unwrap();
                tokio::task::yield_now().await;
                assert_eq!(*seen.borrow(), vec![1]);
            })
            .await;
    }

    /// A side that never subscribes holds a bounded number of messages.
    #[tokio::test(flavor = "current_thread")]
    async fn held_messages_are_capped() {
        tokio::task::LocalSet::new()
            .run_until(async {
                let runtime = RongJS::runtime();
                let ctx = runtime.context();
                init(&ctx).unwrap();
                let (opener, page) = pair(&ctx).unwrap();
                for n in 0..(HELD_LIMIT as i32 + 10) {
                    emit_message(&opener, JSValue::from_rust(&ctx, n)).unwrap();
                }
                let held = page
                    .borrow::<JSMessagePort>()
                    .unwrap()
                    .pending
                    .borrow()
                    .len();
                assert_eq!(held, HELD_LIMIT);
            })
            .await;
    }
}
