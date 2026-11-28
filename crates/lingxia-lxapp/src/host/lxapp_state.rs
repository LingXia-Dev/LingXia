//! Per-lxapp Rust state (`LxAppState<T>`): opened on first use, dropped with the lxapp's Logic.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use tokio::sync::OnceCell;

use super::HostInvocationContext;
use crate::lxapp::LxApp;

/// State one running lxapp opens once, such as a database pool.
pub trait FromLxApp: Sized + Send + Sync + 'static {
    /// Runs on first use; concurrent first calls wait for the same `open`, and a failure
    /// fails that call and lets the next one try again.
    fn open(app: &LxApp) -> impl Future<Output = lingxia_error::Result<Self>> + Send;
}

type Slot = Arc<OnceCell<Arc<dyn Any + Send + Sync>>>;

/// One lxapp's opened states, by type.
#[derive(Default)]
pub(crate) struct LxAppStates(Mutex<HashMap<TypeId, Slot>>);

impl LxAppStates {
    fn slots(&self) -> std::sync::MutexGuard<'_, HashMap<TypeId, Slot>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Drops every state; called where the lxapp's Logic is terminated.
    pub(crate) fn clear(&self) {
        let states = std::mem::take(&mut *self.slots());
        drop(states);
    }
}

/// A route's shared `&T` for the calling lxapp. No lock: `T` brings its own interior
/// mutability.
pub struct LxAppState<T: FromLxApp>(Arc<T>);

impl<T: FromLxApp> LxAppState<T> {
    #[doc(hidden)]
    pub async fn __acquire(invocation: &HostInvocationContext) -> lingxia_error::Result<Self> {
        let app = invocation.lxapp();
        let slot = {
            let mut slots = app.lxapp_states.slots();
            // Checked under the lock: close cancels the session before it clears the slots.
            if app.session.is_cancelled() {
                return Err(lingxia_error::Error::invalid_state(format!(
                    "lxapp {} has closed",
                    app.appid
                )));
            }
            Arc::clone(slots.entry(TypeId::of::<T>()).or_default())
        };
        let state = slot
            .get_or_try_init(|| async {
                T::open(&app)
                    .await
                    .map(|state| Arc::new(state) as Arc<dyn Any + Send + Sync>)
            })
            .await?;
        let state = Arc::clone(state)
            .downcast::<T>()
            .unwrap_or_else(|_| unreachable!("LxAppState slots are keyed by TypeId"));
        Ok(Self(state))
    }
}

impl<T: FromLxApp> Clone for LxAppState<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: FromLxApp> Deref for LxAppState<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}
