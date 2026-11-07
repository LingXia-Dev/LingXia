//! Implicit transitions between natively resolved paint values (field 171).
use super::interpolate::Easing;
use super::spring::Spring;
use super::wire::Ease;
use serde::Deserialize;
use std::{collections::BTreeMap, time::Duration};
use vello_cpu::kurbo::Affine;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Property {
    Opacity,
    Transform,
    Background,
    Foreground,
    Border,
    Shadow,
    /// FLIP: the offset from where layout last put the node, animating to zero.
    Layout,
}

/// The paint properties; `Layout` is reconciled on its own.
const PROPERTIES: [Property; 6] = [
    Property::Opacity,
    Property::Transform,
    Property::Background,
    Property::Foreground,
    Property::Border,
    Property::Shadow,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionDef {
    properties: [bool; 7],
    duration: Duration,
    delay: Duration,
    timing: Timing,
}

/// A curve over the duration, or a spring whose duration is perceptual.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Timing {
    Ease(Easing),
    Spring(f64),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Wire {
    properties: Vec<String>,
    duration_ms: f64,
    delay_ms: f64,
    easing: Ease,
}

impl TransitionDef {
    pub fn decode(source: &str) -> Result<Self, String> {
        let invalid = || "UI_TRANSITION: invalid transition".to_string();
        if source.len() > 512 {
            return Err(invalid());
        }
        let wire: Wire = serde_json::from_str(source).map_err(|_| invalid())?;
        let mut properties = [false; 7];
        for name in &wire.properties {
            let index = [
                "opacity",
                "transform",
                "bg",
                "fg",
                "border",
                "shadow",
                "layout",
            ]
            .iter()
            .position(|known| known == name)
            .ok_or_else(invalid)?;
            if std::mem::replace(&mut properties[index], true) {
                return Err(invalid());
            }
        }
        let limit = crate::protocol::limits::TRANSITION_MS as f64;
        let time = |ms: f64, minimum: f64| {
            (ms.is_finite() && ms >= minimum && ms <= limit)
                .then(|| Duration::from_secs_f64(ms / 1000.0))
                .ok_or_else(invalid)
        };
        let duration = time(wire.duration_ms, f64::MIN_POSITIVE)?;
        if wire.properties.is_empty() || duration.is_zero() {
            return Err(invalid());
        }
        Ok(Self {
            properties,
            duration,
            delay: time(wire.delay_ms, 0.0)?,
            timing: match wire.easing {
                Ease::Spring { spring } if spring.is_finite() && (0.0..=0.9).contains(&spring) => {
                    Timing::Spring(spring)
                }
                Ease::Spring { .. } => return Err(invalid()),
                ease => Timing::Ease(super::wire::easing(ease).map_err(|_| invalid())?),
            },
        })
    }

    pub fn covers(&self, property: Property) -> bool {
        self.properties[property as usize]
    }

    /// The spring a released drag takes: this duration, and the bounce of an `ease-spring*`.
    pub fn release(&self) -> (Duration, f64) {
        match self.timing {
            Timing::Spring(bounce) => (self.duration, bounce),
            Timing::Ease(_) => (self.duration, 0.0),
        }
    }
}

/// A transform as independent translate/rotate/scale about a box-relative origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decomposed {
    /// Logical pixels, resolved against the box it was laid out with.
    pub translate: [f64; 2],
    /// Degrees.
    pub rotate: f64,
    pub scale: [f64; 2],
    /// Fractions of the border box.
    pub origin: [f64; 2],
}

impl Default for Decomposed {
    fn default() -> Self {
        Self {
            translate: [0.0; 2],
            rotate: 0.0,
            scale: [1.0; 2],
            origin: [0.5; 2],
        }
    }
}

impl Decomposed {
    fn mix(&self, to: &Self, t: f64) -> Self {
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        Self {
            translate: [
                lerp(self.translate[0], to.translate[0]),
                lerp(self.translate[1], to.translate[1]),
            ],
            rotate: lerp(self.rotate, to.rotate),
            scale: [
                lerp(self.scale[0], to.scale[0]),
                lerp(self.scale[1], to.scale[1]),
            ],
            origin: to.origin,
        }
    }

