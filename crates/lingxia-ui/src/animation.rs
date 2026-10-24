//! Shared presentation-bound animation time. No platform clock or guest timers.
mod effect;
pub(crate) mod records;
mod wire;
pub use wire::decode_definition;
pub mod interpolate;
pub use effect::{Effect, TransformTracks, Visual};
use std::{collections::BTreeMap, num::NonZeroU32, sync::Arc, time::Duration};

const MAX_INSTANCES: usize = 4096;
const MAX_EFFECT_BYTES: usize = 262_144;
const MAX_INTERVAL: Duration = Duration::from_secs(86_400);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timing {
    duration: Duration,
    delay: Duration,
    iterations: Option<NonZeroU32>,
}

impl Timing {
    /// `None` iterations repeats until the owning node is removed.
    pub fn new(
        duration: Duration,
        delay: Duration,
        iterations: Option<NonZeroU32>,
    ) -> Result<Self, String> {
        if duration < Duration::from_millis(1) || duration > MAX_INTERVAL || delay > MAX_INTERVAL {
            return Err("UI_ANIMATION: interval outside bounds".into());
        }
        Ok(Self {
            duration,
            delay,
            iterations,
        })
    }

    pub fn sample(self, elapsed: Duration) -> Sample {
        if elapsed < self.delay {
            return Sample {
                progress: 0.0,
                phase: Phase::Delayed,
                until_boundary: Some(self.delay - elapsed),
            };
        }
        let elapsed = elapsed - self.delay;
        let nanos = elapsed.as_nanos();
        let duration = self.duration.as_nanos();
        if self
            .iterations
            .is_some_and(|count| nanos >= duration * u128::from(count.get()))
        {
            return Sample {
                progress: 1.0,
                phase: Phase::Finished,
                until_boundary: None,
            };
        }
        let remainder = Duration::from_nanos((nanos % duration) as u64);
        Sample {
            progress: remainder.as_secs_f64() / self.duration.as_secs_f64(),
            phase: Phase::Active,
            until_boundary: Some(self.duration - remainder),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Delayed,
    Active,
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub progress: f64,
    pub phase: Phase,
    /// Delay/iteration boundary, not a request for a continuous render loop.
    pub until_boundary: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub name: String,
    pub timing: Timing,
    pub effect: Effect,
}

#[derive(Clone, Debug)]
struct Instance {
    definition: Definition,
    started: Option<Duration>,
}

#[derive(Debug)]
pub struct Prepared {
    owner: Arc<()>,
    ticket: u64,
    prepared_at: Duration,
    instances: BTreeMap<String, Instance>,
    samples: BTreeMap<String, Sample>,
    visuals: BTreeMap<String, Visual>,
}

impl Prepared {
    pub(crate) fn contains_definition(&self, identity: &str, definition: &Definition) -> bool {
        self.instances
            .get(identity)
            .is_some_and(|instance| &instance.definition == definition)
    }
    /// Only active effects; absent entries use the node's unanimated base style.
    pub fn visuals(&self) -> &BTreeMap<String, Visual> {
        &self.visuals
    }
    pub fn samples(&self) -> &BTreeMap<String, Sample> {
        &self.samples
    }
}

/// One timeline per page. Timestamps are monotonic durations from a host epoch.
#[derive(Debug, Default)]
pub struct Timeline {
    owner: Arc<()>,
    visible: bool,
    observed_at: Duration,
    elapsed: Duration,
    ticket: u64,
    pending: Option<u64>,
    instances: BTreeMap<String, Instance>,
    visuals: BTreeMap<String, Visual>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wakeup {
    None,
    NextFrame,
    After(Duration),
}

impl Wakeup {
    fn earliest(self, other: Self) -> Self {
        match (self, other) {
            (Self::NextFrame, _) | (_, Self::NextFrame) => Self::NextFrame,
            (Self::After(a), Self::After(b)) => Self::After(a.min(b)),
            (Self::None, next) | (next, Self::None) => next,
        }
    }
}

impl Timeline {
    // Only the owning session may fork a transaction; independent pages must
    // never share the owner identity or accept one another's prepared frames.
    pub(crate) fn fork(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            visible: self.visible,
            observed_at: self.observed_at,
            elapsed: self.elapsed,
            ticket: self.ticket,
            pending: self.pending,
            instances: self.instances.clone(),
            visuals: self.visuals.clone(),
        }
    }

    pub(crate) fn clear(&mut self) -> Result<(), String> {
        self.reset(self.observed_at)
    }

    /// Query committed state after presentation. Offscreen retained identities
    /// keep their lifetime but must be omitted from `visible_identities`.
    pub fn next_wakeup<'a>(
        &self,
        now: Duration,
        visible_identities: impl IntoIterator<Item = &'a str>,
    ) -> Result<Wakeup, String> {
        let active_time = self.time_at(now)?;
        if !self.visible {
            return Ok(Wakeup::None);
        }
        let mut next = Wakeup::None;
        for identity in visible_identities {
            let Some(instance) = self.instances.get(identity) else {
                continue;
            };
            let Some(started) = instance.started else {
                continue;
            };
            let elapsed = active_time.saturating_sub(started);
            let timing = instance.definition.timing;
            let sample = timing.sample(elapsed);
            let current = if sample.phase == Phase::Active {
                Some(instance.definition.effect.sample(sample.progress)?)
            } else {
                None
            };
            if current.as_ref() != self.visuals.get(identity) {
                next = next.earliest(Wakeup::NextFrame);
                continue;
            }
            let wakeup = match sample.phase {
                Phase::Finished => Wakeup::None,
                Phase::Delayed => Wakeup::After(sample.until_boundary.unwrap()),
                Phase::Active => match instance.definition.effect.next_change(sample.progress)? {
                    interpolate::NextChange::Continuous => Wakeup::NextFrame,
                    interpolate::NextChange::At(at) => Wakeup::After(
                        timing
                            .duration
                            .mul_f64((at - sample.progress).max(0.0))
                            .max(Duration::from_nanos(1)),
                    ),
                    interpolate::NextChange::None => {
                        if instance.definition.effect.is_constant() {
                            match timing.iterations {
                                Some(count) => Wakeup::After(
                                    timing.delay + timing.duration * count.get() - elapsed,
                                ),
                                None => Wakeup::None,
                            }
                        } else {
                            Wakeup::After(sample.until_boundary.unwrap())
                        }
                    }
                },
            };
            next = next.earliest(wakeup);
        }
        Ok(next)
    }
    fn time_at(&self, now: Duration) -> Result<Duration, String> {
        let delta = now
            .checked_sub(self.observed_at)
            .ok_or("UI_ANIMATION: clock moved backwards")?;
        self.elapsed
            .checked_add(if self.visible { delta } else { Duration::ZERO })
            .ok_or_else(|| "UI_ANIMATION: clock overflow".into())
    }

    fn invalidate(&mut self) -> Result<(), String> {
        self.ticket = self
            .ticket
            .checked_add(1)
            .ok_or("UI_ANIMATION: exhausted frame tickets")?;
        self.pending = None;
        Ok(())
    }

    pub fn set_visible(&mut self, visible: bool, now: Duration) -> Result<(), String> {
        let elapsed = self.time_at(now)?;
        if self.visible != visible {
            self.invalidate()?;
        }
        self.elapsed = elapsed;
        self.observed_at = now;
        self.visible = visible;
        Ok(())
    }

    /// Dropping a prepared frame never installs its additions or removals.
    pub fn prepare(
        &mut self,
        now: Duration,
        definitions: impl IntoIterator<Item = (String, Definition)>,
    ) -> Result<Prepared, String> {
        self.prepare_window(now, definitions, None)
    }

    /// Definitions describe semantic lifetime, including offscreen list items.
    /// Only visible identities are sampled and start on successful presentation.
    pub fn prepare_visible<'a>(
        &mut self,
        now: Duration,
        definitions: impl IntoIterator<Item = (String, Definition)>,
        visible_identities: impl IntoIterator<Item = &'a str>,
    ) -> Result<Prepared, String> {
        let mut visible = std::collections::BTreeSet::new();
        for identity in visible_identities {
            if identity.is_empty() || identity.len() > 1024 || visible.len() == MAX_INSTANCES {
                return Err("UI_ANIMATION: visible identity budget exceeded".into());
            }
            if !visible.insert(identity.to_owned()) {
                return Err("UI_ANIMATION: duplicate visible identity".into());
            }
        }
        self.prepare_window(now, definitions, Some(visible))
    }

