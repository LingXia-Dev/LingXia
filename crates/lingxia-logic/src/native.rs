//! Logic runtime native route dispatch (`globalThis.__lingxiaNative`).
//!
//! Backs `.lingxia/native.logic.ts` (`import { native } from '@lingxia/native'` and `page.native`):
//! - `invoke(route, input?, pageId?)` -> `Promise<R>`
//! - `invokeSync(route, input?, pageId?)` -> `R` (runs directly on the Logic JS thread)
//! - `stream(route, input?, options?, pageId?)` -> `NativeTask<E, R>` (async iterable + `result` + `cancel()`)
//! - `channel(route, input?, pageId?)` -> `Promise<NativeChannel<In, Out>>`
//! - `clearPageState(pageId)` -> cancels in-flight `page.native` calls & drops `PageState<T>`
//!
//! Authority is always derived from `LxApp::from_ctx(&ctx)` and checked against each route's
//! `RouteAudience` policy; callers cannot spoof session authority or another lxapp's page id.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use lxapp::host::{self, ChannelContext, HostInvocationContext, HostOutput, HostStreamItem};
use rong::function::Optional;
use rong::{HostError, JSContext, JSFunc, JSObject, JSResult, JSValue, Source};
use tokio::sync::mpsc;

use crate::i18n::{js_error_from_lxapp_error, js_internal_error, js_invalid_parameter_error};

fn js_value_to_json(ctx: &JSContext, value: Option<JSValue>) -> JSResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_undefined() {
        return Ok(None);
    }
    let json_global: JSObject = ctx.global().get("JSON")?;
    let stringify: JSFunc = json_global.get("stringify")?;
    let out: Option<String> = stringify
        .call(None, (value,))
        .map_err(|e| js_invalid_parameter_error(format!("native input is not JSON: {e}")))?;
    Ok(out)
}

fn json_to_js_value(ctx: &JSContext, json: Option<&str>) -> JSResult<JSValue> {
    let Some(json) = json else {
        return Ok(JSValue::undefined(ctx));
    };
    if json == "null" || json.is_empty() {
        let json_global: JSObject = ctx.global().get("JSON")?;
        let parse: JSFunc = json_global.get("parse")?;
        return if json.is_empty() {
            Ok(JSValue::undefined(ctx))
        } else {
            parse.call(None, (json,))
        };
    }
    let json_global: JSObject = ctx.global().get("JSON")?;
    let parse: JSFunc = json_global.get("parse")?;
    parse.call(None, (json,))
}

async fn invoke(
    ctx: JSContext,
    route: String,
    input: Optional<JSValue>,
    page_id: Optional<String>,
) -> JSResult<JSValue> {
    let invocation = HostInvocationContext::for_logic_context_with_page(&ctx, page_id.0)?;
    let handler = host::lookup_host_for_caller(&route, invocation.caller()).ok_or_else(|| {
        HostError::new(
            rong::error::E_NOT_FOUND,
            format!("native route '{route}' is not registered or not authorized"),
        )
    })?;
    let payload = js_value_to_json(&ctx, input.0)?;
    let (_cancel_tx, cancel_rx, _guard) = host::track_page_call(invocation.page_id());
    let output = handler
        .call(invocation, payload, cancel_rx)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    match output {
        HostOutput::Json(json) => json_to_js_value(&ctx, Some(&json)),
        HostOutput::Stream(_) => Err(js_internal_error(format!(
            "native route '{route}' returned a stream; call it as a stream route"
        ))),
    }
}

fn invoke_sync(
    ctx: JSContext,
    route: String,
    input: Optional<JSValue>,
    page_id: Optional<String>,
) -> JSResult<JSValue> {
    let invocation = HostInvocationContext::for_logic_context_with_page(&ctx, page_id.0)?;
    let payload = js_value_to_json(&ctx, input.0)?;
    let output = host::dispatch_host_call_sync(&route, invocation, payload)
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    match output {
        HostOutput::Json(json) => json_to_js_value(&ctx, Some(&json)),
        HostOutput::Stream(_) => Err(js_internal_error(format!(
            "sync native route '{route}' cannot return a stream"
        ))),
    }
}

fn clear_page_state(_ctx: JSContext, page_id: String) -> JSResult<()> {
    if !page_id.is_empty() {
        host::clear_page_state(&page_id);
    }
    Ok(())
}

