//! Route tests on plain `cargo test`: no platform, no WebView.
//!
//! ```ignore
//! let host = lingxia::test_host!();
//! let note: Note = host.call("notes.add", json!({ "title": "a" })).await?;
//! ```

use crate::host::{
    self, ChannelContext, HostOutput, HostStream, HostStreamItem, JsonValue, LogicChannelInbound,
    LogicChannelOutbound,
};
use crate::{AppSessionClass, HostAddon, LxApp};
use lxapp::host::testing::TestSession;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::any::Any;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::{mpsc, oneshot};

/// A [`TestHost`] with the calling crate's routes installed: `lingxia::test_host!()` in a
/// crate's own tests, `lingxia::test_host!(my_lib)` from an integration test or for a
/// library crate's routes.
#[macro_export]
macro_rules! test_host {
    () => {
        $crate::testing::TestHost::__new(crate::__lingxia_install_routes)
    };
    ($krate:ident) => {
        $crate::testing::TestHost::__new(::$krate::__lingxia_install_routes)
    };
}

/// One lxapp session calling routes the way its pages do. Each one starts with fresh
/// `LxAppState<T>` values.
pub struct TestHost {
    session: TestSession,
    addons: Vec<Arc<dyn Any + Send + Sync>>,
}

impl TestHost {
    #[doc(hidden)]
    pub fn __new(install_routes: fn()) -> Self {
        install_routes();
        Self::session(AppSessionClass::StandardApp, Vec::new())
    }

    fn session(class: AppSessionClass, addons: Vec<Arc<dyn Any + Send + Sync>>) -> Self {
        let session = TestSession::new(class).expect("create the test lxapp session");
        for addon in &addons {
            session.provide(Arc::clone(addon));
        }
        Self { session, addons }
    }

