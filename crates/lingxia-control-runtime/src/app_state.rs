//! Reports background and foreground to the dev session on hosts whose OS
//! freezes a background app, so the server can fail calls the app cannot answer.

use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::{Mutex, MutexGuard, Once};
use std::time::Duration;

/// A change, with the ack the watcher waits on when it holds the OS grace.
pub(crate) type Change = (bool, Option<SyncSender<()>>);

static CONNECTION: Mutex<Option<Sender<Change>>> = Mutex::new(None);
static WATCH: Once = Once::new();
const SEND_WAIT: Duration = Duration::from_millis(500);

fn connection() -> MutexGuard<'static, Option<Sender<Change>>> {
    CONNECTION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The changes for one connection, starting with the current state.
pub(crate) fn attach() -> Receiver<Change> {
    WATCH.call_once(|| lingxia::app::watch_foreground(forward).detach());
    let (tx, rx) = mpsc::channel();
    let _ = tx.send((lingxia::app::is_foreground(), None));
    *connection() = Some(tx);
    rx
}

// Runs inside the OS background grace: waiting until the change is on the
// wire lets the server learn it before the app is frozen.
fn forward(foreground: bool) {
    let Some(tx) = connection().clone() else {
        return;
    };
    let (ack, sent) = mpsc::sync_channel(1);
    if tx.send((foreground, Some(ack))).is_ok() {
        let _ = sent.recv_timeout(SEND_WAIT);
    }
}