    fn prepare_window(
        &mut self,
        now: Duration,
        definitions: impl IntoIterator<Item = (String, Definition)>,
        mut visible: Option<std::collections::BTreeSet<String>>,
    ) -> Result<Prepared, String> {
        let elapsed = self.time_at(now)?;
        let mut instances = BTreeMap::new();
        let mut samples = BTreeMap::new();
        let mut visuals = BTreeMap::new();
        let mut effect_bytes = 0usize;
        for (identity, definition) in definitions {
            effect_bytes = effect_bytes
                .checked_add(definition.effect.storage_bytes() + definition.name.len())
                .ok_or("UI_ANIMATION: effect budget exceeded")?;
            if instances.len() == MAX_INSTANCES
                || definition.name.is_empty()
                || definition.name.len() > 128
                || identity.is_empty()
                || identity.len() > 1024
                || definition.effect.storage_bytes() > 32768
                || effect_bytes > MAX_EFFECT_BYTES
            {
                return Err("UI_ANIMATION: instance budget exceeded".into());
            }
            if instances.contains_key(&identity) {
                return Err("UI_ANIMATION: duplicate identity".into());
            }
            let started = self
                .instances
                .get(&identity)
                .filter(|old| old.definition == definition)
                .and_then(|old| old.started);
            if visible.as_mut().is_none_or(|ids| ids.remove(&identity)) {
                let sample = definition.timing.sample(
                    started
                        .map(|start| elapsed.saturating_sub(start))
                        .unwrap_or_default(),
                );
                if sample.phase == Phase::Active {
                    visuals.insert(identity.clone(), definition.effect.sample(sample.progress)?);
                }
                samples.insert(identity.clone(), sample);
            }
            instances.insert(
                identity,
                Instance {
                    definition,
                    started,
                },
            );
        }
        if visible.is_some_and(|ids| !ids.is_empty()) {
            return Err("UI_ANIMATION: visible identity has no definition".into());
        }
        self.invalidate()?;
        self.pending = Some(self.ticket);
        Ok(Prepared {
            owner: self.owner.clone(),
            ticket: self.ticket,
            prepared_at: now,
            instances,
            samples,
            visuals,
        })
    }

