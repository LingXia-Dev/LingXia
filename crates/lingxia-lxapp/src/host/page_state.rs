//! Per-page Rust state (`PageState<T>`) and in-flight `page.native` call cancellation.
//!
//! Each `(page_instance_id, TypeId)` owns an independent `tokio::sync::Mutex<T>` initialized via
//! `T::default()` on first access. Concurrent calls on the same page targeting the same `T`
//! serialize and receive `&mut T` (`DerefMut`). When the page unloads (or `setup` fails), all
//! in-flight `page.native` calls for that page receive `HostCancel` and every `PageState<T>` for
//! that page is dropped.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, oneshot};

use super::HostInvocationContext;
use crate::LxAppError;

static NEXT_CALL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct PageBucket {
    states: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    in_flight: HashMap<u64, oneshot::Sender<()>>,
}

static PAGE_BUCKETS: LazyLock<Mutex<HashMap<String, PageBucket>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// RAII guard that removes an in-flight call's cancellation sender when the call settles.
pub struct PageCallGuard {
    page_id: Option<String>,
    call_id: u64,
}

impl Drop for PageCallGuard {
    fn drop(&mut self) {
        if let Some(page_id) = &self.page_id {
            let mut buckets = PAGE_BUCKETS.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(bucket) = buckets.get_mut(page_id) {
                bucket.in_flight.remove(&self.call_id);
                if bucket.states.is_empty() && bucket.in_flight.is_empty() {
                    buckets.remove(page_id);
                }
            }
        }
    }
}

/// Registers a cancellable `page.native` call for `page_id` and returns `(HostCancel, PageCallGuard)`.
/// For app-level (`None`) calls, returns a detached `(HostCancel, PageCallGuard)`.
pub fn track_page_call(
    page_id: Option<&str>,
) -> (oneshot::Sender<()>, super::HostCancel, PageCallGuard) {
    let (tx, rx) = oneshot::channel();
    let Some(page_id) = page_id else {
        return (
            tx,
            rx,
            PageCallGuard {
                page_id: None,
                call_id: 0,
            },
        );
    };
    let (page_tx, page_rx) = oneshot::channel();
    let (merged_tx, merged_rx) = oneshot::channel();
    let call_id = NEXT_CALL_ID.fetch_add(1, Ordering::Relaxed);
    {
        let mut buckets = PAGE_BUCKETS.lock().unwrap_or_else(|p| p.into_inner());
        buckets
            .entry(page_id.to_owned())
            .or_default()
            .in_flight
            .insert(call_id, page_tx);
    }
    // Bridge either caller-initiated cancel (`tx`) or page-unload cancel (`page_rx`) into `merged_rx`.
    let _ = tokio::runtime::Handle::try_current().map(|handle| {
        handle.spawn(async move {
            tokio::select! {
                _ = rx => { let _ = merged_tx.send(()); }
                _ = page_rx => { let _ = merged_tx.send(()); }
            }
        })
    });
    (
        tx,
        merged_rx,
        PageCallGuard {
            page_id: Some(page_id.to_owned()),
            call_id,
        },
    )
}

/// Registers a page-scoped `HostCancel` directly for a `page.native` call without a separate
/// caller cancel handle (used when only page unload cancels the call).
pub fn register_page_cancel(page_id: &str) -> (super::HostCancel, PageCallGuard) {
    let (tx, rx) = oneshot::channel();
    let call_id = NEXT_CALL_ID.fetch_add(1, Ordering::Relaxed);
    {
        let mut buckets = PAGE_BUCKETS.lock().unwrap_or_else(|p| p.into_inner());
        buckets
            .entry(page_id.to_owned())
            .or_default()
            .in_flight
            .insert(call_id, tx);
    }
    (
        rx,
        PageCallGuard {
            page_id: Some(page_id.to_owned()),
            call_id,
        },
    )
}

/// Cancels all in-flight `page.native` calls and drops every `PageState<T>` for `page_id`.
pub fn clear_page_state(page_id: &str) {
    let removed = {
        let mut buckets = PAGE_BUCKETS.lock().unwrap_or_else(|p| p.into_inner());
        buckets.remove(page_id)
    };
    if let Some(bucket) = removed {
        for (_, tx) in bucket.in_flight {
            let _ = tx.send(());
        }
        drop(bucket.states);
    }
}

/// Cancels all in-flight `page.native` calls and drops every `PageState<T>` for every id in `ids`.
pub fn clear_page_states(ids: &[String]) {
    for id in ids {
        clear_page_state(id);
    }
}

/// Per-page mutable Rust state extractor for `#[lingxia::native]` handlers.
///
/// Calls on the same page targeting the same `T` serialize automatically. Calling a route that
/// requests `PageState<T>` from module-level `native` (without a page context) fails with a clear
/// error directing the author to `page.native`.
pub struct PageState<T: Send + 'static> {
    guard: OwnedMutexGuard<T>,
}

impl<T: Default + Send + 'static> PageState<T> {
    fn lookup_mutex(invocation: &HostInvocationContext) -> Result<Arc<AsyncMutex<T>>, LxAppError> {
        let Some(page_id) = invocation.page_id() else {
            return Err(LxAppError::InvalidParameter(
                "PageState<T> requires a page context; call this native route via `page.native` instead of module-level `native`"
                    .into(),
            ));
        };
        let mut buckets = PAGE_BUCKETS.lock().unwrap_or_else(|p| p.into_inner());
        let bucket = buckets.entry(page_id.to_owned()).or_default();
        let entry = bucket
            .states
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Arc::new(AsyncMutex::new(T::default()))));
        let mutex = entry
            .downcast_ref::<Arc<AsyncMutex<T>>>()
            .expect("PageState TypeId mismatch");
        Ok(Arc::clone(mutex))
    }

    /// Acquires the page's `T` state asynchronously, serializing concurrent calls on the same page.
    pub async fn acquire_async(invocation: &HostInvocationContext) -> Result<Self, LxAppError> {
        let mutex = Self::lookup_mutex(invocation)?;
        let guard = mutex.lock_owned().await;
        Ok(Self { guard })
    }

    /// Acquires the page's `T` state synchronously (for `#[lingxia::native(..., sync)]` routes).
    pub fn acquire_sync(invocation: &HostInvocationContext) -> Result<Self, LxAppError> {
        let mutex = Self::lookup_mutex(invocation)?;
        let guard = match Arc::clone(&mutex).try_lock_owned() {
            Ok(guard) => guard,
            Err(_) => futures::executor::block_on(mutex.lock_owned()),
        };
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
        value: usize,
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
        let (mut cancel_rx, _guard) = register_page_cancel(page_id);
        {
            let mut buckets = PAGE_BUCKETS.lock().unwrap();
            let bucket = buckets.entry(page_id.to_owned()).or_default();
            bucket.states.insert(
                TypeId::of::<Counter>(),
                Box::new(Arc::new(AsyncMutex::new(Counter {
                    value: 7,
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
    }
}
