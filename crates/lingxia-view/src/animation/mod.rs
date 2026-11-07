//! Shared presentation-bound animation time. No platform clock or guest timers.
mod effect;
pub(crate) mod presence;
pub(crate) mod records;
pub mod transition;
mod wire;
pub use wire::decode_definition;
pub mod interpolate;
pub mod spring;
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
    fill: Fill,
}

/// CSS `animation-fill-mode`: whether the first keyframe shows during the delay and the last
/// one stays after the run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fill {
    pub backwards: bool,
    pub forwards: bool,
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
            fill: Fill::default(),
        })
    }

    pub fn with_fill(mut self, fill: Fill) -> Self {
        self.fill = fill;
        self
    }

    /// The progress the element shows: while running, or held by its fill mode around the run.
    pub fn shown(&self, sample: &Sample) -> Option<f64> {
        match sample.phase {
            Phase::Active => Some(sample.progress),
            Phase::Delayed if self.fill.backwards => Some(0.0),
            Phase::Finished if self.fill.forwards => Some(1.0),
            _ => None,
        }
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
    /// Keeps playing under reduced motion (authored for it explicitly).
    pub keep_reduced: bool,
    pub timing: Timing,
    pub effect: Effect,
}

#[derive(Clone, Debug)]
struct Instance {
    definition: Definition,
    started: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct Prepared {
    owner: Arc<()>,
    ticket: u64,
    prepared_at: Duration,
    /// Active time this frame sampled; a frame prepared ahead assumes its starts here.
    elapsed: Duration,
    /// Prepared while parked: its acknowledgement commits state but starts no time.
    visible: bool,
    instances: BTreeMap<String, Instance>,
    transitions: transition::Transitions,
    samples: BTreeMap<crate::scene::node::Identity, Sample>,
    visuals: BTreeMap<crate::scene::node::Identity, Visual>,
}

impl Prepared {
    pub(crate) fn contains_definition(&self, identity: &str, definition: &Definition) -> bool {
        self.instances
            .get(identity)
            .is_some_and(|instance| &instance.definition == definition)
    }
    /// Exit ghosts this frame draws, with the layout each was last committed in.
    pub(crate) fn ghosts(&self) -> Vec<(crate::scene::node::Identity, transition::Origin)> {
        self.transitions.ghosts(self.elapsed)
    }
    /// Nodes this frame moves toward a new layout position (`transition-layout`).
    pub(crate) fn layout_motions(&self) -> usize {
        self.transitions.layout_motions(self.elapsed)
    }

    /// Only active effects; absent entries use the node's unanimated base style.
    pub fn visuals(&self) -> &BTreeMap<crate::scene::node::Identity, Visual> {
        &self.visuals
    }
    pub fn samples(&self) -> &BTreeMap<crate::scene::node::Identity, Sample> {
        &self.samples
    }
}

/// The user's motion preference, taken from the environment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    /// Transitions complete at once; keyframe animations show their base style.
    pub reduced: bool,
    /// Multiplies every duration and delay; must be positive.
    pub scale: f64,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            reduced: false,
            scale: 1.0,
        }
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
    /// Tickets of prepared frames awaiting presentation, oldest first; a frame prepared
    /// ahead of an unpresented one builds on it and queues behind it.
    pending: std::collections::VecDeque<u64>,
    instances: BTreeMap<String, Instance>,
    transitions: transition::Transitions,
    motion: Motion,
    visuals: BTreeMap<crate::scene::node::Identity, Visual>,
    /// Host times of visible presentations that showed or cleared animated values.
    presents: std::collections::VecDeque<Duration>,
    /// The window is covered: looping keyframes wake no frames.
    covered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wakeup {
    None,
    NextFrame,
    After(Duration),
}