    /// The paint affine for a box whose layout already moved it by `laid_out`.
    pub fn matrix(&self, [x, y, w, h]: [f64; 4], laid_out: [f64; 2]) -> Affine {
        let origin = (x + self.origin[0] * w, y + self.origin[1] * h);
        let shift = (
            self.translate[0] - laid_out[0],
            self.translate[1] - laid_out[1],
        );
        Affine::translate((origin.0 + shift.0, origin.1 + shift.1))
            * Affine::rotate(self.rotate.to_radians())
            * Affine::scale_non_uniform(self.scale[0], self.scale[1])
            * Affine::translate((-origin.0, -origin.1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLayer {
    pub offset: [f64; 2],
    pub blur: f64,
    pub spread: f64,
    pub color: [u8; 4],
}

impl ShadowLayer {
    const NONE: Self = Self {
        offset: [0.0; 2],
        blur: 0.0,
        spread: 0.0,
        color: [0; 4],
    };
}

/// One node's paint values after state variants, as the scene resolved them.
/// `None` marks a value transitions cannot interpolate (a gradient, a raster shadow).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub opacity: f64,
    /// `None` when the box cannot take a transform.
    pub transform: Option<Decomposed>,
    /// The subtree holds an editor, scroll or slider: only translation animates.
    pub translate_only: bool,
    pub background: Option<[u8; 4]>,
    pub foreground: [u8; 4],
    pub border: Option<[u8; 4]>,
    pub shadows: Option<Vec<ShadowLayer>>,
    /// Where layout put the box, before its own transform; `None` when it cannot take one.
    pub position: Option<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
enum Value {
    Number(f64),
    Transform(Decomposed),
    Color([u8; 4]),
    Shadows(Vec<ShadowLayer>),
    Offset([f64; 2]),
}

impl Resolved {
    fn value(&self, property: Property) -> Option<Value> {
        match property {
            Property::Opacity => Some(Value::Number(self.opacity)),
            Property::Transform => self.transform.map(Value::Transform),
            Property::Background => self.background.map(Value::Color),
            Property::Foreground => Some(Value::Color(self.foreground)),
            Property::Border => self.border.map(Value::Color),
            Property::Shadow => self.shadows.clone().map(Value::Shadows),
            Property::Layout => None,
        }
    }
}

fn mix(from: &Value, to: &Value, t: f64) -> Value {
    match (from, to) {
        (Value::Number(a), Value::Number(b)) => Value::Number(a + (b - a) * t),
        (Value::Transform(a), Value::Transform(b)) => Value::Transform(a.mix(b, t)),
        (Value::Color(a), Value::Color(b)) => Value::Color(mix_oklab(*a, *b, t)),
        (Value::Offset(a), Value::Offset(b)) => {
            Value::Offset([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
        }
        (Value::Shadows(a), Value::Shadows(b)) => Value::Shadows(
            (0..a.len().max(b.len()))
                .map(|i| {
                    let (a, b) = (
                        a.get(i).unwrap_or(&ShadowLayer::NONE),
                        b.get(i).unwrap_or(&ShadowLayer::NONE),
                    );
                    let lerp = |x: f64, y: f64| x + (y - x) * t;
                    ShadowLayer {
                        offset: [
                            lerp(a.offset[0], b.offset[0]),
                            lerp(a.offset[1], b.offset[1]),
                        ],
                        blur: lerp(a.blur, b.blur).max(0.0),
                        spread: lerp(a.spread, b.spread),
                        color: mix_oklab(a.color, b.color, t),
                    }
                })
                .collect(),
        ),
        _ => to.clone(),
    }
}

fn linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn encode(linear: f64) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let c = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

fn oklab([r, g, b, _]: [u8; 4]) -> [f64; 3] {
    let [r, g, b] = [linear(r), linear(g), linear(b)];
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ]
}

fn srgb([lightness, a, b]: [f64; 3]) -> [u8; 3] {
    let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lightness - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
    .map(encode)
}

/// Premultiplied OKLab, so a fade from transparent keeps the target hue.
pub fn mix_oklab(from: [u8; 4], to: [u8; 4], t: f64) -> [u8; 4] {
    let (fa, ta) = (f64::from(from[3]) / 255.0, f64::from(to[3]) / 255.0);
    let alpha = fa + (ta - fa) * t;
    if alpha <= 0.0 {
        return [0; 4];
    }
    let (f, g) = (oklab(from), oklab(to));
    let lab = [0, 1, 2].map(|i| (f[i] * fa + (g[i] * ta - f[i] * fa) * t) / alpha);
    let [r, g, b] = srgb(lab);
    [r, g, b, (alpha.min(1.0) * 255.0).round() as u8]
}

#[derive(Clone, Debug, PartialEq)]
struct Instance {
    from: Value,
    to: Value,
    /// How long it runs: the curve's duration, or until the spring settles.
    duration: Duration,
    delay: Duration,
    curve: Curve,
    /// Set by the first presentation of the frame that started it.
    started: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq)]
enum Curve {
    Ease(Easing),
    /// Each numeric channel starts with its own velocity, in units per second.
    Spring {
        spring: Spring,
        velocity: Vec<f64>,
    },
}

/// The numeric channels a spring moves one by one; colours and shadows move by progress.
fn channels(value: &Value) -> Option<Vec<f64>> {
    match value {
        Value::Number(v) => Some(vec![*v]),
        Value::Transform(t) => Some(vec![
            t.translate[0],
            t.translate[1],
            t.rotate,
            t.scale[0],
            t.scale[1],
        ]),
        Value::Offset(o) => Some(o.to_vec()),
        _ => None,
    }
}

fn with_channels(to: &Value, c: &[f64]) -> Value {
    match to {
        Value::Number(_) => Value::Number(c[0]),
        Value::Transform(t) => Value::Transform(Decomposed {
            translate: [c[0], c[1]],
            rotate: c[2],
            scale: [c[3], c[4]],
            origin: t.origin,
        }),
        Value::Offset(_) => Value::Offset([c[0], c[1]]),
        other => other.clone(),
    }
}

impl Instance {
    /// A transition from `from` to `to` on the definition's timing; a spring keeps `velocity`.
    fn new(
        definition: &TransitionDef,
        policy: Policy,
        from: Value,
        to: Value,
        velocity: Option<Vec<f64>>,
    ) -> Result<Self, String> {
        let duration = definition.duration.mul_f64(policy.scale);
        let delay = definition.delay.mul_f64(policy.scale);
        let bounce = match definition.timing {
            Timing::Spring(bounce) => bounce,
            Timing::Ease(easing) => {
                return Ok(Self {
                    from,
                    to,
                    duration,
                    delay,
                    curve: Curve::Ease(easing),
                    started: None,
                });
            }
        };
        let spring = Spring::new(duration, bounce)?;
        let limit = crate::protocol::limits::TRANSITION_MS as f64 / 1000.0;
        let (start, end) = (channels(&from), channels(&to));
        let velocity = match (&start, &end, velocity) {
            (Some(start), Some(_), Some(v)) if v.len() == start.len() => v,
            (Some(start), Some(_), _) => vec![0.0; start.len()],
            _ => Vec::new(),
        };
        let settle = match (&start, &end) {
            (Some(start), Some(end)) => start
                .iter()
                .zip(end)
                .zip(&velocity)
                .map(|((a, b), v)| spring.settle(a - b, *v, limit))
                .fold(0.0, f64::max),
            _ => spring.settle(1.0, 0.0, limit),
        };
        Ok(Self {
            from,
            to,
            duration: Duration::from_secs_f64(settle.max(0.001)),
            delay,
            curve: Curve::Spring { spring, velocity },
            started: None,
        })
    }

    fn elapsed(&self, now: Duration) -> Duration {
        self.started
            .map(|start| now.saturating_sub(start))
            .unwrap_or_default()
    }

    fn finished(&self, now: Duration) -> bool {
        self.elapsed(now) >= self.delay + self.duration
    }

    fn sample(&self, now: Duration) -> Result<Value, String> {
        let elapsed = self.elapsed(now);
        if elapsed < self.delay {
            return Ok(self.from.clone());
        }
        if elapsed >= self.delay + self.duration {
            return Ok(self.to.clone());
        }
        let t = (elapsed - self.delay).as_secs_f64();
        match &self.curve {
            Curve::Ease(easing) => {
                let progress = (t / self.duration.as_secs_f64()).min(1.0);
                Ok(mix(&self.from, &self.to, easing.sample(progress)?))
            }
            Curve::Spring { spring, velocity } => {
                match (channels(&self.from), channels(&self.to)) {
                    (Some(from), Some(to)) => {
                        let moved: Vec<f64> = (0..from.len())
                            .map(|i| spring.at(from[i], to[i], velocity[i], t).0)
                            .collect();
                        Ok(match with_channels(&self.to, &moved) {
                            // Opacity has no overshoot to show.
                            Value::Number(v) => {
                                Value::Number(v.clamp(from[0].min(to[0]), from[0].max(to[0])))
                            }
                            value => value,
                        })
                    }
                    _ => {
                        let progress = 1.0 - spring.basis(t)[0];
                        Ok(mix(&self.from, &self.to, progress.clamp(0.0, 1.0)))
                    }
                }
            }
        }
    }

    /// Each channel's velocity now, for a retarget to carry on; `None` for curves.
    fn velocity(&self, now: Duration) -> Option<Vec<f64>> {
        let Curve::Spring { spring, velocity } = &self.curve else {
            return None;
        };
        let elapsed = self.elapsed(now);
        if elapsed >= self.delay + self.duration {
            return None;
        }
        let t = elapsed.saturating_sub(self.delay).as_secs_f64();
        let (from, to) = (channels(&self.from)?, channels(&self.to)?);
        Some(
            (0..from.len())
                .map(|i| spring.at(from[i], to[i], velocity[i], t).1)
                .collect(),
        )
    }

    fn wakeup(&self, now: Duration) -> super::Wakeup {
        let elapsed = self.elapsed(now);
        if self.started.is_none() {
            super::Wakeup::NextFrame
        } else if elapsed < self.delay {
            super::Wakeup::After(self.delay - elapsed)
        } else if self.curve == Curve::Ease(Easing::STEP_END) {
            super::Wakeup::After((self.delay + self.duration).saturating_sub(elapsed))
        } else {
            super::Wakeup::NextFrame
        }
    }
}

/// Where a node was laid out in a committed frame: an exit ghost repaints it from there.
#[derive(Clone)]
pub(crate) struct Origin(
    pub(crate) std::sync::Arc<crate::scene::retained::RetainedLayout>,
    pub(crate) usize,
);

impl std::fmt::Debug for Origin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Origin({})", self.1)
    }
}

impl PartialEq for Origin {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0) && self.1 == other.1
    }
}

