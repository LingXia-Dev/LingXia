//! Native route mocks: the hook lxapp consults before the host answers a native call, and
//! the Logic half that runs the lxapp's `'native …'` handlers.
//!
//! ```text
//! per call:   scenario `native` rule | selection → mock handler (in the lxapp's Logic) | host route
//! ```

use super::mocks::MockDecision;
use super::registry::NativeDecision;
use super::scenario::NativeAnswer;
use super::{dev, registry};
use lingxia_control_protocol::mock::NATIVE_PREFIX;
use lxapp::LxApp;
use lxapp::host::{
    HostCancel, HostFuture, HostHandler, HostInvocationContext, HostOutput, HostStreamItem,
    NativeMockHook, SyncHostHandler,
};
use rong::{JSContext, JSFunc, JSObject, JSResult, JSValue, Source};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

const SLOT: &str = "globalThis[Symbol.for('lingxia.automation.native.mocks')]";

/// Runs `'native …'` handlers from the mocks module the `fetch` handlers share, so they
/// share its state. `answer` reports through `sink(kind, payload)`: `json`, `event`,
/// `return`, `error` (the route's coded error) or `failed` (a broken handler); `sink`
/// returns false once the caller stopped listening. `answerSync` returns `{ kind, payload }`.
const NATIVE_RUNNER: &str = r#"(function (mocks) {
  'use strict';
  const SLOT = Symbol.for('lingxia.automation.native.mocks');
  if (globalThis[SLOT]) return globalThis[SLOT];
  const message = function (error) {
    try {
      if (error && typeof error === 'object' && 'message' in error) return String(error.message);
      return String(error);
    } catch (_) {
      return 'error';
    }
  };
  const text = function (value) {
    return value === undefined ? 'null' : JSON.stringify(value);
  };
  const coded = function (error) {
    return error !== null && typeof error === 'object' && typeof error.code === 'string';
  };
  const failure = function (key, error) {
    if (coded(error)) {
      return { kind: 'error', payload: JSON.stringify({ code: error.code, message: message(error), data: error.data }) };
    }
    return { kind: 'failed', payload: "mock handler '" + key + "' threw: " + message(error) };
  };
  const context = function (route, real) {
    return Object.freeze({
      route: route,
      call: function (input) {
        if (!real) return Promise.reject(new TypeError("ctx.call reaches a call route; '" + route + "' has none"));
        return real(input === undefined ? undefined : JSON.stringify(input)).then(function (result) {
          return result === undefined ? undefined : JSON.parse(result);
        });
      },
      error: function (code, data, text) {
        const error = new Error(text === undefined ? String(code) : String(text));
        error.code = String(code);
        if (data !== undefined) error.data = data;
        return error;
      },
    });
  };
  const handler = function (key) {
    const found = mocks.handler(key);
    if (found === undefined) {
      throw { missing: "mock handler '" + key + "' is missing from the default export of mocks/index.ts" };
    }
    return found;
  };
  const isIterator = function (value) {
    return value !== null && typeof value === 'object' && typeof value.next === 'function'
      && (typeof value[Symbol.asyncIterator] === 'function' || typeof value[Symbol.iterator] === 'function');
  };
  const answer = function (key, route, input, sink, real) {
    let found;
    try { found = handler(key); } catch (error) { sink('failed', error.missing || message(error)); return; }
    Promise.resolve().then(function () {
      const parsed = input === undefined ? undefined : JSON.parse(input);
      return typeof found === 'function' ? found(parsed, context(route, real)) : found;
    }).then(async function (value) {
      if (!isIterator(value)) { sink('json', text(value)); return; }
      while (true) {
        let step;
        try { step = await value.next(); } catch (error) {
          const failed = failure(key, error);
          sink(failed.kind, failed.payload);
          return;
        }
        if (step.done) { sink('return', text(step.value)); return; }
        if (!sink('event', text(step.value))) {
          if (typeof value.return === 'function') { try { await value.return(); } catch (_) {} }
          return;
        }
      }
    }).catch(function (error) {
      const failed = failure(key, error);
      sink(failed.kind, failed.payload);
    });
  };
  const answerSync = function (key, route, input) {
    try {
      const found = handler(key);
      const parsed = input === undefined ? undefined : JSON.parse(input);
      const value = typeof found === 'function' ? found(parsed, context(route, null)) : found;
      if (value !== null && typeof value === 'object' && typeof value.then === 'function') {
        return { kind: 'failed', payload: "mock handler '" + key + "' answers a sync route: return the result, not a promise" };
      }
      return { kind: 'json', payload: text(value) };
    } catch (error) {
      if (error && error.missing) return { kind: 'failed', payload: error.missing };
      return failure(key, error);
    }
  };
  const runner = Object.freeze({ answer: answer, answerSync: answerSync });
  Object.defineProperty(globalThis, SLOT, { value: runner });
  return runner;
})"#;

