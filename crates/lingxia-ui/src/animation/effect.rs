use super::interpolate::{NextChange, Track, Transform, compose};
use vello_cpu::kurbo::Affine;

#[derive(Clone, Debug, PartialEq)]
pub enum TransformTracks {
    Scale { x: Track, y: Track },
    Translate { x: Track, y: Track },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    opacity: Option<Track>,
    transforms: Vec<TransformTracks>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Visual {
    pub opacity: Option<f64>,
    pub transforms: Vec<Transform>,
}

impl Visual {
    pub fn matrix(&self, origin: [f64; 2]) -> Result<Affine, String> {
        compose(origin, &self.transforms)
    }
}

impl Effect {
    pub fn new(opacity: Option<Track>, transforms: Vec<TransformTracks>) -> Result<Self, String> {
        if transforms.len() > 8
            || opacity
                .as_ref()
                .is_some_and(|track| !track.values_within(0.0, 1.0))
        {
            return Err("UI_ANIMATION: invalid effect tracks".into());
        }
        for transform in &transforms {
            let (x, y, bound) = match transform {
                TransformTracks::Scale { x, y } => (x, y, 16.0),
                TransformTracks::Translate { x, y } => (x, y, 16384.0),
            };
            if !x.values_within(-bound, bound) || !y.values_within(-bound, bound) {
                return Err("UI_ANIMATION: transform track outside bounds".into());
            }
        }
        Ok(Self {
            opacity,
            transforms,
        })
    }

    fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.opacity.iter().chain(
            self.transforms
                .iter()
                .flat_map(|transform| match transform {
                    TransformTracks::Scale { x, y } | TransformTracks::Translate { x, y } => [x, y],
                }),
        )
    }

    pub(crate) fn storage_bytes(&self) -> usize {
        64 + self.tracks().map(Track::storage_bytes).sum::<usize>()
    }
    pub(crate) fn is_constant(&self) -> bool {
        self.tracks().all(Track::is_constant)
    }

    pub fn sample(&self, progress: f64) -> Result<Visual, String> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err("UI_ANIMATION: invalid effect progress".into());
        }
        let mut transforms = Vec::with_capacity(self.transforms.len());
        for transform in &self.transforms {
            transforms.push(match transform {
                TransformTracks::Scale { x, y } => {
                    Transform::Scale([x.sample(progress)?, y.sample(progress)?])
                }
                TransformTracks::Translate { x, y } => {
                    Transform::Translate([x.sample(progress)?, y.sample(progress)?])
                }
            });
        }
        compose([0.0; 2], &transforms)?;
        Ok(Visual {
            opacity: self
                .opacity
                .as_ref()
                .map(|track| track.sample(progress).map(|value| value.clamp(0.0, 1.0)))
                .transpose()?,
            transforms,
        })
    }

    pub fn next_change(&self, progress: f64) -> Result<NextChange, String> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err("UI_ANIMATION: invalid effect progress".into());
        }
        let mut next = NextChange::None;
        for track in self.tracks() {
            match track.next_change(progress)? {
                NextChange::Continuous => return Ok(NextChange::Continuous),
                NextChange::At(at) => {
                    next = match next {
                        NextChange::At(previous) => NextChange::At(at.min(previous)),
                        _ => NextChange::At(at),
                    }
                }
                NextChange::None => {}
            }
        }
        Ok(next)
    }
}
