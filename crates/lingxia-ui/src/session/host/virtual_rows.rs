//! Automation: a keyed `<List>` row off screen stays addressable by testId, as every row is in
//! a WebView's DOM. A miss probes the committed lists on the worker; the next poll finds a
//! virtual entry, and acting on it scrolls its list until the row materializes.
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use crate::session::lifecycle::CommitTicket;
use std::sync::Arc;

/// Probes by testId, each for one presented commit.
pub(super) type VirtualRows = std::collections::HashMap<String, Probe>;

#[derive(Clone)]
pub(super) enum Probe {
    Pending(CommitTicket),
    /// The row's list scroll id and offset, or `None` when no off-screen row holds it.
    Done(CommitTicket, Option<(String, f32)>),
}

impl Probe {
    fn commit(&self) -> CommitTicket {
        match self {
            Self::Pending(commit) | Self::Done(commit, _) => *commit,
        }
    }
}

fn test_id(css: &str) -> Option<String> {
    let inner = css
        .trim()
        .strip_prefix("[data-testid=\"")?
        .strip_suffix("\"]")?;
    (!inner.contains(['"', '\\'])).then(|| inner.to_owned())
}

impl<A: PlatformAdapter> Inner<A> {
    /// The off-screen row a testId selector names on the presented frame, as its list's
    /// scroll id and offset; a first miss starts a probe and answers `None`.
    pub(super) fn virtual_row(self: &Arc<Self>, css: &str) -> Option<(String, f32)> {
        let test_id = test_id(css)?;
        let commit = {
            let mut main = self.main();
            let commit = main.interaction.frame()?.commit;
            main.virtual_rows
                .retain(|_, probe| probe.commit() == commit);
            match main.virtual_rows.get(&test_id) {
                Some(Probe::Done(_, found)) => return found.clone(),
                Some(Probe::Pending(_)) => return None,
                None if main.virtual_rows.len() >= 64 => return None,
                None => {}
            }
            main.virtual_rows
                .insert(test_id.clone(), Probe::Pending(commit));
            commit
        };
        let page = self.lifecycle.page();
        let probed = test_id.clone();
        self.work(
            Guard::Page(page),
            move |_, session| session.probe_list_row(&probed),
            move |inner, result| {
                let found = match result {
                    Some(Ok(found)) => found,
                    _ => None,
                };
                inner
                    .main()
                    .virtual_rows
                    .insert(test_id, Probe::Done(commit, found));
            },
        );
        None
    }
}