/// Logic-context hook: the runner the native mock hook calls in this context.
pub(crate) fn install(ctx: &JSContext, mocks: JSValue) -> JSResult<()> {
    let installer = ctx.eval::<JSFunc>(Source::from_bytes(NATIVE_RUNNER))?;
    installer.call::<_, JSValue>(None, (mocks,))?;
    Ok(())
}

/// The hook lxapp calls for every native call of a dev build.
pub struct NativeMocks;

/// What `sink` received.
enum Answer {
    Json(String),
    Event(String),
    Return(String),
    Error(lingxia_error::Error),
    Failed(String),
}

impl Answer {
    fn parse(kind: &str, payload: String) -> Self {
        match kind {
            "json" => Self::Json(payload),
            "event" => Self::Event(payload),
            "return" => Self::Return(payload),
            "error" => match serde_json::from_str::<serde_json::Value>(&payload) {
                Ok(error) => Self::Error(rule_error(&error)),
                Err(err) => Self::Failed(format!("mock handler error is not an error: {err}")),
            },
            _ => Self::Failed(payload),
        }
    }
}

/// The error for a route no host registered and no mock answered.
fn missing(route: &str) -> lingxia_error::Error {
    lingxia_error::Error::not_found(format!(
        "native route '{route}' is not registered or not authorized; add '{NATIVE_PREFIX} \
         {route}' to mocks/index.ts"
    ))
}

/// Record a broken handler and the error its caller sees.
fn failed(appid: &str, key: &str, detail: String) -> lingxia_error::Error {
    registry::with_registry(|routes| routes.mocks.failed(appid, key, &detail));
    dev::warn(appid, detail.clone());
    lingxia_error::Error::internal(detail)
}

fn decide(appid: &str, route: &str, input: Option<&str>) -> NativeDecision {
    let decision = registry::with_registry(|routes| routes.decide_native(appid, route, input));
    if let NativeDecision::Select(MockDecision::Unhandled {
        detail,
        first: true,
    }) = &decision
    {
        dev::warn(appid, detail.clone());
    }
    decision
}

/// A rule's or handler's route error, shaped like the one the route would raise.
fn rule_error(error: &serde_json::Value) -> lingxia_error::Error {
    let code = error["code"].as_str().unwrap_or("E_INTERNAL").to_string();
    let message = error["message"].as_str().unwrap_or(&code).to_string();
    let built = lingxia_error::Error::new(code, message);
    match error.get("data") {
        Some(data) => built.with_data(data.clone()),
        None => built,
    }
}

fn text(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

/// Serve a scenario rule's answer: after its delay, its events, then its result or error.
async fn serve(answer: NativeAnswer, mut cancel: HostCancel) -> lingxia_error::Result<HostOutput> {
    if answer.delay_ms > 0 {
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_millis(answer.delay_ms)) => {}
            _ = &mut cancel => return Err(lingxia_error::Error::aborted("Canceled")),
        }
    }
    if answer.events.is_empty() {
        return match &answer.outcome {
            Ok(result) => Ok(HostOutput::Json(text(result))),
            Err(error) => Err(rule_error(error)),
        };
    }
    let last = match &answer.outcome {
        Ok(result) => Ok(HostStreamItem::Return(text(result))),
        Err(error) => Err(rule_error(error)),
    };
    let items: Vec<lingxia_error::Result<HostStreamItem>> = answer
        .events
        .iter()
        .map(|event| Ok(HostStreamItem::Event(text(event))))
        .chain(std::iter::once(last))
        .collect();
    Ok(HostOutput::Stream(Box::pin(futures::stream::iter(items))))
}