/// Starts a `#[lingxia::native(..., stream)]` route and returns a standard `NativeTask<E, R>`
/// (async iterable yielding events and returning the final result, plus `.result` and `.cancel()`).
fn stream(
    ctx: JSContext,
    route: String,
    input: Optional<JSValue>,
    options: Optional<JSObject>,
    page_id: Optional<String>,
) -> JSResult<JSObject> {
    let invocation = HostInvocationContext::for_logic_context_with_page(&ctx, page_id.0)?;
    let handler = host::lookup_host_for_caller(&route, invocation.caller()).ok_or_else(|| {
        HostError::new(
            rong::error::E_NOT_FOUND,
            format!("native stream route '{route}' is not registered or not authorized"),
        )
    })?;
    let payload = js_value_to_json(&ctx, input.0)?;
    let (cancel_tx, cancel_rx, page_guard) = host::track_page_call(invocation.page_id());
    let cancel_slot = Arc::new(Mutex::new(Some(cancel_tx)));

    let (ev_tx, ev_rx) = mpsc::unbounded_channel::<Result<HostStreamItem, lxapp::LxAppError>>();
    let ev_rx = Arc::new(tokio::sync::Mutex::new(ev_rx));

    // Spawn the Rust stream handler; feed every event/return into `ev_tx`.
    let route_name = route.clone();
    host::__native::spawn(async move {
        let _guard = page_guard;
        match handler.call(invocation, payload, cancel_rx).await {
            Ok(HostOutput::Stream(mut stream)) => {
                let mut sent_return = false;
                while let Some(item) = stream.next().await {
                    if matches!(item, Ok(HostStreamItem::Return(_))) {
                        sent_return = true;
                    }
                    if ev_tx.send(item).is_err() {
                        break;
                    }
                }
                if !sent_return {
                    let _ = ev_tx.send(Ok(HostStreamItem::Return("null".to_string())));
                }
            }
            Ok(HostOutput::Json(json)) => {
                let _ = ev_tx.send(Ok(HostStreamItem::Return(json)));
            }
            Err(err) => {
                let _ = ev_tx.send(Err(err));
            }
        }
        let _ = route_name;
    });

    // Build a JS-side `NativeTask` backed by `__next()` and `__cancel()` primitives so
    // `for await`, `return yield*`, `.result`, `.cancel()`, and `{ signal }` share one state machine.
    let cancel_for_js = Arc::clone(&cancel_slot);
    let cancel_fn = JSFunc::new(&ctx, move || -> JSResult<()> {
        if let Some(tx) = cancel_for_js
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            let _ = tx.send(());
        }
        Ok(())
    })?;

    let rx_for_next = Arc::clone(&ev_rx);
    let next_fn = JSFunc::new(&ctx, move |ctx: JSContext| {
        let rx_arc = Arc::clone(&rx_for_next);
        async move {
            let mut rx = rx_arc.lock().await;
            let obj = JSObject::new(&ctx);
            match rx.recv().await {
                Some(Ok(HostStreamItem::Event(json))) => {
                    obj.set("kind", "event")?;
                    obj.set("value", json_to_js_value(&ctx, Some(&json))?)?;
                }
                Some(Ok(HostStreamItem::Return(json))) => {
                    obj.set("kind", "return")?;
                    obj.set("value", json_to_js_value(&ctx, Some(&json))?)?;
                }
                Some(Err(err)) => {
                    return Err(js_error_from_lxapp_error(&err));
                }
                None => {
                    obj.set("kind", "return")?;
                    obj.set("value", JSValue::undefined(&ctx))?;
                }
            }
            Ok::<JSObject, rong::RongJSError>(obj)
        }
    })?;

    let helper: JSFunc = ctx.eval(Source::from_bytes(TASK_WRAPPER_JS))?;
    let opts_val = match options.0 {
        Some(o) => o.into_js_value(),
        None => JSValue::undefined(&ctx),
    };
    let task: JSObject = helper.call(None, (next_fn, cancel_fn, opts_val))?;
    Ok(task)
}

const TASK_WRAPPER_JS: &str = r#"(function(pullNext, doCancel, options) {
  if (options && options.signal) {
    if (options.signal.aborted) doCancel();
    else options.signal.addEventListener("abort", () => doCancel(), { once: true });
  }
  const queue = [];
  let settled = false;
  let failed = false;
  let outcome;
  let failure;
  let wake = null;
  let resolveResult, rejectResult;
  const result = new Promise((res, rej) => { resolveResult = res; rejectResult = rej; });
  const notify = () => { const fn = wake; wake = null; if (fn) fn(); };

  (async () => {
    try {
      while (true) {
        const item = await pullNext();
        if (item.kind === "event") {
          queue.push(item.value);
          notify();
        } else {
          outcome = item.value;
          settled = true;
          resolveResult(outcome);
          notify();
          break;
        }
      }
    } catch (err) {
      failure = err;
      failed = true;
      settled = true;
      rejectResult(err);
      notify();
    }
  })();

  let claimed = false;
  return {
    result,
    cancel() { doCancel(); },
    [Symbol.asyncIterator]() {
      if (claimed) throw new Error("A task has one progress consumer");
      claimed = true;
      let done = false;
      return {
        async next() {
          while (true) {
            if (queue.length > 0) return { done: false, value: queue.shift() };
            if (settled) {
              done = true;
              if (failed) throw failure;
              return { done: true, value: outcome };
            }
            await new Promise((r) => { wake = r; });
          }
        },
        async return() {
          if (!done && !settled) doCancel();
          done = true;
          return { done: true, value: undefined };
        },
      };
    },
  };
})"#;

