//! Dev sessions: a page module swapped in place on the worker, then redrawn.
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use std::sync::{Arc, OnceLock};

/// How a swap ended: `Err` when the module could not be swapped in and the page loaded it
/// afresh instead.
pub type SwapReporter = fn(source: &str, outcome: Result<(), String>);

static REPORTER: OnceLock<SwapReporter> = OnceLock::new();

/// Dev sessions: tell `report` how each [`super::ViewHost::swap_module`] ended, so the dev
/// server can say so. The first reporter set stays.
pub fn set_swap_reporter(report: SwapReporter) {
    let _ = REPORTER.set(report);
}

fn report(source: &str, outcome: Result<(), String>) {
    if let Some(report) = REPORTER.get() {
        report(source, outcome);
    }
}

impl<A: PlatformAdapter> Inner<A> {
    pub(super) fn swap_module(self: &Arc<Self>, source: String) {
        let page = self.lifecycle.page();
        let next = source.clone();
        let started = std::time::Instant::now();
        self.work(
            Guard::Page(page),
            move |inner, session| {
                session.swap_module(&next)?;
                inner.drained(session)
            },
            move |inner, result| match result {
                Some(Ok(update)) => {
                    log::info!(
                        target: "lingxia_view",
                        "UI_SWAP applied ms={}",
                        started.elapsed().as_millis()
                    );
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    let mut main = inner.main();
                    // A later reload evaluates what is now running.
                    main.source = source.clone();
                    main.dirty = true;
                    drop(main);
                    inner.schedule();
                    report(&source, Ok(()));
                }
                // A save always ends with its module on screen: what cannot swap in place
                // loads afresh, losing local state, and shows its error page if it cannot render.
                Some(Err(error)) => {
                    inner.adapter.warn(&format!(
                        "hot swap rolled back: {error}; reloading the page"
                    ));
                    inner.load(Some(source.clone()));
                    report(&source, Err(error));
                }
                // The page closed meanwhile; its next open loads the new module.
                None => report(&source, Ok(())),
            },
        );
    }
}