impl Wakeup {
    pub(crate) fn earliest(self, other: Self) -> Self {
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
            pending: self.pending.clone(),
            instances: self.instances.clone(),
            transitions: self.transitions.clone(),
            motion: self.motion,
            visuals: self.visuals.clone(),
            presents: self.presents.clone(),
            covered: self.covered,
        }
    }

    /// A looping keyframe animation shows nothing behind other windows and would run
    /// forever, so it parks while the window is covered; finite ones run to completion.
    pub(crate) fn visible(&self) -> bool {
        self.visible
    }

    pub fn set_covered(&mut self, covered: bool) {
        self.covered = covered;
    }

    /// Animated presentations in the second up to `now`; parked frames never count.
    pub fn frames_per_second(&self, now: Duration) -> usize {
        let since = now.saturating_sub(Duration::from_secs(1));
        self.presents.iter().filter(|at| **at > since).count()
    }

    pub fn motion(&self) -> Motion {
        self.motion
    }

    /// Takes effect from the next prepared frame.
    pub fn set_motion(&mut self, motion: Motion) -> Result<(), String> {
        if !motion.scale.is_finite() || !(0.01..=10.0).contains(&motion.scale) {
            return Err("UI_ANIMATION: invalid duration scale".into());
        }
        self.motion = motion;
        Ok(())
    }

    /// Keyframe time under the duration scale.
    fn local(&self, elapsed: Duration) -> Duration {
        elapsed.div_f64(self.motion.scale)
    }

    fn keyframe_visual(
        &self,
        instance: &Instance,
        active_time: Duration,
    ) -> Result<Option<Visual>, String> {
        let Some(started) = instance.started else {
            return Ok(None);
        };
        if self.motion.reduced && !instance.definition.keep_reduced {
            return Ok(None);
        }
        let sample = instance
            .definition
            .timing
            .sample(self.local(active_time.saturating_sub(started)));
        instance
            .definition
            .timing
            .shown(&sample)
            .map(|progress| instance.definition.effect.sample(progress))
            .transpose()
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
        // Exit ghosts are not in the View any more, so the caller cannot name them.
        let ghosts: Vec<String> = self
            .transitions
            .ghosts(active_time)
            .into_iter()
            .map(|(identity, _)| identity.to_string())
            .collect();
        let identities: Vec<String> = visible_identities
            .into_iter()
            .map(str::to_owned)
            .chain(ghosts)
            .collect();
        for identity in identities.iter().map(String::as_str) {
            let instance = self.instances.get(identity);
            if self.covered && instance.is_some_and(|i| i.definition.timing.iterations.is_none()) {
                next = next.earliest(self.transitions.wakeup(identity, active_time));
                continue;
            }
            let mut current = match instance {
                Some(instance) => self.keyframe_visual(instance, active_time)?,
                None => None,
            };
            let overrides = self.transitions.overrides(identity, active_time)?;
            if !overrides.is_empty() {
                current.get_or_insert_with(Visual::default).transition = overrides;
            }
            if current.as_ref() != self.visuals.get(identity) {
                next = next.earliest(Wakeup::NextFrame);
                continue;
            }
            next = next.earliest(self.transitions.wakeup(identity, active_time));
            let Some((instance, started)) =
                instance.and_then(|instance| Some((instance, instance.started?)))
            else {
                continue;
            };
            if self.motion.reduced && !instance.definition.keep_reduced {
                continue;
            }
            let scale = self.motion.scale;
            let elapsed = self.local(active_time.saturating_sub(started));
            let timing = instance.definition.timing;
            let sample = timing.sample(elapsed);
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
            next = next.earliest(match wakeup {
                Wakeup::After(delay) => Wakeup::After(delay.mul_f64(scale)),
                other => other,
            });
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
        self.pending.clear();
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
        self.prepare_window(now, now, definitions, Vec::new(), None, None)
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
        self.prepare_window(now, now, definitions, Vec::new(), Some(visible), None)
    }

    /// Like `prepare_visible`, also reconciling transitions against each node's resolved paint.
    /// A visible identity needs a keyframe definition or a transition.
    /// `sample` (≥ `now`) is when the frame is expected on screen. With `base`, the frame
    /// builds on that unpresented frame, as if it had presented at its own sample time.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_frame<'a>(
        &mut self,
        now: Duration,
        sample: Duration,
        definitions: impl IntoIterator<Item = (String, Definition)>,
        transitions: Vec<transition::Input>,
        visible_identities: impl IntoIterator<Item = &'a str>,
        base: Option<&Prepared>,
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
        self.prepare_window(now, sample, definitions, transitions, Some(visible), base)
    }

    fn prepare_window(
        &mut self,
        now: Duration,
        sample: Duration,
        definitions: impl IntoIterator<Item = (String, Definition)>,
        transitions: Vec<transition::Input>,
        mut visible: Option<std::collections::BTreeSet<String>>,
        base: Option<&Prepared>,
    ) -> Result<Prepared, String> {
        self.time_at(now)?;
        let elapsed = self.time_at(sample.max(now))?;
        let base = match base {
            Some(base)
                if self.visible && base.visible && self.pending.back() == Some(&base.ticket) =>
            {
                let mut assumed = base.clone();
                for (identity, instance) in &mut assumed.instances {
                    if assumed.samples.contains_key(identity.as_str()) {
                        instance.started.get_or_insert(assumed.elapsed);
                    }
                }
                assumed.transitions.start(assumed.elapsed);
                Some(assumed)
            }
            Some(_) => return Err("UI_ANIMATION: stale frame to build on".into()),
            None => None,
        };
        let (old_instances, old_transitions) = match &base {
            Some(base) => (&base.instances, &base.transitions),
            None => (&self.instances, &self.transitions),
        };
        let transitions = old_transitions.reconcile(
            transitions,
            elapsed,
            transition::Policy {
                start: self.visible && !self.motion.reduced,
                keep: !self.motion.reduced,
                scale: self.motion.scale,
            },
        )?;
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
            let started = old_instances
                .get(&identity)
                .filter(|old| old.definition == definition)
                .and_then(|old| old.started);
            if visible.as_mut().is_none_or(|ids| ids.remove(&identity)) {
                let sample = definition.timing.sample(
                    self.local(
                        started
                            .map(|start| elapsed.saturating_sub(start))
                            .unwrap_or_default(),
                    ),
                );
                let key = crate::scene::node::Identity::from(identity.as_str());
                if let Some(progress) = definition.timing.shown(&sample)
                    && (!self.motion.reduced || definition.keep_reduced)
                {
                    visuals.insert(key.clone(), definition.effect.sample(progress)?);
                }
                samples.insert(key, sample);
            }
            instances.insert(
                identity,
                Instance {
                    definition,
                    started,
                },
            );
        }
        if let Some(ids) = &mut visible {
            ids.retain(|id| !transitions.contains(id));
        }
        if visible.is_some_and(|ids| !ids.is_empty()) {
            return Err("UI_ANIMATION: visible identity has no definition".into());
        }
        for identity in transitions.identities() {
            let overrides = transitions.overrides(identity, elapsed)?;
            if !overrides.is_empty() {
                visuals
                    .entry(identity.clone())
                    .or_insert_with(Visual::default)
                    .transition = overrides;
            }
        }
        if base.is_some() {
            self.ticket = self
                .ticket
                .checked_add(1)
                .ok_or("UI_ANIMATION: exhausted frame tickets")?;
        } else {
            self.invalidate()?;
        }
        self.pending.push_back(self.ticket);
        Ok(Prepared {
            owner: self.owner.clone(),
            ticket: self.ticket,
            prepared_at: now,
            elapsed,
            visible: self.visible,
            instances,
            transitions,
            samples,
            visuals,
        })
    }

    /// Forget a prepared frame that will not be presented, if it is the newest.
    pub(crate) fn discard(&mut self, frame: &Prepared) {
        if self.pending.back() == Some(&frame.ticket) {
            self.pending.pop_back();
        }
    }

    pub(crate) fn validate_presentation(
        &self,
        frame: &Prepared,
        now: Duration,
    ) -> Result<Duration, String> {
        if !Arc::ptr_eq(&self.owner, &frame.owner)
            || self.visible != frame.visible
            || !self.pending.contains(&frame.ticket)
            || now < frame.prepared_at
        {
            return Err("UI_ANIMATION: stale or hidden presentation".into());
        }
        self.time_at(now)
    }

    /// Call only after the matching page/surface frame actually presented.
    pub fn presented(&mut self, mut frame: Prepared, now: Duration) -> Result<(), String> {
        let elapsed = self.validate_presentation(&frame, now)?;
        // A parked frame shows state without starting entrances or transitions.
        if frame.visible {
            for (identity, instance) in &mut frame.instances {
                if frame.samples.contains_key(identity.as_str()) {
                    instance.started.get_or_insert(elapsed);
                }
            }
            frame.transitions.start(elapsed);
            if !frame.visuals.is_empty() || !self.visuals.is_empty() {
                let since = now.saturating_sub(Duration::from_secs(1));
                while self.presents.front().is_some_and(|at| *at <= since) {
                    self.presents.pop_front();
                }
                if self.presents.len() < 1024 {
                    self.presents.push_back(now);
                }
            }
        }
        self.instances = frame.instances;
        self.transitions = frame.transitions;
        self.visuals = frame.visuals;
        // Frames queued before this one can no longer present.
        while self
            .pending
            .pop_front()
            .is_some_and(|ticket| ticket != frame.ticket)
        {}
        self.elapsed = elapsed;
        self.observed_at = now;
        Ok(())
    }

    pub fn reset(&mut self, now: Duration) -> Result<(), String> {
        self.time_at(now)?;
        self.invalidate()?;
        self.instances.clear();
        self.transitions = Default::default();
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
            keep_reduced: false,
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
    fn fill_modes_hold_the_first_keyframe_before_and_the_last_after() {
        let timing = effect().timing;
        let held = |fill: Fill, at| timing.with_fill(fill).shown(&timing.sample(ms(at)));
        assert_eq!(held(Fill::default(), 0), None);
        assert_eq!(held(Fill::default(), 500), None);
        let both = Fill {
            backwards: true,
            forwards: true,
        };
        assert_eq!(held(both, 0), Some(0.0));
        assert_eq!(held(both, 70), Some(0.5));
        assert_eq!(held(both, 500), Some(1.0));
        let forwards = Fill {
            backwards: false,
            forwards: true,
        };
        assert_eq!(held(forwards, 0), None);
        assert_eq!(held(forwards, 500), Some(1.0));
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
    fn a_frame_builds_only_on_the_newest_pending_frame_of_a_visible_timeline() {
        let mut t = Timeline::default();
        t.set_visible(true, ms(0)).unwrap();
        let frame = |t: &mut Timeline, at: u64, base: Option<&Prepared>| {
            t.prepare_frame(
                ms(at),
                ms(at + 16),
                [("chart".into(), effect())],
                vec![],
                ["chart"],
                base,
            )
        };
        let stale = |result: Result<Prepared, String>| {
            assert_eq!(result.unwrap_err(), "UI_ANIMATION: stale frame to build on");
        };
        let a = frame(&mut t, 0, None).unwrap();
        let b = frame(&mut t, 0, Some(&a)).unwrap();
        assert!(b.ticket > a.ticket);
        // Only the newest waiting frame is a base; an older one would fork the queue.
        stale(frame(&mut t, 0, Some(&a)));
        let c = frame(&mut t, 0, Some(&b)).unwrap();
        t.presented(b, ms(16)).unwrap();
        let d = frame(&mut t, 16, Some(&c)).unwrap();
        // A visibility flip drops every waiting frame, even after it flips back.
        t.set_visible(false, ms(20)).unwrap();
        stale(frame(&mut t, 20, Some(&d)));
        t.set_visible(true, ms(30)).unwrap();
        stale(frame(&mut t, 30, Some(&d)));
        assert!(t.presented(d, ms(40)).is_err());
        let e = frame(&mut t, 40, None).unwrap();
        frame(&mut t, 40, Some(&e)).unwrap();
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
    fn other_pages_stale_hidden_frames_and_budget_failures_cannot_commit() {
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
        // A frame prepared while parked commits without starting anything.
        let parked = b.prepare(ms(100), [("new".into(), effect())]).unwrap();
        b.presented(parked, ms(100)).unwrap();
        b.set_visible(true, ms(500)).unwrap();
        let shown = b.prepare(ms(500), [("new".into(), effect())]).unwrap();
        assert_eq!(shown.samples()["new"].phase, Phase::Delayed);
        assert_eq!(shown.samples()["new"].until_boundary, Some(ms(20)));
    }

    #[test]
    fn duration_scale_stretches_keyframe_time_and_wakeups_and_reduced_motion_shows_base() {
        let mut t = Timeline::default();
        t.set_motion(Motion {
            reduced: false,
            scale: 2.0,
        })
        .unwrap();
        t.set_visible(true, ms(0)).unwrap();
        let first = prepare(&mut t, 0, effect());
        t.presented(first, ms(0)).unwrap();
        assert_eq!(
            t.next_wakeup(ms(0), ["chart"]).unwrap(),
            Wakeup::After(ms(40))
        );
        assert_eq!(
            prepare(&mut t, 140, effect()).samples()["chart"].progress,
            0.5
        );
        t.set_motion(Motion {
            reduced: true,
            scale: 2.0,
        })
        .unwrap();
        let reduced = prepare(&mut t, 140, effect());
        assert!(reduced.visuals().is_empty());
        t.presented(reduced, ms(140)).unwrap();
        assert_eq!(t.next_wakeup(ms(140), ["chart"]).unwrap(), Wakeup::None);
        assert!(
            t.set_motion(Motion {
                reduced: false,
                scale: 0.0
            })
            .is_err()
        );
    }

    fn blink() -> Definition {
        let keys = [(0.0, 1.0), (0.49, 1.0), (0.5, 0.0), (1.0, 0.0)]
            .map(|(offset, value)| interpolate::Keyframe { offset, value })
            .to_vec();
        Definition {
            keep_reduced: false,
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
            keep_reduced: false,
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
            keep_reduced: false,
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