fn serve_sync(answer: NativeAnswer, route: &str) -> lingxia_error::Result<HostOutput> {
    if !answer.events.is_empty() {
        return Err(lingxia_error::Error::internal(format!(
            "a scenario rule with events answers a stream route; '{route}' is a sync route"
        )));
    }
    if answer.delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(answer.delay_ms));
    }
    match &answer.outcome {
        Ok(result) => Ok(HostOutput::Json(text(result))),
        Err(error) => Err(rule_error(error)),
    }
}

impl NativeMockHook for NativeMocks {
    fn call(
        &self,
        route: &str,
        invocation: HostInvocationContext,
        input: Option<String>,
        cancel: HostCancel,
        real: Option<Arc<dyn HostHandler>>,
    ) -> HostFuture<'static> {
        let route = route.to_string();
        Box::pin(async move {
            let lxapp = invocation.lxapp();
            match decide(&lxapp.appid, &route, input.as_deref()) {
                NativeDecision::Rule(answer) => serve(answer, cancel).await,
                NativeDecision::Select(MockDecision::Real) => match real {
                    Some(real) => real.call(invocation, input, cancel).await,
                    None => Err(missing(&route)),
                },
                NativeDecision::Select(MockDecision::Unhandled { detail, .. }) => {
                    Err(lingxia_error::Error::not_found(detail))
                }
                NativeDecision::Select(MockDecision::Mock { key }) => {
                    run(lxapp, key, route, invocation, input, cancel, real).await
                }
            }
        })
    }

    fn call_sync(
        &self,
        logic: &JSContext,
        route: &str,
        invocation: HostInvocationContext,
        input: Option<String>,
        real: Option<Arc<dyn SyncHostHandler>>,
    ) -> lingxia_error::Result<HostOutput> {
        let appid = invocation.lxapp().appid.clone();
        match decide(&appid, route, input.as_deref()) {
            NativeDecision::Rule(answer) => serve_sync(answer, route),
            NativeDecision::Select(MockDecision::Real) => match real {
                Some(real) => real.call_sync(invocation, input),
                None => Err(missing(route)),
            },
            NativeDecision::Select(MockDecision::Unhandled { detail, .. }) => {
                Err(lingxia_error::Error::not_found(detail))
            }
            NativeDecision::Select(MockDecision::Mock { key }) => {
                let answer = (|| -> JSResult<(String, String)> {
                    let runner: JSObject = logic.eval(Source::from_bytes(SLOT))?;
                    let answer_sync: JSFunc = runner.get("answerSync")?;
                    let answer: JSObject =
                        answer_sync.call(None, (key.clone(), route.to_string(), input))?;
                    Ok((answer.get("kind")?, answer.get("payload")?))
                })()
                .map_err(|err| {
                    lingxia_error::Error::internal(format!("native mocks unavailable: {err}"))
                })?;
                match Answer::parse(&answer.0, answer.1) {
                    Answer::Json(json) => Ok(HostOutput::Json(json)),
                    Answer::Error(error) => Err(error),
                    Answer::Failed(detail) => Err(failed(&appid, &key, detail)),
                    Answer::Event(_) | Answer::Return(_) => Err(failed(
                        &appid,
                        &key,
                        format!("mock handler '{key}' streams, but '{route}' is a sync route"),
                    )),
                }
            }
        }
    }
}