/// One realized node with a transition, its resolved paint and its presence values.
#[derive(Clone, Debug)]
pub(crate) struct Input {
    pub identity: crate::scene::node::Identity,
    pub definition: TransitionDef,
    pub target: Resolved,
    /// `starting:` values: a node seen for the first time enters from them.
    pub starting: Option<Resolved>,
    /// `exit:` values: a removed node leaves toward them as a ghost.
    pub exit: Option<Resolved>,
    pub origin: Option<Origin>,
}

impl From<(String, TransitionDef, Resolved)> for Input {
    fn from((identity, definition, target): (String, TransitionDef, Resolved)) -> Self {
        Self {
            identity: identity.into(),
            definition,
            target,
            starting: None,
            exit: None,
            origin: None,
        }
    }
}

impl Node {
    /// The FLIP motion of a node that layout moved: from where it was on screen back to zero.
    fn layout_motion(
        &self,
        target: &Resolved,
        definition: &TransitionDef,
        now: Duration,
        policy: Policy,
    ) -> Result<Option<Instance>, String> {
        let (Some(before), Some(after)) = (self.target.position, target.position) else {
            return Ok(None);
        };
        if !definition.covers(Property::Layout) {
            return Ok(None);
        }
        let running = self
            .instances
            .get(&Property::Layout)
            .filter(|instance| policy.keep && !instance.finished(now));
        if before == after {
            return Ok(running.cloned());
        }
        if !policy.start {
            return Ok(None);
        }
        let shown = match running.map(|instance| instance.sample(now)).transpose()? {
            Some(Value::Offset(offset)) => offset,
            _ => [0.0; 2],
        };
        let from = [
            before[0] + shown[0] - after[0],
            before[1] + shown[1] - after[1],
        ];
        // Sub-pixel reflows (rounding, a scrollbar) are not worth a motion.
        if from[0].abs() < 0.5 && from[1].abs() < 0.5 {
            return Ok(None);
        }
        let velocity = running.and_then(|instance| instance.velocity(now));
        Ok(Some(Instance::new(
            definition,
            policy,
            Value::Offset(from),
            Value::Offset([0.0; 2]),
            velocity,
        )?))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Node {
    target: Resolved,
    instances: BTreeMap<Property, Instance>,
    definition: TransitionDef,
    exit: Option<Resolved>,
    origin: Option<Origin>,
    /// The node left the View; its last paint is kept until the exit finishes.
    ghost: bool,
}

/// The paint values a transition currently overrides; absent fields use the base style.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    pub opacity: Option<f64>,
    pub transform: Option<Decomposed>,
    pub background: Option<[u8; 4]>,
    pub foreground: Option<[u8; 4]>,
    pub border: Option<[u8; 4]>,
    pub shadows: Option<Vec<ShadowLayer>>,
}

impl Overrides {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

/// How a frame may treat new and running transitions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Policy {
    /// Visible and motion allowed: changes start instances; otherwise they jump.
    pub start: bool,
    /// Reduced motion completes running instances at once.
    pub keep: bool,
    /// Animator duration scale (> 0).
    pub scale: f64,
}

/// Per-identity transition state of one page; identities absent from a frame are dropped.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Transitions(BTreeMap<crate::scene::node::Identity, Node>);

impl Transitions {
    /// `now` is the timeline's active time. Inputs are the frame's nodes with a transition.
    pub(crate) fn reconcile<I: Into<Input>>(
        &self,
        inputs: impl IntoIterator<Item = I>,
        now: Duration,
        policy: Policy,
    ) -> Result<Self, String> {
        let mut next = BTreeMap::new();
        let mut moving = 0;
        for input in inputs {
            let Input {
                identity,
                definition,
                target,
                starting,
                exit,
                origin,
            } = input.into();
            if next.len() == super::MAX_INSTANCES {
                return Err("UI_TRANSITION: instance budget exceeded".into());
            }
            let mut instances = BTreeMap::new();
            let old = self.0.get(&*identity);
            let entering = old.is_none_or(|old| old.ghost);
            if entering {
                // Enters from its `starting:` values, or from where its exit ghost had got to.
                if let Some(from) = starting.as_ref().filter(|_| policy.start) {
                    for property in PROPERTIES {
                        if !definition.covers(property) {
                            continue;
                        }
                        let ghost = old
                            .and_then(|old| old.instances.get(&property))
                            .filter(|instance| policy.keep && !instance.finished(now));
                        let begin = match ghost {
                            Some(instance) => Some(instance.sample(now)?),
                            None => from.value(property),
                        };
                        let (Some(begin), Some(end)) = (begin, target.value(property)) else {
                            continue;
                        };
                        if begin != end {
                            let velocity = ghost.and_then(|instance| instance.velocity(now));
                            instances.insert(
                                property,
                                Instance::new(&definition, policy, begin, end, velocity)?,
                            );
                        }
                    }
                }
            } else if let Some(old) = old {
                for property in PROPERTIES {
                    if !definition.covers(property) {
                        continue;
                    }
                    let running = old
                        .instances
                        .get(&property)
                        .filter(|instance| policy.keep && !instance.finished(now));
                    let (Some(before), Some(after)) =
                        (old.target.value(property), target.value(property))
                    else {
                        continue;
                    };
                    if before == after {
                        if let Some(instance) = running {
                            instances.insert(property, instance.clone());
                        }
                        continue;
                    }
                    if !policy.start {
                        continue;
                    }
                    // A retarget starts from the value on screen, a spring also with its velocity.
                    let (from, velocity) = match running {
                        Some(instance) => (instance.sample(now)?, instance.velocity(now)),
                        None => (before, None),
                    };
                    instances.insert(
                        property,
                        Instance::new(&definition, policy, from, after, velocity)?,
                    );
                }
                if let Some(instance) = old
                    .layout_motion(&target, &definition, now, policy)?
                    .filter(|_| moving < crate::protocol::limits::LAYOUT_MOTIONS)
                {
                    instances.insert(Property::Layout, instance);
                    moving += 1;
                }
            }
            next.insert(
                identity,
                Node {
                    target,
                    instances,
                    definition,
                    exit,
                    origin,
                    ghost: false,
                },
            );
        }
        // Removed nodes: a running ghost carries on; one with `exit:` values starts one.
        let mut ghosts = next.values().filter(|node| node.ghost).count();
        for (identity, old) in &self.0 {
            if next.contains_key(&**identity) || !policy.start {
                continue;
            }
            if old.ghost {
                if old.instances.values().any(|i| !i.finished(now)) {
                    next.insert(identity.clone(), old.clone());
                    ghosts += 1;
                }
                continue;
            }
            let (Some(exit), Some(origin)) = (&old.exit, &old.origin) else {
                continue;
            };
            if ghosts == crate::protocol::limits::GHOSTS || self.removed_within(identity, &next) {
                continue;
            }
            let mut instances = BTreeMap::new();
            for property in PROPERTIES {
                if !old.definition.covers(property) {
                    continue;
                }
                let running = old
                    .instances
                    .get(&property)
                    .filter(|instance| policy.keep && !instance.finished(now));
                let begin = match running {
                    Some(instance) => Some(instance.sample(now)?),
                    None => old.target.value(property),
                };
                let (Some(begin), Some(end)) = (begin, exit.value(property)) else {
                    continue;
                };
                if begin != end {
                    let velocity = running.and_then(|instance| instance.velocity(now));
                    instances.insert(
                        property,
                        Instance::new(&old.definition, policy, begin, end, velocity)?,
                    );
                }
            }
            if !instances.is_empty() {
                next.insert(
                    identity.to_string().into(),
                    Node {
                        target: exit.clone(),
                        instances,
                        definition: old.definition,
                        exit: None,
                        origin: Some(origin.clone()),
                        ghost: true,
                    },
                );
                ghosts += 1;
            }
        }
        Ok(Self(next))
    }

