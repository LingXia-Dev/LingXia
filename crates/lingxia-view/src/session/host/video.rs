//! Player states reach the page as events at most every 250 ms, while its videos draw. While the
//! page scrolls, time alone moves its readers once a second, so a time label costs no frames.
use super::{Guard, Inner};
use crate::platform::PlatformAdapter;
use std::sync::Arc;
use std::time::Duration;

const PERIOD: Duration = Duration::from_millis(250);
const SCROLLING_PERIOD: Duration = Duration::from_secs(1);

impl<A: PlatformAdapter> Inner<A> {
    pub(super) fn video_tick(self: &Arc<Self>) {
        let now = self.adapter.now();
        let presented = {
            let mut main = self.main();
            if main.disposed
                || main
                    .video_flushed
                    .is_some_and(|at| now.saturating_sub(at) < PERIOD)
            {
                return;
            }
            main.video_flushed = Some(now);
            let revision = main.interaction.scroll().revision();
            let scrolling =
                std::mem::replace(&mut main.video_scroll_revision, revision) != revision;
            let hold_time = scrolling
                && main
                    .video_time_flushed
                    .is_some_and(|at| now.saturating_sub(at) < SCROLLING_PERIOD);
            if !hold_time {
                main.video_time_flushed = Some(now);
            }
            main.snapshot.is_some().then_some(hold_time)
        };
        let Some(hold_time) = presented else {
            return;
        };
        let page = self.lifecycle.page();
        self.work(
            Guard::Page(page),
            move |inner, session| {
                if !session.dispatch_video_events(hold_time)? {
                    return Ok(None);
                }
                inner.drained(session).map(Some)
            },
            move |inner, result| match result {
                Some(Ok(Some(update))) => {
                    if let Some((connection, update)) = update {
                        inner.publish(connection, update);
                    }
                    if !inner.main().disposed && inner.lifecycle.is_current_page(page) {
                        inner.main().dirty = true;
                        inner.schedule();
                    }
                }
                Some(Err(error)) if inner.lifecycle.is_current_page(page) => inner.fail(error),
                _ => {}
            },
        );
    }
}
