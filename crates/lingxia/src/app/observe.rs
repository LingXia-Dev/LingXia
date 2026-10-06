//! The one mechanism behind every `watch_*` and `*_changes` in `lingxia::app`.
//!
//! An [`Observable`] holds a value. Callbacks are delivered by a [`Delivery`]
//! queue drained by one thread; the initial call and every change are queued
//! under the observable's lock, so they reach each callback in the order they
//! happened. Streams are woken directly and keep only the latest value.

use std::collections::VecDeque;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

type Job = Box<dyn FnOnce() + Send>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

/// A FIFO of callback work, run one job at a time on a thread of its own.
pub(crate) struct Delivery {
    name: &'static str,
    state: Mutex<DeliveryState>,
    ready: Condvar,
}

struct DeliveryState {
    jobs: VecDeque<Job>,
    running: bool,
}

impl Delivery {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self {
            name,
            state: Mutex::new(DeliveryState {
                jobs: VecDeque::new(),
                running: false,
            }),
            ready: Condvar::new(),
        }
    }

    /// The thread starts on first use. If it cannot start, the job stays
    /// queued and the next one retries: running it here instead would put a
    /// callback on a caller that may be the platform main thread. A queued
    /// grace release then waits for that next enqueue; the platform's own
    /// expiry ends the grace if none comes.
    fn enqueue(&'static self, job: Job) {
        let mut state = lock(&self.state);
        state.jobs.push_back(job);
        if state.running {
            self.ready.notify_one();
            return;
        }
        let spawned = std::thread::Builder::new()
            .name(self.name.into())
            .spawn(move || self.run());
        match spawned {
            Ok(_) => state.running = true,
            Err(error) => log::error!("failed to start {}: {error}", self.name),
        }
    }

    fn run(&self) {
        loop {
            let job = {
                let mut state = lock(&self.state);
                loop {
                    if let Some(job) = state.jobs.pop_front() {
                        break job;
                    }
                    state = self
                        .ready
                        .wait(state)
                        .unwrap_or_else(|error| error.into_inner());
                }
            };
            // Callbacks are guarded one by one; this guards the rest (a grace
            // release), so no job can end the thread and stall the queue.
            if catch_unwind(AssertUnwindSafe(job)).is_err() {
                log::error!("a {} job panicked", self.name);
            }
        }
    }
}

struct Callback<T> {
    active: AtomicBool,
    /// Only the delivery thread calls it, one call at a time, so the lock is
    /// never contended; it only makes an `FnMut` shareable.
    listener: Mutex<Box<dyn FnMut(T) + Send>>,
}

impl<T> Callback<T> {
    fn call(&self, value: T) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }
        let mut listener = lock(&self.listener);
        if catch_unwind(AssertUnwindSafe(|| (*listener)(value))).is_err() {
            log::error!("a lingxia::app watch callback panicked");
        }
    }
}

/// The latest value a stream has not yielded yet.
struct Slot<T> {
    state: Mutex<SlotState<T>>,
}

struct SlotState<T> {
    pending: Option<T>,
    last: Option<T>,
    waker: Option<Waker>,
}

impl<T: Clone + PartialEq> Slot<T> {
    /// Returns the waker to call once the observable's lock is released.
    fn offer(&self, value: T) -> Option<Waker> {
        let mut state = lock(&self.state);
        if state.last.as_ref() == Some(&value) {
            // Back to what the consumer already has: nothing to yield.
            state.pending = None;
            return None;
        }
        state.pending = Some(value);
        state.waker.take()
    }

    fn poll(&self, cx: &mut Context<'_>) -> Poll<T> {
        let mut state = lock(&self.state);
        if let Some(value) = state.pending.take() {
            state.last = Some(value.clone());
            return Poll::Ready(value);
        }
        match &state.waker {
            Some(waker) if waker.will_wake(cx.waker()) => {}
            _ => state.waker = Some(cx.waker().clone()),
        }
        Poll::Pending
    }
}

pub(crate) struct Observable<T> {
    delivery: &'static Delivery,
    state: Mutex<State<T>>,
}