/// Opens a `#[lingxia::native(..., channel)]` route from Logic and returns a `NativeChannel`.
async fn open_channel(
    ctx: JSContext,
    route: String,
    input: Optional<JSValue>,
    page_id: Optional<String>,
) -> JSResult<JSObject> {
    let invocation = HostInvocationContext::for_logic_context_with_page(&ctx, page_id.0)?;
    let handler =
        host::lookup_channel_for_caller(&route, invocation.caller()).ok_or_else(|| {
            HostError::new(
                rong::error::E_NOT_FOUND,
                format!("native channel route '{route}' is not registered or not authorized"),
            )
        })?;
    let payload = js_value_to_json(&ctx, input.0)?;
    let (channel_ctx, inbound_tx, outbound_rx) =
        ChannelContext::<host::JsonValue, host::JsonValue>::for_logic(format!(
            "logic-ch-{}",
            uuid::Uuid::new_v4()
        ));
    handler.on_open(invocation, channel_ctx, payload);

    let outbound_rx = Arc::new(tokio::sync::Mutex::new(outbound_rx));
    let send_tx = inbound_tx.clone();
    let send_fn = JSFunc::new(
        &ctx,
        move |ctx: JSContext, payload: JSValue| -> JSResult<()> {
            let json = js_value_to_json(&ctx, Some(payload))?.unwrap_or_else(|| "null".to_string());
            send_tx.send_data(json);
            Ok(())
        },
    )?;

    let close_tx = inbound_tx;
    let close_fn = JSFunc::new(
        &ctx,
        move |code: Optional<String>, reason: Optional<String>| -> JSResult<()> {
            close_tx.send_close(code.0, reason.0);
            Ok(())
        },
    )?;

    let pull_rx = Arc::clone(&outbound_rx);
    let pull_fn = JSFunc::new(&ctx, move |ctx: JSContext| {
        let rx_arc = Arc::clone(&pull_rx);
        async move {
            let mut rx = rx_arc.lock().await;
            let obj = JSObject::new(&ctx);
            match rx.recv().await {
                Some(host::LogicChannelOutbound::Data(json)) => {
                    obj.set("kind", "data")?;
                    obj.set("value", json_to_js_value(&ctx, Some(&json))?)?;
                }
                Some(host::LogicChannelOutbound::Close { code, reason }) => {
                    obj.set("kind", "close")?;
                    if let Some(code) = code {
                        obj.set("code", code)?;
                    }
                    if let Some(reason) = reason {
                        obj.set("reason", reason)?;
                    }
                }
                None => {
                    obj.set("kind", "close")?;
                }
            }
            Ok::<JSObject, rong::RongJSError>(obj)
        }
    })?;

    let helper: JSFunc = ctx.eval(Source::from_bytes(CHANNEL_WRAPPER_JS))?;
    let ch: JSObject = helper.call(None, (send_fn, close_fn, pull_fn))?;
    Ok(ch)
}

const CHANNEL_WRAPPER_JS: &str = r#"(function(doSend, doClose, pullNext) {
  let msgListeners = [];
  let closeListeners = [];
  let closed = false;
  (async () => {
    while (!closed) {
      const item = await pullNext();
      if (item.kind === "data") {
        msgListeners.slice().forEach((fn) => fn(item.value));
      } else {
        closed = true;
        const ev = { code: item.code, reason: item.reason };
        closeListeners.slice().forEach((fn) => fn(ev));
        break;
      }
    }
  })();
  return {
    send(payload) { if (!closed) doSend(payload); },
    onMessage(listener) {
      msgListeners.push(listener);
      return () => { msgListeners = msgListeners.filter((f) => f !== listener); };
    },
    onClose(listener) {
      closeListeners.push(listener);
      return () => { closeListeners = closeListeners.filter((f) => f !== listener); };
    },
    close(code, reason) {
      if (closed) return;
      closed = true;
      doClose(code !== undefined ? String(code) : undefined, reason);
    },
  };
})"#;

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let bridge = JSObject::new(ctx);
    bridge.set("invoke", JSFunc::new(ctx, invoke)?)?;
    bridge.set("invokeSync", JSFunc::new(ctx, invoke_sync)?)?;
    bridge.set("stream", JSFunc::new(ctx, stream)?)?;
    bridge.set("channel", JSFunc::new(ctx, open_channel)?)?;
    bridge.set("clearPageState", JSFunc::new(ctx, clear_page_state)?)?;
    ctx.global().set("__lingxiaNative", bridge)?;
    Ok(())
}