    pub(crate) fn validate_presentation(
        &self,
        frame: &Prepared,
        now: Duration,
    ) -> Result<Duration, String> {
        if !Arc::ptr_eq(&self.owner, &frame.owner)
            || !self.visible
            || self.pending != Some(frame.ticket)
            || now < frame.prepared_at
        {
            return Err("UI_ANIMATION: stale or hidden presentation".into());
        }
        self.time_at(now)
    }

    /// Call only after the matching page/surface frame actually presented.
    pub fn presented(&mut self, mut frame: Prepared, now: Duration) -> Result<(), String> {
        let elapsed = self.validate_presentation(&frame, now)?;
        for (identity, instance) in &mut frame.instances {
            if frame.samples.contains_key(identity) {
                instance.started.get_or_insert(elapsed);
            }
        }
        self.instances = frame.instances;
        self.visuals = frame.visuals;
        self.pending = None;
        self.elapsed = elapsed;
        self.observed_at = now;
        Ok(())
    }

    pub fn reset(&mut self, now: Duration) -> Result<(), String> {
        self.time_at(now)?;
        self.invalidate()?;
        self.instances.clear();
        self.visuals.clear();
        self.elapsed = Duration::ZERO;
        self.observed_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }
    fn opacity(from: f64, to: f64) -> Effect {
        Effect::new(
            Some(
                interpolate::Track::new(
                    vec![
                        interpolate::Keyframe {
                            offset: 0.0,
                            value: from,
                        },
                        interpolate::Keyframe {
                            offset: 1.0,
                            value: to,
                        },
                    ],
                    interpolate::Easing::LINEAR,
                )
                .unwrap(),
            ),
            vec![],
        )
        .unwrap()
    }
    fn effect() -> Definition {
        Definition {
            name: "effect".into(),
            timing: Timing::new(ms(100), ms(20), NonZeroU32::new(2)).unwrap(),
            effect: opacity(0.0, 1.0),
        }
    }
    fn prepare(t: &mut Timeline, now: u64, definition: Definition) -> Prepared {
        t.prepare(ms(now), [("chart".into(), definition)]).unwrap()
    }