    /// Only the outermost removed node ghosts: one inside an exiting ancestor is part of it.
    fn removed_within(
        &self,
        identity: &str,
        next: &BTreeMap<crate::scene::node::Identity, Node>,
    ) -> bool {
        self.0.iter().any(|(other, node)| {
            other.len() < identity.len()
                && identity.starts_with(&**other)
                && identity.as_bytes()[other.len()] == b'/'
                && (node.ghost || (node.exit.is_some() && !next.contains_key(other)))
        })
    }

    /// The exit ghosts still drawing, with the frame each was laid out in.
    pub(crate) fn ghosts(&self, now: Duration) -> Vec<(crate::scene::node::Identity, Origin)> {
        self.0
            .iter()
            .filter(|(_, node)| node.ghost && node.instances.values().any(|i| !i.finished(now)))
            .filter_map(|(identity, node)| Some((identity.clone(), node.origin.clone()?)))
            .collect()
    }

    /// Nodes still moving to a new layout position.
    pub(crate) fn layout_motions(&self, now: Duration) -> usize {
        self.0
            .values()
            .filter(|node| {
                node.instances
                    .get(&Property::Layout)
                    .is_some_and(|instance| !instance.finished(now))
            })
            .count()
    }

    pub(crate) fn start(&mut self, now: Duration) {
        for node in self.0.values_mut() {
            for instance in node.instances.values_mut() {
                instance.started.get_or_insert(now);
            }
        }
    }

