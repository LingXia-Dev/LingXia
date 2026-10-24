//! Shared ordered composition of isolated groups and backdrop captures.
use crate::{color_filter::Saturation, render::ClipSnapshot};
use std::ops::Range;

#[derive(Clone, Debug)]
pub enum Layer {
    /// An overflow clip; unlike an opacity group, it is not a backdrop root.
    Clip(ClipSnapshot),
    Opacity(f32),
    /// Filter lengths and clips are already resolved into device coordinates.
    Backdrop {
        clip: ClipSnapshot,
        blur: f32,
        saturation: Saturation,
        opacity: f32,
    },
}

impl Layer {
    fn is_root(&self) -> bool {
        matches!(self, Self::Backdrop { .. })
            || matches!(self, Self::Opacity(alpha) if *alpha < 1.0)
    }

    fn validate(&self) -> Result<(), String> {
        let (clip, alpha) = match self {
            Self::Clip(clip) => (Some(clip), 1.0),
            Self::Opacity(alpha) => (None, *alpha),
            Self::Backdrop {
                clip,
                blur,
                opacity,
                ..
            } => {
                if !blur.is_finite() || !(0.0..=256.0).contains(blur) {
                    return Err("UI_COMPOSITE: blur outside 0..256 device pixels".into());
                }
                (Some(clip), *opacity)
            }
        };
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err("UI_COMPOSITE: invalid opacity".into());
        }
        if let Some(clip) = clip {
            let valid = clip.bounds.iter().all(|n| n.is_finite() && n.abs() <= 1e12)
                && clip.bounds[2..].iter().all(|n| *n >= 0.0)
                && clip.radius.is_finite()
                && (0.0..=1e12).contains(&clip.radius)
                && clip.radii.is_none_or(|radii| {
                    radii
                        .iter()
                        .flatten()
                        .all(|n| n.is_finite() && (0.0..=1e12).contains(n))
                });
            if !valid {
                return Err("UI_COMPOSITE: invalid clip".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum Step {
    Draw(Range<usize>),
    Push(Layer),
    Pop,
}

/// Preflighted program. Draw spans refer to the caller's immutable display list.
pub struct Plan {
    steps: Vec<Step>,
    width: u16,
    height: u16,
}

impl Plan {
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
    pub fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }
    pub fn new(
        width: u16,
        height: u16,
        draw_count: usize,
        steps: Vec<Step>,
    ) -> Result<Self, String> {
        let pixels = u64::from(width) * u64::from(height);
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || pixels > 4 * 1024 * 1024
            || steps.len() > 4096
        {
            return Err("UI_COMPOSITE: frame or program limit exceeded".into());
        }
        let mut depth = 1u64;
        let mut peak = depth;
        let mut layers: Vec<&Layer> = Vec::new();
        let frame_bytes = pixels * 4;
        let mut working_bytes = frame_bytes;
        let mut drawn = 0;
        for step in &steps {
            match step {
                Step::Draw(range)
                    if range.start < drawn || range.start > range.end || range.end > draw_count =>
                {
                    return Err("UI_COMPOSITE: invalid draw span".into());
                }
                Step::Draw(range) => {
                    drawn = range.end;
                    if !range.is_empty() {
                        working_bytes = working_bytes.max((depth + 1) * frame_bytes);
                    }
                }
                Step::Push(layer) => {
                    layer.validate()?;
                    match layer {
                        Layer::Backdrop { clip, blur, .. } => {
                            let region =
                                crate::backdrop_region::Region::new(*clip, width, height, *blur)?;
                            let region_bytes = region.map_or(0, |region| {
                                u64::from(region.width) * u64::from(region.height) * 4
                            });
                            let merges = layers
                                .iter()
                                .rev()
                                .take_while(|layer| !layer.is_root())
                                .count() as u64;
                            working_bytes =
                                working_bytes.max((depth + 1) * frame_bytes + region_bytes);
                            if merges > 0 {
                                working_bytes = working_bytes.max((depth + 2) * frame_bytes);
                            }
                        }
                        _ => working_bytes = working_bytes.max((depth + 1) * frame_bytes),
                    }
                    layers.push(layer);
                    depth += 1;
                    peak = peak.max(depth);
                }
                Step::Pop if depth == 1 => return Err("UI_COMPOSITE: unmatched pop".into()),
                Step::Pop => {
                    working_bytes = working_bytes.max((depth + 1) * frame_bytes);
                    layers.pop();
                    depth -= 1;
                }
            }
        }
        if depth != 1 {
            return Err("UI_COMPOSITE: unclosed layer".into());
        }
        // GPU surfaces return to a bounded pool after their last use is submitted;
        // later writes are ordered behind those reads. CPU writes finish before
        // old pixels are released. Filter padding and depth-cache pixels have
        // separate bounds; these figures do not describe total driver memory.
        if peak > 32 || working_bytes > 64 * 1024 * 1024 {
            return Err("UI_COMPOSITE: working surfaces exceed limit".into());
        }
        Ok(Self {
            steps,
            width,
            height,
        })
    }

    pub fn render<B: Backend>(&self, backend: &mut B) -> Result<B::Surface, String> {
        let mut stack = vec![(None, backend.blank(self.width, self.height)?)];
        for step in &self.steps {
            match step {
                Step::Draw(range) => {
                    backend.draw(&mut stack.last_mut().unwrap().1, range.clone())?
                }
                Step::Push(layer) => {
                    let surface = if matches!(layer, Layer::Backdrop { .. }) {
                        let nearest = stack
                            .iter()
                            .rposition(|(layer, _)| layer.as_ref().is_some_and(Layer::is_root))
                            .unwrap_or(0);
                        let mut capture = backend.snapshot(&stack.last().unwrap().1)?;
                        // Merge unfinished overflow clips from inner to outer.
                        // Each is composited once; never replay the display prefix.
                        for index in (nearest..stack.len() - 1).rev() {
                            let mut parent = backend.snapshot(&stack[index].1)?;
                            backend.composite(
                                &mut parent,
                                &capture,
                                stack[index + 1].0.as_ref().unwrap(),
                            )?;
                            capture = parent;
                        }
                        backend.filter(capture, layer)?
                    } else {
                        backend.blank(self.width, self.height)?
                    };
                    stack.push((Some(layer.clone()), surface));
                }
                Step::Pop => {
                    let (layer, surface) = stack.pop().unwrap();
                    backend.composite(
                        &mut stack.last_mut().unwrap().1,
                        &surface,
                        layer.as_ref().unwrap(),
                    )?;
                }
            }
        }
        Ok(stack.pop().unwrap().1)
    }
}

pub trait Backend {
    type Surface;
    fn blank(&mut self, width: u16, height: u16) -> Result<Self::Surface, String>;
    fn draw(&mut self, target: &mut Self::Surface, span: Range<usize>) -> Result<(), String>;
    /// Must remain unchanged by subsequent writes to the original surface.
    fn snapshot(&mut self, source: &Self::Surface) -> Result<Self::Surface, String>;
    fn composite(
        &mut self,
        target: &mut Self::Surface,
        source: &Self::Surface,
        layer: &Layer,
    ) -> Result<(), String>;
    /// Initializes the owner's surface with its filtered, border-box-clipped backdrop.
    fn filter(&mut self, source: Self::Surface, layer: &Layer) -> Result<Self::Surface, String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preflight_rejects_unbalanced_invalid_and_oversized_programs() {
        for steps in [
            vec![Step::Pop],
            vec![Step::Push(Layer::Opacity(0.5))],
            vec![Step::Draw(0..2)],
            vec![Step::Draw(Range { start: 1, end: 0 })],
            vec![Step::Push(Layer::Opacity(f32::NAN)), Step::Pop],
        ] {
            assert!(Plan::new(96, 96, 1, steps).is_err());
        }
        assert!(Plan::new(4096, 4096, 0, vec![]).is_err());
        let mut steps = vec![Step::Push(Layer::Opacity(0.5)); 4];
        steps.extend(vec![Step::Pop; 4]);
        assert!(Plan::new(2048, 2048, 0, steps).is_err());
        let steps: Vec<_> = (0..20).map(|index| Step::Draw(index..index + 1)).collect();
        assert!(
            Plan::new(1024, 1024, 20, steps).is_ok(),
            "sequential spans reuse working storage"
        );
        assert!(
            Plan::new(96, 96, 1, vec![Step::Draw(0..1), Step::Draw(0..1)]).is_err(),
            "draw prefixes cannot be replayed"
        );
    }
}
