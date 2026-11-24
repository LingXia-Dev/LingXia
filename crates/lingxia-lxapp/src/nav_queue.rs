//! One FIFO of navigation operations per stack. Each runs once every operation before it has
//! settled, against the stack as it is at its turn. An operation identical to the last one still
//! waiting joins it instead of queueing again; past `MAX_WAITING` waiting, one more is refused.

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

/// Operations that may wait at once; a tap storm past it is refused, not queued.
pub const MAX_WAITING: usize = 8;

pub struct NavQueue {
    state: Mutex<State>,
    turn: watch::Sender<u64>,
}

#[derive(Default)]
struct State {
    next: u64,
    /// Admitted and not yet started, oldest first, with the key each was admitted under.
    waiting: VecDeque<(u64, String)>,
    /// Tickets dropped before their turn; the turn passes over them.
    abandoned: BTreeSet<u64>,
}

pub enum Admission {
    /// A new operation; await `NavTicket::turn` before touching the stack.
    Run(NavTicket),
    /// Identical to the operation with this id, still waiting: share its outcome.
    Join(u64),
    /// `MAX_WAITING` operations are already waiting.
    Full,
}

/// One admitted operation. It holds the stack from its turn until it is dropped.
pub struct NavTicket {
    queue: Arc<NavQueue>,
    id: u64,
    started: bool,
}

impl Default for NavQueue {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            turn: watch::channel(0).0,
        }
    }
}

impl NavQueue {
    pub fn admit(self: &Arc<Self>, key: &str) -> Admission {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((id, last)) = state.waiting.back()
            && last == key
        {
            return Admission::Join(*id);
        }
        if state.waiting.len() >= MAX_WAITING {
            return Admission::Full;
        }
        let id = state.next;
        state.next += 1;
        state.waiting.push_back((id, key.to_string()));
        Admission::Run(NavTicket {
            queue: self.clone(),
            id,
            started: false,
        })
    }

    fn advance(&self, finished: u64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = finished + 1;
        while state.abandoned.remove(&next) {
            next += 1;
        }
        self.turn.send_replace(next);
    }
}

impl NavTicket {
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Resolves at this operation's turn; from then on nothing joins it.
    pub async fn turn(&mut self) {
        let mut turn = self.queue.turn.subscribe();
        while *turn.borrow_and_update() != self.id {
            if turn.changed().await.is_err() {
                break;
            }
        }
        self.started = true;
        let mut state = self.queue.state.lock().unwrap_or_else(|e| e.into_inner());
        state.waiting.retain(|(id, _)| *id != self.id);
    }
}

impl Drop for NavTicket {
    fn drop(&mut self) {
        if self.started || *self.queue.turn.borrow() == self.id {
            self.queue.advance(self.id);
            return;
        }
        let mut state = self.queue.state.lock().unwrap_or_else(|e| e.into_inner());
        state.waiting.retain(|(id, _)| *id != self.id);
        state.abandoned.insert(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn run(queue: &Arc<NavQueue>, key: &str) -> NavTicket {
        match queue.admit(key) {
            Admission::Run(ticket) => ticket,
            Admission::Join(id) => panic!("{key} joined {id}"),
            Admission::Full => panic!("{key} refused"),
        }
    }

    async fn starts_within(ticket: &mut NavTicket, wait: Duration) -> bool {
        tokio::time::timeout(wait, ticket.turn()).await.is_ok()
    }

    #[tokio::test(start_paused = true)]
    async fn operations_run_one_at_a_time_in_the_order_they_were_issued() {
        let queue = Arc::new(NavQueue::default());
        let mut first = run(&queue, "push:a");
        let mut second = run(&queue, "back");
        let mut third = run(&queue, "push:b");
        assert!(starts_within(&mut first, Duration::from_millis(1)).await);
        assert!(
            !starts_within(&mut second, Duration::from_millis(1)).await,
            "waits for the first"
        );
        drop(first);
        assert!(starts_within(&mut second, Duration::from_millis(1)).await);
        assert!(!starts_within(&mut third, Duration::from_millis(1)).await);
        drop(second);
        assert!(starts_within(&mut third, Duration::from_millis(1)).await);
    }

    #[tokio::test(start_paused = true)]
    async fn an_operation_identical_to_the_last_waiting_one_joins_it() {
        let queue = Arc::new(NavQueue::default());
        let mut running = run(&queue, "reuse:cart");
        running.turn().await;
        let waiting = run(&queue, "reuse:cart");
        // The running one is not joined: its outcome may already be settling.
        assert!(matches!(queue.admit("reuse:cart"), Admission::Join(id) if id == waiting.id()));
        // Only consecutive: another operation between them ends the run of joins.
        let _back = run(&queue, "back");
        assert!(matches!(queue.admit("reuse:cart"), Admission::Run(_)));
    }

    #[tokio::test(start_paused = true)]
    async fn a_tap_storm_is_bounded() {
        let queue = Arc::new(NavQueue::default());
        let mut running = run(&queue, "first");
        running.turn().await;
        // Identical taps all join one waiting operation.
        let _waiting = run(&queue, "push:item");
        for _ in 0..100 {
            assert!(matches!(queue.admit("push:item"), Admission::Join(_)));
        }
        // Different ones fill the queue, then are refused.
        let held: Vec<_> = (1..MAX_WAITING)
            .map(|n| run(&queue, &format!("push:{n}")))
            .collect();
        assert!(matches!(queue.admit("push:last"), Admission::Full));
        drop(held);
        assert!(matches!(queue.admit("push:last"), Admission::Run(_)));
    }

    #[tokio::test(start_paused = true)]
    async fn an_operation_dropped_before_its_turn_does_not_hold_the_queue() {
        let queue = Arc::new(NavQueue::default());
        let mut first = run(&queue, "a");
        first.turn().await;
        let abandoned = run(&queue, "b");
        let mut third = run(&queue, "c");
        drop(abandoned);
        drop(first);
        assert!(starts_within(&mut third, Duration::from_millis(1)).await);
    }
}