    pub(crate) fn identities(&self) -> impl Iterator<Item = &crate::scene::node::Identity> {
        self.0.keys()
    }

    pub(crate) fn contains(&self, identity: &str) -> bool {
        self.0.contains_key(identity)
    }

    pub(crate) fn overrides(&self, identity: &str, now: Duration) -> Result<Overrides, String> {
        let mut overrides = Overrides::default();
        let Some(node) = self.0.get(identity) else {
            return Ok(overrides);
        };
        let mut offset = None;
        for (property, instance) in &node.instances {
            if instance.finished(now) {
                continue;
            }
            match (property, instance.sample(now)?) {
                (Property::Opacity, Value::Number(v)) => {
                    overrides.opacity = Some(v.clamp(0.0, 1.0))
                }
                (Property::Transform, Value::Transform(v)) => overrides.transform = Some(v),
                (Property::Background, Value::Color(v)) => overrides.background = Some(v),
                (Property::Foreground, Value::Color(v)) => overrides.foreground = Some(v),
                (Property::Border, Value::Color(v)) => overrides.border = Some(v),
                (Property::Shadow, Value::Shadows(v)) => overrides.shadows = Some(v),
                (Property::Layout, Value::Offset(v)) => offset = Some(v),
                _ => return Err("UI_TRANSITION: mismatched value".into()),
            }
        }
        // The FLIP offset rides on top of the node's own (possibly transitioning) transform.
        if let (Some([dx, dy]), Some(mut transform)) =
            (offset, overrides.transform.or(node.target.transform))
        {
            transform.translate[0] += dx;
            transform.translate[1] += dy;
            overrides.transform = Some(transform);
        }
        Ok(overrides)
    }