/// Run the handler under `key` in the lxapp's Logic and carry its answer back.
async fn run(
    lxapp: Arc<LxApp>,
    key: String,
    route: String,
    invocation: HostInvocationContext,
    input: Option<String>,
    mut cancel: HostCancel,
    real: Option<Arc<dyn HostHandler>>,
) -> lingxia_error::Result<HostOutput> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Answer>();
    let listening = Arc::new(AtomicBool::new(true));
    let appid = lxapp.appid.clone();
    {
        let tx = tx.clone();
        let listening = Arc::clone(&listening);
        let key = key.clone();
        let route = route.clone();
        lxapp.run_in_logic(Box::new(move |ctx: JSContext| {
            let started = (|| -> JSResult<()> {
                let runner: JSObject = ctx.eval(Source::from_bytes(SLOT))?;
                let answer: JSFunc = runner.get("answer")?;
                let sink_tx = tx.clone();
                let sink = JSFunc::new(&ctx, move |kind: String, payload: String| -> bool {
                    let _ = sink_tx.send(Answer::parse(&kind, payload));
                    listening.load(Ordering::SeqCst)
                })?;
                let real = match real {
                    Some(real) => rong::IntoJSValue::into_js_value(
                        JSFunc::new(&ctx, move |input: Option<String>| {
                            let real = Arc::clone(&real);
                            let invocation = invocation.clone();
                            async move {
                                let (_keep, cancel) = tokio::sync::oneshot::channel();
                                match real.call(invocation, input, cancel).await {
                                    Ok(HostOutput::Json(json)) => Ok(json),
                                    Ok(HostOutput::Stream(_)) => Err(rong::HostError::new(
                                        rong::error::E_INTERNAL,
                                        "ctx.call reaches a call route, not a stream",
                                    )
                                    .into()),
                                    Err(err) => Err(rong::HostError::new(
                                        err.static_code(),
                                        err.message().to_string(),
                                    )
                                    .into()),
                                }
                            }
                        })?,
                        &ctx,
                    ),
                    None => JSValue::null(&ctx),
                };
                answer.call::<_, ()>(None, (key, route, input, sink, real))
            })();
            if let Err(err) = started {
                let _ = tx.send(Answer::Failed(format!("native mocks unavailable: {err}")));
            }
        }))?;
    }
    drop(tx);
    let first = tokio::select! {
        answer = rx.recv() => answer,
        _ = &mut cancel => {
            listening.store(false, Ordering::SeqCst);
            return Err(lingxia_error::Error::aborted("Canceled"));
        }
    };
    match first {
        None => Err(failed(
            &appid,
            &key,
            format!("mock handler '{key}' never answered"),
        )),
        Some(Answer::Json(json)) => Ok(HostOutput::Json(json)),
        Some(Answer::Error(error)) => Err(error),
        Some(Answer::Failed(detail)) => Err(failed(&appid, &key, detail)),
        Some(first) => {
            // Events until the first terminal item; the handler's sink may outlive it.
            let stream = futures::stream::unfold(
                (Some(first), rx, cancel, listening, false),
                |(pending, mut rx, mut cancel, listening, done)| async move {
                    if done {
                        return None;
                    }
                    let answer = match pending {
                        Some(answer) => answer,
                        None => tokio::select! {
                            answer = rx.recv() => answer?,
                            _ = &mut cancel => {
                                listening.store(false, Ordering::SeqCst);
                                return None;
                            }
                        },
                    };
                    let done = !matches!(answer, Answer::Event(_));
                    if done {
                        listening.store(false, Ordering::SeqCst);
                    }
                    Some((answer, (None, rx, cancel, listening, done)))
                },
            );
            let stream = futures::StreamExt::map(stream, move |answer| match answer {
                Answer::Event(json) => Ok(HostStreamItem::Event(json)),
                Answer::Return(json) | Answer::Json(json) => Ok(HostStreamItem::Return(json)),
                Answer::Error(error) => Err(error),
                Answer::Failed(detail) => Err(failed(&appid, &key, detail)),
            });
            Ok(HostOutput::Stream(Box::pin(stream)))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_route_no_mock_and_no_host_answers_names_the_key_to_add() {
        let error = super::missing("notes.list");
        assert_eq!(error.static_code(), lingxia_error::code::NOT_FOUND);
        assert!(
            error
                .message()
                .contains("add 'native notes.list' to mocks/index.ts"),
            "{}",
            error.message()
        );
    }

    #[test]
    fn a_rule_error_keeps_its_code_and_data() {
        let error =
            super::rule_error(&serde_json::json!({ "code": "LOCKED", "data": { "id": 7 } }));
        assert_eq!(error.static_code(), "LOCKED");
        assert_eq!(error.data(), Some(&serde_json::json!({ "id": 7 })));
    }
}