    #[test]
    fn boundaries_delays_repeats_and_large_time_jumps() {
        let timing = effect().timing;
        assert_eq!(timing.sample(ms(19)).phase, Phase::Delayed);
        assert_eq!(timing.sample(ms(19)).until_boundary, Some(ms(1)));
        assert_eq!(timing.sample(ms(20)).progress, 0.0);
        assert_eq!(timing.sample(ms(70)).progress, 0.5);
        assert_eq!(timing.sample(ms(120)).progress, 0.0);
        assert_eq!(timing.sample(ms(220)).phase, Phase::Finished);
        assert_eq!(timing.sample(Duration::MAX).until_boundary, None);
        let repeating = Timing::new(ms(100), Duration::ZERO, None).unwrap();
        assert_eq!(repeating.sample(ms(10_050)).progress, 0.5);
        assert_eq!(repeating.sample(Duration::MAX).phase, Phase::Active);
    }

    #[test]
    fn offscreen_insertions_wait_for_their_first_visible_presentation() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let definitions = || [("chart".into(), effect())];
        let hidden = t.prepare_visible(ms(0), definitions(), []).unwrap();
        assert!(hidden.samples().is_empty());
        assert!(hidden.visuals().is_empty());
        t.presented(hidden, ms(50)).unwrap();
        assert_eq!(t.next_wakeup(ms(500), []).unwrap(), Wakeup::None);

