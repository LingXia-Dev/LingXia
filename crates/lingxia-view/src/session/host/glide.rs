//! A list `anchor` with a `duration` scrolls there natively: the host steps the region each
//! refresh, and the View runs only when the window shifts. Input on the region ends it.
use super::Inner;
use crate::platform::PlatformAdapter;
use std::sync::Arc;
use std::time::Duration;

const STEP: Duration = Duration::from_millis(16);

impl<A: PlatformAdapter> Inner<A> {
    /// After a frame presented: start the glides it began.
    pub(super) fn glide_later(self: &Arc<Self>) {
        let started = self.main().interaction.take_started_glides();
        if started.is_empty() {
            return;
        }
        let jump = self.adapter.environment().is_some_and(|env| {
            env.value["reducedMotion"] == true || env.value["animatorDurationScale"] == 0.0
        });
        for id in started {
            self.glide_step(id, jump);
        }
    }

    fn glide_step(self: &Arc<Self>, id: String, jump: bool) {
        let now = self.adapter.now();
        let Some(done) = self.main().interaction.glide_step(&id, now, jump) else {
            return;
        };
        self.deliver(vec![crate::interaction::view::Effect::Redraw]);
        if !done {
            let inner = self.clone();
            self.adapter
                .post_after(STEP, Box::new(move || inner.glide_step(id, jump)));
        }
    }
}