struct State<T> {
    /// `None` until a source seeds it.
    value: Option<T>,
    /// The source's revision of `value`, for sources whose publishers race.
    revision: u64,
    next_id: u64,
    callbacks: Vec<(u64, Arc<Callback<T>>)>,
    streams: Vec<(u64, Arc<Slot<T>>)>,
}

impl<T> Observable<T>
where
    T: Clone + PartialEq + Send + 'static,
{
    pub(crate) const fn new(delivery: &'static Delivery, value: Option<T>) -> Self {
        Self {
            delivery,
            state: Mutex::new(State {
                value,
                revision: 0,
                next_id: 0,
                callbacks: Vec::new(),
                streams: Vec::new(),
            }),
        }
    }

    pub(crate) fn get(&self) -> Option<T> {
        lock(&self.state).value.clone()
    }

    /// Publish `value` if it differs from the current one.
    pub(crate) fn set(&self, value: T) {
        self.publish(lock(&self.state), value);
    }

    /// Publish `value` at a source `revision`, ignoring anything not newer
    /// than what is already held.
    pub(crate) fn offer(&self, revision: u64, value: T) {
        let mut state = lock(&self.state);
        if state.value.is_some() && revision <= state.revision {
            return;
        }
        state.revision = revision;
        self.publish(state, value);
    }

    /// Publish `value`, then run `then` on the delivery thread once every
    /// callback queued so far, this change's included, has returned. `then`
    /// runs even when the value is unchanged.
    pub(crate) fn set_then(&self, value: T, then: impl FnOnce() + Send + 'static) {
        let mut state = lock(&self.state);
        let wakers = self.apply(&mut state, value);
        self.delivery.enqueue(Box::new(then));
        drop(state);
        wakers.into_iter().for_each(Waker::wake);
    }

    fn publish(&self, mut state: MutexGuard<'_, State<T>>, value: T) {
        let wakers = self.apply(&mut state, value);
        drop(state);
        wakers.into_iter().for_each(Waker::wake);
    }

    /// Queues the callbacks under the lock, so their order is the order of
    /// the changes; wakers are returned to be called after it.
    fn apply(&self, state: &mut State<T>, value: T) -> Vec<Waker> {
        if state.value.as_ref() == Some(&value) {
            return Vec::new();
        }
        state.value = Some(value.clone());
        if !state.callbacks.is_empty() {
            let callbacks: Vec<_> = state.callbacks.iter().map(|(_, c)| c.clone()).collect();
            let value = value.clone();
            self.delivery.enqueue(Box::new(move || {
                for callback in callbacks {
                    callback.call(value.clone());
                }
            }));
        }
        state
            .streams
            .iter()
            .filter_map(|(_, slot)| slot.offer(value.clone()))
            .collect()
    }

    pub(crate) fn watch(&'static self, listener: impl FnMut(T) + Send + 'static) -> Subscription {
        let callback = Arc::new(Callback {
            active: AtomicBool::new(true),
            listener: Mutex::new(Box::new(listener)),
        });
        let id = {
            let mut state = lock(&self.state);
            let id = state.next_id;
            state.next_id += 1;
            state.callbacks.push((id, callback.clone()));
            if let Some(value) = state.value.clone() {
                let callback = callback.clone();
                self.delivery
                    .enqueue(Box::new(move || callback.call(value)));
            }
            id
        };
        Subscription::new(move || {
            callback.active.store(false, Ordering::Release);
            let removed = {
                let mut state = lock(&self.state);
                let index = state.callbacks.iter().position(|(key, _)| *key == id);
                index.map(|index| state.callbacks.swap_remove(index))
            };
            // Dropped off the lock: the listener may own Subscriptions of its
            // own, and dropping those locks this observable again.
            drop(removed);
        })
    }

    pub(crate) fn changes(&'static self) -> Changes<T> {
        let slot = Arc::new(Slot {
            state: Mutex::new(SlotState {
                pending: None,
                last: None,
                waker: None,
            }),
        });
        let id = {
            let mut state = lock(&self.state);
            let id = state.next_id;
            state.next_id += 1;
            lock(&slot.state).pending = state.value.clone();
            state.streams.push((id, slot.clone()));
            id
        };
        Changes {
            slot,
            _registration: Subscription::new(move || {
                let removed = {
                    let mut state = lock(&self.state);
                    let index = state.streams.iter().position(|(key, _)| *key == id);
                    index.map(|index| state.streams.swap_remove(index))
                };
                drop(removed);
            }),
        }
    }
}

