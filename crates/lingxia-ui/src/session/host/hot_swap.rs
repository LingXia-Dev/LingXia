//! Dev sessions: a page module swapped in place on the worker, then redrawn.
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use std::sync::Arc;

impl<A: PlatformAdapter> Inner<A> {
    pub(super) fn swap_module(self: &Arc<Self>, source: String) {
        let page = self.lifecycle.page();
        let next = source.clone();
        self.work(
            Guard::Page(page),
            move |inner, session| {
                session.swap_module(&next)?;
                inner.drained(session)
            },
            move |inner, result| match result {
                Some(Ok(update)) => {
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    let mut main = inner.main();
                    // A later reload evaluates what is now running.
                    main.source = source;
                    main.dirty = true;
                    drop(main);
                    inner.schedule();
                }
                // The previous module keeps running; the author sees why the edit did not land.
                Some(Err(error)) => inner
                    .adapter
                    .warn(&format!("hot swap rolled back: {error}")),
                None => {}
            },
        );
    }
}