        let first = t
            .prepare_visible(ms(500), definitions(), ["chart"])
            .unwrap();
        assert_eq!(first.samples()["chart"].phase, Phase::Delayed);
        t.presented(first, ms(600)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(600), ["chart"]).unwrap(),
            Wakeup::After(ms(20))
        );
        let active = t
            .prepare_visible(ms(670), definitions(), ["chart"])
            .unwrap();
        assert_eq!(active.samples()["chart"].progress, 0.5);
    }

    #[test]
    fn virtual_eviction_retains_started_time_without_wakeups_or_reentry() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let definitions = || [("chart".into(), effect())];
        let first = t.prepare_visible(ms(0), definitions(), ["chart"]).unwrap();
        t.presented(first, ms(0)).unwrap();
        let evicted = t.prepare_visible(ms(50), definitions(), []).unwrap();
        t.presented(evicted, ms(50)).unwrap();
        assert_eq!(t.next_wakeup(ms(100), []).unwrap(), Wakeup::None);
        t.set_visible(false, ms(100)).unwrap();
        t.set_visible(true, ms(1000)).unwrap();
        let resumed = t
            .prepare_visible(ms(1000), definitions(), ["chart"])
            .unwrap();
        assert!((resumed.samples()["chart"].progress - 0.8).abs() < 1e-12);
        t.presented(resumed, ms(1000)).unwrap();
        let done = t
            .prepare_visible(ms(1200), definitions(), ["chart"])
            .unwrap();
        assert_eq!(done.samples()["chart"].phase, Phase::Finished);
        assert!(done.visuals().is_empty());
        t.presented(done, ms(1200)).unwrap();
        assert_eq!(t.next_wakeup(ms(1200), ["chart"]).unwrap(), Wakeup::None);

        let removed = t.prepare_visible(ms(1200), [], []).unwrap();
        t.presented(removed, ms(1200)).unwrap();
        let reinserted = t
            .prepare_visible(ms(1300), definitions(), ["chart"])
            .unwrap();
        assert_eq!(reinserted.samples()["chart"].phase, Phase::Delayed);
    }

    #[test]
    fn invalid_visible_windows_preserve_pending_presentation() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let first = prepare(&mut t, 0, effect());
        assert!(t.prepare_visible(ms(1), [], ["unknown"]).is_err());
        assert!(t.prepare_visible(ms(1), [], ["chart", "chart"]).is_err());
        assert!(t.prepare_visible(ms(1), [], [""]).is_err());
        t.presented(first, ms(2)).unwrap();
        assert_eq!(
            prepare(&mut t, 72, effect()).samples()["chart"].progress,
            0.5
        );
    }

    #[test]
    fn first_presentation_starts_time_and_rejected_candidates_do_not_replace_identity() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let rejected = prepare(&mut t, 10, effect());
        drop(rejected);
        let first = prepare(&mut t, 500, effect());
        assert_eq!(first.samples()["chart"].phase, Phase::Delayed);
        t.presented(first, ms(600)).unwrap();
        assert_eq!(
            prepare(&mut t, 670, effect()).samples()["chart"].progress,
            0.5
        );
        let mut replacement = effect();
        replacement.effect = opacity(1.0, 0.0);
        drop(prepare(&mut t, 680, replacement));
        assert!((prepare(&mut t, 690, effect()).samples()["chart"].progress - 0.7).abs() < 1e-12);
    }

    #[test]
    fn changing_animation_name_restarts_even_when_tracks_are_identical() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let first = prepare(&mut t, 0, effect());
        t.presented(first, ms(0)).unwrap();
        assert_eq!(
            prepare(&mut t, 70, effect()).samples()["chart"].progress,
            0.5
        );
        let mut renamed = effect();
        renamed.name = "replacement".into();
        let frame = prepare(&mut t, 70, renamed.clone());
        assert_eq!(frame.samples()["chart"].phase, Phase::Delayed);
        t.presented(frame, ms(80)).unwrap();
        assert_eq!(
            prepare(&mut t, 150, renamed).samples()["chart"].progress,
            0.5
        );
    }

    #[test]
    fn pause_freezes_time_and_invalidates_prepared_presentations() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let first = prepare(&mut t, 0, effect());
        t.presented(first, ms(0)).unwrap();
        let stale = prepare(&mut t, 50, effect());
        t.set_visible(false, ms(70)).unwrap();
        t.set_visible(true, ms(1_000)).unwrap();
        assert!(t.presented(stale, ms(1_000)).is_err());
        let resumed = prepare(&mut t, 1_000, effect());
        assert_eq!(resumed.samples()["chart"].progress, 0.5);
        t.presented(resumed, ms(1_000)).unwrap();
        assert_eq!(
            prepare(&mut t, 1_010, effect()).samples()["chart"].progress,
            0.6
        );
    }

    #[test]
    fn superseded_removal_reset_and_changed_definition_have_distinct_lifetimes() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let stale = prepare(&mut t, 0, effect());
        let first = prepare(&mut t, 10, effect());
        assert!(t.presented(stale, ms(10)).is_err());
        t.presented(first, ms(10)).unwrap();
        drop(t.prepare(ms(50), []).unwrap());
        assert_eq!(
            prepare(&mut t, 80, effect()).samples()["chart"].progress,
            0.5
        );
        let removal = t.prepare(ms(80), []).unwrap();
        t.presented(removal, ms(80)).unwrap();
        let inserted = prepare(&mut t, 100, effect());
        t.presented(inserted, ms(100)).unwrap();
        let mut replacement = effect();
        replacement.effect = opacity(1.0, 0.0);
        let changed = prepare(&mut t, 170, replacement.clone());
        assert_eq!(changed.samples()["chart"].phase, Phase::Delayed);
        t.presented(changed, ms(180)).unwrap();
        assert_eq!(
            prepare(&mut t, 250, replacement).samples()["chart"].progress,
            0.5
        );
        let stale = prepare(&mut t, 260, effect());
        t.reset(ms(270)).unwrap();
        assert!(t.presented(stale, ms(280)).is_err());
        assert_eq!(
            prepare(&mut t, 280, effect()).samples()["chart"].phase,
            Phase::Delayed
        );
    }

    #[test]
    fn invalid_input_does_not_consume_a_valid_pending_frame() {
        assert!(Timing::new(Duration::ZERO, ms(0), None).is_err());
        assert!(Timing::new(ms(1), Duration::MAX, None).is_err());
        let mut t = Timeline::default();
        t.set_visible(true, ms(10)).unwrap();
        let frame = prepare(&mut t, 10, effect());
        assert!(
            t.prepare(ms(11), [("x".into(), effect()), ("x".into(), effect())])
                .is_err()
        );
        assert!(t.prepare(ms(9), []).is_err());
        t.presented(frame, ms(12)).unwrap();
        assert_eq!(
            prepare(&mut t, 82, effect()).samples()["chart"].progress,
            0.5
        );
    }

    #[test]
    fn other_pages_hidden_frames_and_budget_failures_cannot_commit() {
        let mut a = Timeline::default();
        let mut b = Timeline::default();
        a.set_visible(true, ms(0)).unwrap();
        b.set_visible(true, ms(0)).unwrap();
        let foreign = prepare(&mut a, 0, effect());
        let own = prepare(&mut b, 0, effect());
        assert!(b.presented(foreign, ms(1)).is_err());
        let mut large = effect();
        let track = interpolate::Track::new(
            (0..64)
                .map(|i| interpolate::Keyframe {
                    offset: i as f64 / 63.0,
                    value: 0.5,
                })
                .collect(),
            interpolate::Easing::LINEAR,
        )
        .unwrap();
        large.effect = Effect::new(
            Some(track.clone()),
            vec![
                TransformTracks::Scale {
                    x: track.clone(),
                    y: track
                };
                8
            ],
        )
        .unwrap();
        assert!(
            b.prepare(ms(1), (0..20).map(|i| (i.to_string(), large.clone())))
                .is_err()
        );
        b.presented(own, ms(1)).unwrap();
        let hidden = prepare(&mut b, 20, effect());
        b.set_visible(false, ms(20)).unwrap();
        assert!(b.presented(hidden, ms(21)).is_err());
        let hidden = prepare(&mut b, 100, effect());
        assert!(b.presented(hidden, ms(100)).is_err());
    }

    fn blink() -> Definition {
        let keys = [(0.0, 1.0), (0.49, 1.0), (0.5, 0.0), (1.0, 0.0)]
            .map(|(offset, value)| interpolate::Keyframe { offset, value })
            .to_vec();
        Definition {
            name: "blink".into(),
            timing: Timing::new(ms(900), Duration::ZERO, None).unwrap(),
            effect: Effect::new(
                Some(interpolate::Track::new(keys, interpolate::Easing::STEP_END).unwrap()),
                vec![],
            )
            .unwrap(),
        }
    }

    #[test]
    fn blink_prepares_pixels_and_schedules_only_real_changes() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let first = prepare(&mut t, 0, blink());
        assert_eq!(first.visuals()["chart"].opacity, Some(1.0));
        t.presented(first, ms(100)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(100), ["chart"]).unwrap(),
            Wakeup::After(ms(450))
        );
        assert_eq!(
            t.next_wakeup(ms(549), ["chart"]).unwrap(),
            Wakeup::After(ms(1))
        );
        assert_eq!(
            t.next_wakeup(ms(550), ["chart"]).unwrap(),
            Wakeup::NextFrame
        );
        let hidden = prepare(&mut t, 550, blink());
        assert_eq!(hidden.visuals()["chart"].opacity, Some(0.0));
        t.presented(hidden, ms(550)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(550), ["chart"]).unwrap(),
            Wakeup::After(ms(450))
        );
        assert_eq!(t.next_wakeup(ms(600), []).unwrap(), Wakeup::None);
        t.set_visible(false, ms(600)).unwrap();
        assert_eq!(t.next_wakeup(ms(900), ["chart"]).unwrap(), Wakeup::None);
        t.set_visible(true, ms(900)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(900), ["chart"]).unwrap(),
            Wakeup::After(ms(400))
        );
    }

    #[test]
    fn late_presentation_cannot_skip_a_step_or_leave_a_finished_effect_painted() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let first = prepare(&mut t, 0, blink());
        t.presented(first, ms(0)).unwrap();
        let late = prepare(&mut t, 400, blink());
        t.presented(late, ms(600)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(600), ["chart"]).unwrap(),
            Wakeup::NextFrame
        );
        let current = prepare(&mut t, 600, blink());
        t.presented(current, ms(600)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(600), ["chart"]).unwrap(),
            Wakeup::After(ms(300))
        );
        let constant = Definition {
            name: "constant".into(),
            timing: Timing::new(ms(100), Duration::ZERO, NonZeroU32::new(2)).unwrap(),
            effect: opacity(0.5, 0.5),
        };
        let first = prepare(&mut t, 700, constant.clone());
        t.presented(first, ms(700)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(700), ["chart"]).unwrap(),
            Wakeup::After(ms(200))
        );
        assert_eq!(
            t.next_wakeup(ms(900), ["chart"]).unwrap(),
            Wakeup::NextFrame
        );
        let finished = prepare(&mut t, 900, constant);
        assert!(finished.visuals().is_empty());
        t.presented(finished, ms(900)).unwrap();
        assert_eq!(t.next_wakeup(ms(900), ["chart"]).unwrap(), Wakeup::None);
    }

    #[test]
    fn sampling_failure_preserves_pending_frame_and_opacity_overshoot_is_clamped() {
        let overshoot = interpolate::Easing::cubic_bezier([0.0, 2.0, 1.0, 2.0]).unwrap();
        let track = interpolate::Track::new(
            vec![
                interpolate::Keyframe {
                    offset: 0.0,
                    value: 0.0,
                },
                interpolate::Keyframe {
                    offset: 1.0,
                    value: 1.0,
                },
            ],
            overshoot,
        )
        .unwrap();
        assert_eq!(
            Effect::new(Some(track), vec![])
                .unwrap()
                .sample(0.5)
                .unwrap()
                .opacity,
            Some(1.0)
        );
        let scale = interpolate::Track::new(
            vec![
                interpolate::Keyframe {
                    offset: 0.0,
                    value: 1.0,
                },
                interpolate::Keyframe {
                    offset: 1.0,
                    value: 16.0,
                },
            ],
            overshoot,
        )
        .unwrap();
        let invalid_midpoint = Definition {
            name: "invalid-midpoint".into(),
            timing: Timing::new(ms(100), Duration::ZERO, None).unwrap(),
            effect: Effect::new(
                None,
                vec![TransformTracks::Scale {
                    x: scale.clone(),
                    y: scale,
                }],
            )
            .unwrap(),
        };
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let first = prepare(&mut t, 0, invalid_midpoint.clone());
        t.presented(first, ms(0)).unwrap();
        let valid = prepare(&mut t, 20, effect());
        assert!(
            t.prepare(ms(50), [("chart".into(), invalid_midpoint)])
                .is_err()
        );
        t.presented(valid, ms(50)).unwrap();
    }
}
