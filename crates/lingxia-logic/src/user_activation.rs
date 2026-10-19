//! A person just acted on something a guest lxapp put in front of them.
//!
//! A guest may take the product's focus (`lx.surface.bringToFront`) only on
//! the heels of such an action, so a background lxapp cannot pull the window
//! forward or take over the main area on its own. The action is recorded
//! natively — a banner button the person clicked — never claimed by the
//! guest, is bound to that guest's session, and is spent by the first use.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Long enough for Logic to react to the click, short enough that a stale
/// one cannot be saved up for later.
const WINDOW: Duration = Duration::from_secs(10);

type Key = (String, u64);

fn grants() -> &'static Mutex<HashMap<Key, Instant>> {
    static GRANTS: OnceLock<Mutex<HashMap<Key, Instant>>> = OnceLock::new();
    GRANTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Record that the person acted on this session's banner.
pub(crate) fn grant(app_id: &str, session_id: u64) {
    let mut grants = grants().lock().unwrap_or_else(|error| error.into_inner());
    grants.retain(|_, at| at.elapsed() < WINDOW);
    grants.insert((app_id.to_string(), session_id), Instant::now());
}

/// Spend this session's activation. `false` when there is none, or it expired.
pub(crate) fn take(app_id: &str, session_id: u64) -> bool {
    let mut grants = grants().lock().unwrap_or_else(|error| error.into_inner());
    grants
        .remove(&(app_id.to_string(), session_id))
        .is_some_and(|at| at.elapsed() < WINDOW)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_activation_is_bound_to_its_session_and_spent_once() {
        grant("com.example.activation", 1);
        assert!(!take("com.example.activation", 2), "another session");
        assert!(!take("com.example.other", 1), "another app");
        assert!(take("com.example.activation", 1));
        assert!(!take("com.example.activation", 1), "spent");
    }
}