    pub(crate) fn wakeup(&self, identity: &str, now: Duration) -> super::Wakeup {
        let mut next = super::Wakeup::None;
        if let Some(node) = self.0.get(identity) {
            for instance in node.instances.values() {
                if !instance.finished(now) {
                    next = next.earliest(instance.wakeup(now));
                }
            }
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn oklab_mixes_black_and_white_through_perceptual_middle_gray() {
        assert_eq!(
            mix_oklab([0, 0, 0, 255], [255, 255, 255, 255], 0.5),
            [99, 99, 99, 255]
        );
        assert_eq!(
            mix_oklab([12, 34, 56, 255], [200, 10, 90, 255], 0.0),
            [12, 34, 56, 255]
        );
        assert_eq!(
            mix_oklab([12, 34, 56, 255], [200, 10, 90, 255], 1.0),
            [200, 10, 90, 255]
        );
        // From transparent, only alpha moves: the target hue is kept throughout.
        let half = mix_oklab([0, 0, 0, 0], [255, 0, 0, 255], 0.5);
        assert_eq!((half[0], half[3]), (255, 128));
    }

    #[test]
    fn definitions_are_bounded_and_exact() {
        let valid = r#"{"properties":["opacity","bg"],"durationMs":200,"delayMs":50,"easing":[0.4,0,0.2,1]}"#;
        let def = TransitionDef::decode(valid).unwrap();
        assert!(def.covers(Property::Opacity) && def.covers(Property::Background));
        assert!(!def.covers(Property::Transform));
        assert_eq!((def.duration, def.delay), (ms(200), ms(50)));
        for broken in [
            r#"{"properties":["height"],"durationMs":200,"delayMs":0,"easing":"linear"}"#,
            r#"{"properties":["bg","bg"],"durationMs":200,"delayMs":0,"easing":"linear"}"#,
            r#"{"properties":[],"durationMs":200,"delayMs":0,"easing":"linear"}"#,
            r#"{"properties":["bg"],"durationMs":0,"delayMs":0,"easing":"linear"}"#,
            r#"{"properties":["bg"],"durationMs":10001,"delayMs":0,"easing":"linear"}"#,
            r#"{"properties":["bg"],"durationMs":200,"delayMs":0,"easing":"bounce"}"#,
            r#"{"properties":["bg"],"durationMs":200,"delayMs":0,"easing":"linear","x":1}"#,
        ] {
            assert!(TransitionDef::decode(broken).is_err(), "{broken}");
        }
    }

    fn style(opacity: f64) -> Resolved {
        Resolved {
            opacity,
            transform: Some(Decomposed::default()),
            translate_only: false,
            background: Some([0; 4]),
            foreground: [0, 0, 0, 255],
            border: None,
            shadows: Some(Vec::new()),
            position: Some([0.0; 2]),
        }
    }

    const MOVE: Policy = Policy {
        start: true,
        keep: true,
        scale: 1.0,
    };

    #[test]
    fn interruption_continues_from_the_sampled_value_and_removal_cancels() {
        let def = TransitionDef::decode(
            r#"{"properties":["opacity"],"durationMs":200,"delayMs":0,"easing":"linear"}"#,
        )
        .unwrap();
        let input = |o: f64| [("a".to_owned(), def, style(o))];
        let first = Transitions::default()
            .reconcile(input(1.0), ms(0), MOVE)
            .unwrap();
        assert!(first.overrides("a", ms(0)).unwrap().is_empty());
        let mut fading = first.reconcile(input(0.0), ms(0), MOVE).unwrap();
        fading.start(ms(0));
        assert_eq!(fading.overrides("a", ms(50)).unwrap().opacity, Some(0.75));
        let mut back = fading.reconcile(input(1.0), ms(100), MOVE).unwrap();
        assert_eq!(back.overrides("a", ms(100)).unwrap().opacity, Some(0.5));
        back.start(ms(100));
        assert_eq!(back.overrides("a", ms(200)).unwrap().opacity, Some(0.75));
        assert!(back.overrides("a", ms(300)).unwrap().is_empty());
        assert_eq!(back.wakeup("a", ms(300)), super::super::Wakeup::None);
        let removed = back.reconcile(Vec::<Input>::new(), ms(150), MOVE).unwrap();
        assert!(!removed.contains("a"));
        let hidden = fading
            .reconcile(
                input(1.0),
                ms(100),
                Policy {
                    start: false,
                    ..MOVE
                },
            )
            .unwrap();
        assert!(hidden.overrides("a", ms(100)).unwrap().is_empty());
        let reduced = fading
            .reconcile(
                input(0.0),
                ms(100),
                Policy {
                    start: false,
                    keep: false,
                    scale: 1.0,
                },
            )
            .unwrap();
        assert!(reduced.overrides("a", ms(100)).unwrap().is_empty());
        let mut slow = first
            .reconcile(input(0.0), ms(0), Policy { scale: 2.0, ..MOVE })
            .unwrap();
        slow.start(ms(0));
        assert_eq!(slow.overrides("a", ms(200)).unwrap().opacity, Some(0.5));
    }

    #[test]
    fn a_spring_retarget_keeps_position_and_velocity() {
        let def = TransitionDef::decode(
            r#"{"properties":["transform"],"durationMs":300,"delayMs":0,"easing":{"spring":0.2}}"#,
        )
        .unwrap();
        let moved = |y: f64| {
            let mut style = style(1.0);
            style.transform = Some(Decomposed {
                translate: [0.0, y],
                ..Decomposed::default()
            });
            [("a".to_owned(), def, style)]
        };
        let y = |t: &Transitions, at| {
            t.overrides("a", ms(at))
                .unwrap()
                .transform
                .map(|d| d.translate[1])
        };
        let first = Transitions::default()
            .reconcile(moved(0.0), ms(0), MOVE)
            .unwrap();
        let mut down = first.reconcile(moved(100.0), ms(0), MOVE).unwrap();
        down.start(ms(0));
        // Retarget back up mid-flight: the value continues, and so does the downward velocity.
        let (before, just_before) = (y(&down, 80).unwrap(), y(&down, 79).unwrap());
        let mut up = down.reconcile(moved(0.0), ms(80), MOVE).unwrap();
        up.start(ms(80));
        let (at, just_after) = (y(&up, 80).unwrap(), y(&up, 81).unwrap());
        assert!((at - before).abs() < 1e-9, "{at} {before}");
        assert!(
            just_after > at && before > just_before,
            "still moving down: {just_after} {at}"
        );
        // It settles on the new target and reports no wakeup after.
        let settled = (80..3000)
            .step_by(16)
            .find(|t| y(&up, *t).is_none())
            .unwrap();
        assert!(settled < 2000, "{settled}");
        assert!(
            TransitionDef::decode(
                r#"{"properties":["opacity"],"durationMs":300,"delayMs":0,"easing":{"spring":1.5}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn decomposed_transform_replaces_the_laid_out_translation() {
        let to = Decomposed {
            translate: [0.0, 4.0],
            ..Decomposed::default()
        };
        // At the target the override is the identity over geometry already moved by 4px.
        assert_eq!(
            to.matrix([0.0, 4.0, 10.0, 10.0], [0.0, 4.0]),
            Affine::IDENTITY
        );
        let from = Decomposed {
            scale: [0.5; 2],
            ..Decomposed::default()
        };
        let half = from.mix(&to, 0.5);
        let p = half.matrix([0.0, 4.0, 10.0, 10.0], [0.0, 4.0])
            * vello_cpu::kurbo::Point::new(5.0, 9.0);
        // The center sits 2px down from its unmoved place, i.e. 2px above the laid-out center.
        assert!(
            (p.x - 5.0).abs() < 1e-9 && (p.y - 7.0).abs() < 1e-9,
            "{p:?}"
        );
    }
}