    /// The addon a route's `lingxia::Host<A>` gets in this session.
    pub fn with_addon<A: HostAddon + 'static>(self, addon: A) -> Self {
        let addon: Arc<dyn Any + Send + Sync> = Arc::new(addon);
        self.session.provide(Arc::clone(&addon));
        let mut addons = self.addons.clone();
        addons.push(addon);
        Self {
            session: self.session,
            addons,
        }
    }

    /// Calls routes as this session class, so audiences apply as they do for a real lxapp.
    /// A new session: earlier `LxAppState<T>` values are gone.
    pub fn as_class(self, class: AppSessionClass) -> Self {
        Self::session(class, self.addons)
    }

    /// The test lxapp, for a route that takes `Arc<LxApp>` directly.
    pub fn lxapp(&self) -> Arc<LxApp> {
        self.session.lxapp()
    }

    /// Calls a route and decodes its result. An unknown route, one this session's class may
    /// not call, and a route's own error all arrive as the error.
    pub async fn call<T: DeserializeOwned>(
        &self,
        route: &str,
        input: impl Serialize,
    ) -> crate::Result<T> {
        let payload = encode(&input)?;
        let invocation = self.session.invocation();
        let Some(handler) = host::lookup_host_for_caller(route, invocation.caller()) else {
            return match host::dispatch_host_call_sync(route, invocation, payload)? {
                HostOutput::Json(json) => decode(route, &json),
                HostOutput::Stream(_) => Err(crate::Error::internal(format!(
                    "native route '{route}' returned a stream"
                ))),
            };
        };
        let (_keep, cancel) = oneshot::channel();
        let output =
            host::__native::spawn(async move { handler.call(invocation, payload, cancel).await })
                .await
                .map_err(|err| {
                    crate::Error::internal(format!("native route '{route}' did not finish: {err}"))
                })??;
        match output {
            HostOutput::Json(json) => decode(route, &json),
            HostOutput::Stream(_) => Err(crate::Error::internal(format!(
                "native route '{route}' is a stream; call it with TestHost::stream"
            ))),
        }
    }

    /// Like [`call`](Self::call), with the route's own error enum (which also derives
    /// `Deserialize`) told apart from framework errors.
    pub async fn call_as<T: DeserializeOwned, E: DeserializeOwned>(
        &self,
        route: &str,
        input: impl Serialize,
    ) -> Result<T, RouteError<E>> {
        self.call(route, input)
            .await
            .map_err(RouteError::from_error)
    }

    /// Opens a channel route: [`TestChannel::send`] is what the page sends. Dropping the
    /// channel closes it, as leaving a page does.
    pub fn channel<S: Serialize, R: DeserializeOwned>(
        &self,
        route: &str,
        input: impl Serialize,
    ) -> crate::Result<TestChannel<S, R>> {
        let payload = encode(&input)?;
        let invocation = self.session.invocation();
        let handler =
            host::lookup_channel_for_caller(route, invocation.caller()).ok_or_else(|| {
                crate::Error::not_found(format!(
                    "native channel route '{route}' is not registered or not authorized"
                ))
            })?;
        let (ctx, inbound, outbound) =
            ChannelContext::<JsonValue, JsonValue>::for_logic(format!("test-{route}"));
        handler.on_open(invocation, ctx, payload);
        Ok(TestChannel {
            route: route.to_string(),
            inbound,
            outbound,
            closed: false,
            _types: PhantomData,
        })
    }

    /// Opens a stream route. Dropping the stream cancels it, as leaving a page does.
    pub fn stream<E: DeserializeOwned, R: DeserializeOwned>(
        &self,
        route: &str,
        input: impl Serialize,
    ) -> TestStream<E, R> {
        let route = route.to_string();
        let invocation = self.session.invocation();
        let (keep, cancel) = oneshot::channel();
        let state = match encode(&input) {
            Err(err) => State::Failed(Some(err)),
            Ok(payload) => {
                let handler = host::lookup_host_for_caller(&route, invocation.caller());
                let name = route.clone();
                State::Opening(Box::pin(async move {
                    let handler = handler.ok_or_else(|| {
                        crate::Error::not_found(format!(
                            "native stream route '{name}' is not registered or not authorized"
                        ))
                    })?;
                    match handler.call(invocation, payload, cancel).await? {
                        HostOutput::Stream(stream) => Ok(stream),
                        HostOutput::Json(_) => Err(crate::Error::internal(format!(
                            "native route '{name}' is not a stream; call it with TestHost::call"
                        ))),
                    }
                }))
            }
        };
        TestStream {
            route,
            state,
            result: None,
            _keep: keep,
            _types: PhantomData,
        }
    }
}

/// A route's error as [`TestHost::call_as`] reports it.
#[derive(Debug)]
pub enum RouteError<E> {
    /// The route's own error enum.
    Route(E),
    /// A framework error: an unknown or unauthorized route, bad input, `E_INTERNAL`.
    Framework(crate::Error),
}

impl<E: DeserializeOwned> RouteError<E> {
    /// The enum's `code` tag and `data` fields, as `#[derive(NativeError)]` sent them.
    fn from_error(error: crate::Error) -> Self {
        let mut fields = match error.data() {
            None => serde_json::Map::new(),
            Some(serde_json::Value::Object(fields)) => fields.clone(),
            Some(_) => return Self::Framework(error),
        };
        fields.insert("code".into(), error.code().into());
        match serde_json::from_value(serde_json::Value::Object(fields)) {
            Ok(route) => Self::Route(route),
            Err(_) => Self::Framework(error),
        }
    }
}

/// A channel route seen from the page.
pub struct TestChannel<S, R> {
    route: String,
    inbound: LogicChannelInbound,
    outbound: mpsc::UnboundedReceiver<LogicChannelOutbound>,
    closed: bool,
    _types: PhantomData<fn(S) -> R>,
}