/// A registered `watch_*` callback. Dropping it unsubscribes; [`detach`]
/// keeps the callback for the rest of the process.
///
/// Dropping never waits: no new call starts after it, and a call already
/// running finishes on the delivery thread. Dropping a `Subscription` from
/// inside its own callback is fine.
///
/// [`detach`]: Subscription::detach
#[must_use = "dropping a Subscription stops it; call .detach() to keep it for the process"]
pub struct Subscription {
    cancel: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Subscription {
    fn new(cancel: impl FnOnce() + Send + Sync + 'static) -> Self {
        Self {
            cancel: Some(Box::new(cancel)),
        }
    }

    /// Keep the callback registered for the rest of the process.
    pub fn detach(mut self) {
        self.cancel = None;
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel();
        }
    }
}

impl fmt::Debug for Subscription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Subscription").finish_non_exhaustive()
    }
}

/// An async view of one `lingxia::app` value, from a `*_changes()` call.
///
/// Yields the current value first, then each change. A consumer that falls
/// behind gets the latest value, never a backlog, and never the same value
/// twice in a row. It never ends; drop it to unsubscribe.
///
/// Implements [`futures_core::Stream`]; [`Changes::next`] needs no import:
///
/// ```no_run
/// # async fn demo() {
/// let mut changes = lingxia::app::foreground_changes();
/// loop {
///     let foreground = changes.next().await;
///     println!("foreground: {foreground}");
/// }
/// # }
/// ```
pub struct Changes<T> {
    slot: Arc<Slot<T>>,
    _registration: Subscription,
}

impl<T: Clone + PartialEq> Changes<T> {
    /// The next value: the current one on the first call, then each change.
    /// Cancel-safe. The stream never ends, so there is no `None`.
    pub async fn next(&mut self) -> T {
        std::future::poll_fn(|cx| self.slot.poll(cx)).await
    }
}

impl<T: Clone + PartialEq> futures_core::Stream for Changes<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        self.slot.poll(cx).map(Some)
    }
}

