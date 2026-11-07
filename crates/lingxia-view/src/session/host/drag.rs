//! A released `drag` element springs from where the finger left it to the snap point the
//! release picked, keeping the finger's velocity. A finger on it, or a newer release, stops it.
use super::Inner;
use crate::interaction::drag::Release;
use crate::platform::PlatformAdapter;
use std::sync::Arc;
use std::time::Duration;

const STEP: Duration = Duration::from_millis(16);

#[derive(Default)]
pub(super) struct Drags {
    /// The release each element springs to now; an older one stops.
    serials: std::collections::HashMap<String, u64>,
    next: u64,
}

impl<A: PlatformAdapter> Inner<A> {
    /// After input: spring each released element to its point.
    pub(super) fn drag_later(self: &Arc<Self>) {
        let releases = {
            let mut main = self.main();
            let releases = main.interaction.take_drag_releases();
            for release in &releases {
                main.drags.next += 1;
                let serial = main.drags.next;
                main.drags.serials.insert(release.identity.clone(), serial);
            }
            releases
                .into_iter()
                .map(|release| {
                    let serial = main.drags.serials[&release.identity];
                    (release, serial)
                })
                .collect::<Vec<_>>()
        };
        let reduce = self.adapter.environment().is_some_and(|env| {
            env.value["reducedMotion"] == true || env.value["animatorDurationScale"] == 0.0
        });
        for (release, serial) in releases {
            self.drag_step(
                Arc::new(release),
                serial,
                if reduce { f64::INFINITY } else { 0.0 },
            );
        }
    }

    fn drag_step(self: &Arc<Self>, release: Arc<Release>, serial: u64, elapsed: f64) {
        // Durations and bounces come validated from the transition; one that is not lands at once.
        let spring = crate::animation::spring::Spring::new(release.duration, release.bounce).ok();
        let (from, to, velocity) = (
            f64::from(release.from),
            f64::from(release.to),
            f64::from(release.velocity),
        );
        let done = spring
            .as_ref()
            .is_none_or(|spring| elapsed >= spring.settle(from - to, velocity, 2.0));
        let effects = {
            let mut main = self.main();
            if main.drags.serials.get(&*release.identity) != Some(&serial)
                || main.interaction.drag_held(&release.identity)
            {
                return;
            }
            let at = match &spring {
                Some(spring) if !done => spring.at(from, to, velocity, elapsed).0 as f32,
                _ => release.to,
            };
            main.interaction
                .set_drag_offset(&release.identity, at, done);
            let mut effects = vec![crate::interaction::view::Effect::Redraw];
            if done {
                main.drags.serials.remove(&*release.identity);
                effects.extend(
                    main.interaction
                        .drag_settled(&release.identity, release.index),
                );
            }
            effects
        };
        self.deliver(effects);
        if !done {
            let inner = self.clone();
            self.adapter.post_after(
                STEP,
                Box::new(move || inner.drag_step(release, serial, elapsed + STEP.as_secs_f64())),
            );
        }
    }
}