impl<S: Serialize, R: DeserializeOwned> TestChannel<S, R> {
    /// Sends a message to the route.
    pub fn send(&self, message: S) -> crate::Result<()> {
        let json = serde_json::to_string(&message)
            .map_err(|err| crate::Error::invalid_arg(format!("channel message: {err}")))?;
        self.inbound.send_data(json);
        Ok(())
    }

    /// The route's next message; `None` once it closed the channel. A route that failed
    /// reports its error once, then `None`.
    pub async fn recv(&mut self) -> Option<crate::Result<R>> {
        if self.closed {
            return None;
        }
        match self.outbound.recv().await {
            Some(LogicChannelOutbound::Data(json)) => Some(decode(&self.route, &json)),
            Some(LogicChannelOutbound::Close { error }) => {
                self.closed = true;
                error.map(Err)
            }
            None => {
                self.closed = true;
                None
            }
        }
    }

    /// Closes the channel from the page; the route's `recv` sees the close.
    pub fn close(self) {}
}

impl<S, R> Drop for TestChannel<S, R> {
    fn drop(&mut self) {
        self.inbound.send_close(None);
    }
}

fn encode(input: &impl Serialize) -> crate::Result<Option<String>> {
    let value = serde_json::to_value(input)
        .map_err(|err| crate::Error::invalid_arg(format!("route input: {err}")))?;
    Ok((!value.is_null()).then(|| value.to_string()))
}

fn decode<T: DeserializeOwned>(route: &str, json: &str) -> crate::Result<T> {
    serde_json::from_str(json).map_err(|err| {
        crate::Error::internal(format!("native route '{route}' result: {err}: {json}"))
    })
}

type Opening = Pin<Box<dyn Future<Output = crate::Result<HostStream>> + Send>>;

enum State {
    Opening(Opening),
    Open(HostStream),
    Failed(Option<crate::Error>),
    Done,
}

/// A stream route's events, then its result.
pub struct TestStream<E, R> {
    route: String,
    state: State,
    result: Option<crate::Result<String>>,
    /// Held until the stream drops: dropping it cancels the route.
    _keep: oneshot::Sender<()>,
    _types: PhantomData<fn() -> (E, R)>,
}

impl<E: DeserializeOwned, R: DeserializeOwned> TestStream<E, R> {
    /// The next event; `None` once the route has returned or failed.
    pub async fn next(&mut self) -> Option<crate::Result<E>> {
        std::future::poll_fn(|cx| self.poll_event(cx)).await
    }

    /// Skips any remaining events and returns the route's result.
    pub async fn result(mut self) -> crate::Result<R> {
        while let Some(event) = self.next().await {
            event?;
        }
        match self.result.take() {
            Some(Ok(json)) => decode(&self.route, &json),
            Some(Err(err)) => Err(err),
            None => Err(crate::Error::internal(format!(
                "native stream route '{}' ended without a result",
                self.route
            ))),
        }
    }

    fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<Option<crate::Result<E>>> {
        loop {
            match &mut self.state {
                State::Opening(opening) => match opening.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(stream)) => self.state = State::Open(stream),
                    Poll::Ready(Err(err)) => self.state = State::Failed(Some(err)),
                },
                State::Open(stream) => match stream.as_mut().poll_next(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Some(Ok(HostStreamItem::Event(json)))) => {
                        return Poll::Ready(Some(decode(&self.route, &json)));
                    }
                    Poll::Ready(Some(Ok(HostStreamItem::Return(json)))) => {
                        self.result = Some(Ok(json));
                        self.state = State::Done;
                    }
                    Poll::Ready(Some(Err(err))) => self.state = State::Failed(Some(err)),
                    Poll::Ready(None) => self.state = State::Done,
                },
                State::Failed(err) => {
                    if let Some(err) = err.take() {
                        self.result = Some(Err(err));
                    }
                    self.state = State::Done;
                }
                State::Done => return Poll::Ready(None),
            }
        }
    }
}