impl<T> fmt::Debug for Changes<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Changes").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread::ThreadId;
    use std::time::Duration;

    const WAIT: Duration = Duration::from_secs(2);
    const QUIET: Duration = Duration::from_millis(100);

    fn observable<T>(value: Option<T>) -> &'static Observable<T>
    where
        T: Clone + PartialEq + Send + 'static,
    {
        let delivery: &'static Delivery = Box::leak(Box::new(Delivery::new("lingxia-test")));
        Box::leak(Box::new(Observable::new(delivery, value)))
    }

    fn record<T>(signal: &'static Observable<T>) -> (Subscription, mpsc::Receiver<(T, ThreadId)>)
    where
        T: Clone + PartialEq + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel();
        // `FnMut` + `Send`: the listener owns its sender outright.
        let subscription = signal.watch(move |value| {
            let _ = sender.send((value, std::thread::current().id()));
        });
        (subscription, receiver)
    }

    fn next<T>(receiver: &mpsc::Receiver<(T, ThreadId)>) -> Option<T> {
        receiver.recv_timeout(WAIT).ok().map(|(value, _)| value)
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        struct Unpark(std::thread::Thread);
        impl std::task::Wake for Unpark {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
                return output;
            }
            std::thread::park_timeout(WAIT);
        }
    }

    fn poll_once<T: Clone + PartialEq>(changes: &mut Changes<T>) -> Poll<Option<T>> {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        Pin::new(changes).poll_next(&mut cx)
    }

    use futures_core::Stream;

    #[test]
    fn the_first_call_is_the_current_value_off_the_calling_thread() {
        let signal = observable(Some(true));
        signal.set(false);
        let (_subscription, events) = record(signal);
        let (value, thread) = events.recv_timeout(WAIT).unwrap();
        assert!(!value);
        assert_ne!(thread, std::thread::current().id());
    }

    #[test]
    fn fires_once_per_change_in_order() {
        let signal = observable(Some(true));
        let (_subscription, events) = record(signal);
        for value in [true, false, false, true, true, false] {
            signal.set(value);
        }
        assert_eq!(next(&events), Some(true));
        assert_eq!(next(&events), Some(false));
        assert_eq!(next(&events), Some(true));
        assert_eq!(next(&events), Some(false));
        assert!(
            events.recv_timeout(QUIET).is_err(),
            "a repeat must not fire"
        );
    }

    #[test]
    fn no_change_is_missed_between_registration_and_the_first_call() {
        for _ in 0..50 {
            let signal = observable(Some(0u32));
            let setter = std::thread::spawn(move || {
                for value in 1..=200u32 {
                    signal.set(value);
                }
            });
            let (_subscription, events) = record(signal);
            setter.join().unwrap();
            let mut seen = vec![next(&events).unwrap()];
            while *seen.last().unwrap() != 200 {
                seen.push(next(&events).expect("the last value arrives"));
            }
            // Whatever was current at registration, then every later value.
            let first = seen[0];
            assert_eq!(seen, (first..=200).collect::<Vec<_>>());
        }
    }

    #[test]
    fn a_dropped_subscription_stops_and_a_detached_one_continues() {
        let signal = observable(Some(1u32));
        let (dropped, dropped_events) = record(signal);
        let (detached, detached_events) = record(signal);
        assert_eq!(next(&dropped_events), Some(1));
        assert_eq!(next(&detached_events), Some(1));
        drop(dropped);
        detached.detach();
        signal.set(2);
        assert_eq!(next(&detached_events), Some(2));
        assert!(dropped_events.recv_timeout(QUIET).is_err());
        assert!(lock(&signal.state).callbacks.len() == 1);
    }

    #[test]
    fn a_callback_may_drop_its_own_subscription() {
        let signal = observable(Some(0u32));
        let own: Arc<Mutex<Option<Subscription>>> = Arc::new(Mutex::new(None));
        let (sender, events) = mpsc::channel();
        let sender = Mutex::new(sender);
        // Hold the slot's lock while registering so the first call (the
        // current value) cannot run before the subscription is stored.
        let mut slot = lock(&own);
        let inner = own.clone();
        *slot = Some(signal.watch(move |value| {
            let _ = lock(&sender).send(value);
            if value == 1 {
                // The listener owns the Subscription, which owns the listener.
                drop(lock(&inner).take());
            }
        }));
        drop(slot);
        assert_eq!(events.recv_timeout(WAIT), Ok(0));
        signal.set(1);
        assert_eq!(events.recv_timeout(WAIT), Ok(1));
        signal.set(2);
        assert!(events.recv_timeout(QUIET).is_err());
        assert!(lock(&signal.state).callbacks.is_empty());
    }

    #[test]
    fn a_panicking_callback_does_not_stop_the_others() {
        let signal = observable(Some(true));
        signal.watch(|_| panic!("listener fault")).detach();
        let (_subscription, events) = record(signal);
        signal.set(false);
        signal.set(true);
        assert_eq!(next(&events), Some(true));
        assert_eq!(next(&events), Some(false));
        assert_eq!(next(&events), Some(true));
    }

    #[test]
    fn an_unseeded_value_calls_back_on_its_first_value() {
        let signal = observable::<u32>(None);
        let (_subscription, events) = record(signal);
        assert!(events.recv_timeout(QUIET).is_err());
        signal.offer(3, 7);
        assert_eq!(next(&events), Some(7));
    }

    #[test]
    fn offer_keeps_the_newest_revision() {
        let signal = observable::<u32>(None);
        signal.offer(5, 50);
        signal.offer(4, 40);
        signal.offer(5, 51);
        assert_eq!(signal.get(), Some(50));
        signal.offer(6, 60);
        assert_eq!(signal.get(), Some(60));
    }

    #[test]
    fn then_runs_after_every_callback_for_that_change_returned() {
        let signal = observable(Some(true));
        let order = Arc::new(Mutex::new(Vec::new()));
        for name in ["first", "second"] {
            let order = order.clone();
            signal
                .watch(move |foreground: bool| {
                    if !foreground {
                        std::thread::sleep(Duration::from_millis(30));
                        lock(&order).push(name);
                    }
                })
                .detach();
        }
        let (done, finished) = mpsc::channel();
        let completion = order.clone();
        signal.set_then(false, move || {
            lock(&completion).push("then");
            let _ = done.send(());
        });
        finished.recv_timeout(WAIT).unwrap();
        assert_eq!(*lock(&order), ["first", "second", "then"]);

        // A repeat changes nothing, but its grace still completes.
        let (done, finished) = mpsc::channel();
        signal.set_then(false, move || {
            let _ = done.send(());
        });
        finished.recv_timeout(WAIT).unwrap();
    }

    #[test]
    fn a_panicking_release_does_not_stop_later_callbacks() {
        let signal = observable(Some(true));
        let (_subscription, events) = record(signal);
        assert_eq!(next(&events), Some(true));
        signal.set_then(false, || panic!("release fault"));
        assert_eq!(next(&events), Some(false));
        signal.set(true);
        assert_eq!(next(&events), Some(true));
    }

    #[test]
    fn a_listener_may_keep_mutable_state() {
        let signal = observable(Some(0u32));
        let (sender, calls) = mpsc::channel();
        let mut count = 0;
        let _subscription = signal.watch(move |_| {
            count += 1;
            let _ = sender.send(count);
        });
        signal.set(1);
        assert_eq!(calls.recv_timeout(WAIT), Ok(1));
        assert_eq!(calls.recv_timeout(WAIT), Ok(2));
    }

    #[test]
    fn a_slow_callback_does_not_delay_another_value() {
        let slow = observable(Some(false));
        let fast = observable(Some(0u32));
        let (entered, started) = mpsc::channel();
        let (release, held) = mpsc::channel::<()>();
        slow.watch(move |busy| {
            if busy {
                let _ = entered.send(());
                let _ = held.recv_timeout(WAIT);
            }
        })
        .detach();
        slow.set(true);
        started.recv_timeout(WAIT).unwrap();
        let (_subscription, events) = record(fast);
        fast.set(1);
        assert_eq!(next(&events), Some(0));
        assert_eq!(next(&events), Some(1));
        let _ = release.send(());
    }

    #[test]
    fn changes_yield_the_current_value_then_each_change() {
        let signal = observable(Some(1u32));
        let mut changes = signal.changes();
        assert_eq!(block_on(changes.next()), 1);
        assert_eq!(poll_once(&mut changes), Poll::Pending);
        let setter = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            signal.set(2);
        });
        assert_eq!(block_on(changes.next()), 2);
        setter.join().unwrap();
    }

    #[test]
    fn a_slow_consumer_gets_the_latest_value() {
        let signal = observable(Some(1u32));
        let mut changes = signal.changes();
        for value in 2..=9 {
            signal.set(value);
        }
        assert_eq!(poll_once(&mut changes), Poll::Ready(Some(9)));
        assert_eq!(poll_once(&mut changes), Poll::Pending);
        // Away and back before the consumer looked: nothing new to yield.
        signal.set(3);
        signal.set(9);
        assert_eq!(poll_once(&mut changes), Poll::Pending);
    }

    #[test]
    fn dropping_changes_unsubscribes() {
        let signal = observable(Some(1u32));
        let changes = signal.changes();
        assert_eq!(lock(&signal.state).streams.len(), 1);
        drop(changes);
        assert!(lock(&signal.state).streams.is_empty());
    }

    #[test]
    fn handles_are_send_and_unpin() {
        fn assert_send_sync_unpin<T: Send + Sync + Unpin>() {}
        assert_send_sync_unpin::<Subscription>();
        assert_send_sync_unpin::<Changes<bool>>();
        fn assert_send<T: Send>(_: &T) {}
        let mut changes = observable(Some(true)).changes();
        assert_send(&changes.next());
    }
}
