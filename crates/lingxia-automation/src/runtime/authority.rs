//! What the code of one host run may still do.
//!
//! A test run executes every spec in one JS context, so nothing tells a
//! continuation of a spec the runner gave up on from the spec after it. The
//! host scopes what specs install to *attempts* instead: the runner opens one
//! per spec, routes, mock scenarios, test clocks and dialog watches installed
//! while it is open belong to it, and closing it removes them and refuses
//! installs until the next one opens. An abandoned spec's attempt is ended like any other
//! (the runner refuses that spec's drivers on its side); when the runner
//! cannot isolate what the abandoned spec left running, it revokes the
//! context: every driver call from it is refused for the rest of the run.

use std::sync::Mutex;

#[derive(Debug, Default)]
struct State {
    next: u64,
    open: Option<u64>,
    /// An attempt was opened: from then on installs need an open one. A
    /// program that never opens one installs for the whole run.
    opened_any: bool,
    revoked: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct RunAuthority {
    state: Mutex<State>,
}

impl RunAuthority {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Open the next attempt and return its token.
    pub(crate) fn begin(&self) -> Result<u64, String> {
        let mut state = self.state();
        if let Some(reason) = &state.revoked {
            return Err(format!(
                "automation access of this run was revoked: {reason}"
            ));
        }
        if let Some(open) = state.open {
            return Err(format!("attempt {open} is still open"));
        }
        state.next += 1;
        state.open = Some(state.next);
        state.opened_any = true;
        Ok(state.next)
    }

    /// Close attempt `token`; installs are refused until the next opens.
    pub(crate) fn end(&self, token: u64) -> Result<(), String> {
        let mut state = self.state();
        if state.open != Some(token) {
            return Err(format!("attempt {token} is not open"));
        }
        state.open = None;
        Ok(())
    }

    /// Refuse the context's driver calls from now on. Returns the attempt
    /// that was open, now closed.
    pub(crate) fn revoke(&self, reason: String) -> Option<u64> {
        let mut state = self.state();
        state.revoked.get_or_insert(reason);
        state.open.take()
    }

    pub(crate) fn revoked(&self) -> Option<String> {
        self.state().revoked.clone()
    }

    /// Whether something may be installed now, and for which attempt
    /// (`None`: the whole run).
    pub(crate) fn admit(&self) -> Result<Option<u64>, String> {
        let state = self.state();
        if let Some(reason) = &state.revoked {
            return Err(format!(
                "automation access of this run was revoked: {reason}"
            ));
        }
        match state.open {
            Some(open) => Ok(Some(open)),
            None if state.opened_any => Err(
                "no spec is running: installs between specs are refused (a spec that already ended cannot install)"
                    .to_string(),
            ),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_belong_to_the_open_attempt_and_are_refused_between_attempts() {
        let authority = RunAuthority::default();
        assert_eq!(authority.admit(), Ok(None), "before any attempt: the run");
        let first = authority.begin().unwrap();
        assert_eq!(authority.admit(), Ok(Some(first)));
        assert!(authority.begin().is_err(), "one attempt at a time");
        authority.end(first).unwrap();
        assert!(
            authority
                .admit()
                .unwrap_err()
                .contains("no spec is running")
        );
        assert!(authority.end(first).is_err(), "an attempt ends once");
        let second = authority.begin().unwrap();
        assert!(second > first);
        assert_eq!(authority.admit(), Ok(Some(second)));
    }

    #[test]
    fn a_revoke_closes_the_open_attempt_and_refuses_everything_after() {
        let authority = RunAuthority::default();
        let open = authority.begin().unwrap();
        assert_eq!(authority.revoke("abandoned".into()), Some(open));
        assert_eq!(authority.revoked().as_deref(), Some("abandoned"));
        assert!(
            authority
                .admit()
                .unwrap_err()
                .contains("revoked: abandoned")
        );
        assert!(authority.begin().is_err());
        // The first reason stays.
        assert_eq!(authority.revoke("again".into()), None);
        assert_eq!(authority.revoked().as_deref(), Some("abandoned"));
    }
}
