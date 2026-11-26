//! Per-page Rust state (`PageState<T>`) and in-flight `page.native` call cancellation.
//!
//! Each `(page_instance_id, TypeId)` owns an independent `tokio::sync::Mutex<T>` initialized via
//! `T::default()` on first access. When the page is removed (or `setup` throws), every in-flight
//! `page.native` call for that page receives `HostCancel` and every `PageState<T>` is dropped.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, oneshot};

use super::{HostCancel, HostInvocationContext};

static NEXT_CALL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct PageBucket {
    states: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    in_flight: HashMap<u64, oneshot::Sender<()>>,
}

static PAGE_BUCKETS: LazyLock<Mutex<HashMap<String, PageBucket>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn buckets() -> MutexGuard<'static, HashMap<String, PageBucket>> {
    PAGE_BUCKETS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Keeps a call's cancellation registered until the call settles.
pub struct PageCallGuard {
    page_id: Option<String>,
    call_id: u64,
    // A page-less call keeps its sender here: dropping it would resolve `HostCancel`.
    _sender: Option<oneshot::Sender<()>>,
}

impl Drop for PageCallGuard {
    fn drop(&mut self) {
        let Some(page_id) = &self.page_id else {
            return;
        };
        let mut buckets = buckets();
        if let Some(bucket) = buckets.get_mut(page_id) {
            bucket.in_flight.remove(&self.call_id);
            if bucket.states.is_empty() && bucket.in_flight.is_empty() {
                buckets.remove(page_id);
            }
        }
    }
}

/// The `HostCancel` for one native call: it fires when the call's page is removed.
/// A call without a page is never cancelled this way.
pub fn track_page_call(invocation: &HostInvocationContext) -> (HostCancel, PageCallGuard) {
    let (tx, rx) = oneshot::channel();
    let Some(page_id) = invocation.page_id() else {
        return (
            rx,
            PageCallGuard {
                page_id: None,
                call_id: 0,
                _sender: Some(tx),
            },
        );
    };
    let call_id = NEXT_CALL_ID.fetch_add(1, Ordering::Relaxed);
    let mut buckets = buckets();
    // Checked under the bucket lock: removal drops the page first, then clears its bucket.
    if invocation.page().is_none() {
        drop(buckets);
        let _ = tx.send(());
        return (
            rx,
            PageCallGuard {
                page_id: None,
                call_id: 0,
                _sender: None,
            },
        );
    }
    buckets
        .entry(page_id.to_owned())
        .or_default()
        .in_flight
        .insert(call_id, tx);
    (
        rx,
        PageCallGuard {
            page_id: Some(page_id.to_owned()),
            call_id,
            _sender: None,
        },
    )
}

/// Cancels all in-flight `page.native` calls and drops every `PageState<T>` for `page_id`.
pub fn clear_page_state(page_id: &str) {
    let removed = buckets().remove(page_id);
    if let Some(bucket) = removed {
        for (_, tx) in bucket.in_flight {
            let _ = tx.send(());
        }
    }
}

pub(crate) fn clear_page_states(ids: &[String]) {
    for id in ids {
        clear_page_state(id);
    }
}

/// Per-page mutable Rust state for unary `#[lingxia::native]` handlers.
///
/// Calls on the same page targeting the same `T` serialize. A call without a page context
/// (module-level `native` instead of `page.native`) fails.
pub struct PageState<T: Send + 'static> {
    guard: OwnedMutexGuard<T>,
}

impl<T: Default + Send + 'static> PageState<T> {
    fn lookup_mutex(
        invocation: &HostInvocationContext,
    ) -> Result<Arc<AsyncMutex<T>>, lingxia_error::Error> {
        let Some(page_id) = invocation.page_id() else {
            return Err(lingxia_error::Error::invalid_arg(
                "PageState<T> requires a page context; call this native route via `page.native` instead of module-level `native`",
            ));
        };
        let mut buckets = buckets();
        // A bucket created after its page was removed would never be cleared.
        if invocation.page().is_none() {
            return Err(lingxia_error::Error::not_found(format!(
                "page {page_id} has unloaded"
            )));
        }
        let entry = buckets
            .entry(page_id.to_owned())
            .or_default()
            .states
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Arc::new(AsyncMutex::new(T::default()))));
        let mutex = entry
            .downcast_ref::<Arc<AsyncMutex<T>>>()
            .expect("PageState TypeId mismatch");
        Ok(Arc::clone(mutex))
    }

    /// Acquires the page's `T`, waiting for a concurrent call on the same page to finish.
    pub async fn acquire_async(
        invocation: &HostInvocationContext,
    ) -> Result<Self, lingxia_error::Error> {
        let mutex = Self::lookup_mutex(invocation)?;
        Ok(Self {
            guard: mutex.lock_owned().await,
        })
    }

    /// Acquires the page's `T` for a `sync` route, which runs on the Logic thread and so
    /// fails instead of waiting while an async call on the same page holds it.
    pub fn acquire_sync(invocation: &HostInvocationContext) -> Result<Self, lingxia_error::Error> {
        let mutex = Self::lookup_mutex(invocation)?;
        let guard = mutex.try_lock_owned().map_err(|_| {
            lingxia_error::Error::internal(format!(
                "PageState<{}> is held by an in-flight call on this page",
                std::any::type_name::<T>()
            ))
        })?;
        Ok(Self { guard })
    }
}

impl<T: Send + 'static> Deref for PageState<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<T: Send + 'static> DerefMut for PageState<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Counter {
        drops: Option<Arc<AtomicUsize>>,
    }

    impl Drop for Counter {
        fn drop(&mut self) {
            if let Some(d) = &self.drops {
                d.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    #[test]
    fn clear_page_state_cancels_in_flight_calls_and_drops_state() {
        let drops = Arc::new(AtomicUsize::new(0));
        let page_id = "test-page-state-drop-1";
        let (tx, mut cancel_rx) = oneshot::channel();
        {
            let mut buckets = buckets();
            let bucket = buckets.entry(page_id.to_owned()).or_default();
            bucket.in_flight.insert(1, tx);
            bucket.states.insert(
                TypeId::of::<Counter>(),
                Box::new(Arc::new(AsyncMutex::new(Counter {
                    drops: Some(Arc::clone(&drops)),
                }))),
            );
        }
        assert!(cancel_rx.try_recv().is_err());
        clear_page_state(page_id);
        assert!(
            cancel_rx.try_recv().is_ok(),
            "in-flight call must be cancelled"
        );
        assert_eq!(
            drops.load(Ordering::SeqCst),
            1,
            "PageState<T> must be dropped"
        );
        assert!(!buckets().contains_key(page_id));
    }
}
