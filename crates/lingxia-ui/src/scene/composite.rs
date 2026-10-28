//! Shared ordered composition of isolated groups and backdrop captures.
use crate::{scene::color_filter::Saturation, scene::node::ClipSnapshot};
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
    /// `mask-image` fades, in device coordinates: the group is drawn through them.
    Mask(std::sync::Arc<DeviceMask>),
    /// `blur-*`: the group is blurred by this standard deviation in device pixels.
    Blur(f32),
}

/// A mask's fades and the box they cover, placed in device pixels.
#[derive(Clone, Debug)]
pub struct DeviceMask {
    pub(crate) layers: Vec<crate::scene::gradient::GradientBrush>,
    pub(crate) path: vello_cpu::kurbo::BezPath,
}

impl DeviceMask {
    pub(crate) fn new(mask: &crate::scene::mask::Mask, device: vello_cpu::kurbo::Affine) -> Self {
        use vello_cpu::kurbo::Shape;
        let [x, y, w, h] = mask.bounds.map(f64::from);
        let placed = device * mask.transform;
        Self {
            layers: mask.layers.iter().map(|layer| layer.then(placed)).collect(),
            path: placed * vello_cpu::kurbo::Rect::new(x, y, x + w, y + h).to_path(0.1),
        }
    }
}

impl Layer {
    fn is_root(&self) -> bool {
        matches!(self, Self::Backdrop { .. } | Self::Mask(_) | Self::Blur(_))
            || matches!(self, Self::Opacity(alpha) if *alpha < 1.0)
    }

    fn validate(&self) -> Result<(), String> {
        let (clip, alpha) = match self {
            Self::Clip(clip) => (Some(clip), 1.0),
            Self::Mask(_) => (None, 1.0),
            Self::Blur(radius) => {
                if !radius.is_finite() || !(0.0..=256.0).contains(radius) {
                    return Err("UI_COMPOSITE: blur outside 0..256 device pixels".into());
                }
                (None, 1.0)
            }
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
    draw_count: usize,
    /// Peak bytes of the frame-sized surfaces this plan holds at once.
    working_bytes: u64,
}

const OVER_BUDGET: &str = "UI_COMPOSITE: working surfaces exceed limit";

impl Plan {
    /// [`Plan::new`], flattening the deepest groups until the working surfaces fit: a frame
    /// loses its innermost opacity or blur before it fails. Returns how many were flattened.
    pub fn flattened(
        width: u16,
        height: u16,
        draw_count: usize,
        mut steps: Vec<Step>,
    ) -> Result<(Self, usize), String> {
        let mut flattened = 0;
        loop {
            match Self::new(width, height, draw_count, steps.clone()) {
                Err(error) if error == OVER_BUDGET => {
                    if !flatten_deepest(&mut steps) {
                        return Err(error);
                    }
                    flattened += 1;
                }
                result => return result.map(|plan| (plan, flattened)),
            }
        }
    }

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
        if width == 0 || height == 0 || width > 4096 || height > 4096 || steps.len() > 4096 {
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
                            let region = crate::scene::backdrop_region::Region::new(
                                *clip, width, height, *blur,
                            )?;
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
        // Working surfaces are frame-sized, so the bound scales with the frame: a tablet or a
        // 3x phone nests as deep as a small phone, under one absolute ceiling for huge windows.
        let budget = (crate::protocol::limits::COMPOSITE_FRAMES as u64 * frame_bytes)
            .max(64 * 1024 * 1024)
            .min(crate::protocol::limits::COMPOSITE_BYTES as u64);
        if peak > 32 || working_bytes > budget {
            return Err(OVER_BUDGET.into());
        }
        Ok(Self {
            steps,
            width,
            height,
            draw_count,
            working_bytes,
        })
    }

    pub fn working_bytes(&self) -> u64 {
        self.working_bytes
    }

    /// Flatten the deepest groups until the working surfaces fit in `bytes`, or no group is
    /// left. Returns how many were flattened.
    pub fn within(self, bytes: u64) -> (Self, usize) {
        let mut plan = self;
        let mut flattened = 0;
        while plan.working_bytes > bytes {
            let mut steps = plan.steps.clone();
            if !flatten_deepest(&mut steps) {
                break;
            }
            match Self::new(plan.width, plan.height, plan.draw_count, steps) {
                Ok(next) => plan = next,
                Err(_) => break,
            }
            flattened += 1;
        }
        (plan, flattened)
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

/// Removes the deepest group, preferring opacity and backdrop groups over clips, which keep
/// content inside its box. False when no group is left.
fn flatten_deepest(steps: &mut Vec<Step>) -> bool {
    let mut depth = 0usize;
    let mut best: Option<(bool, usize, usize)> = None;
    for (index, step) in steps.iter().enumerate() {
        match step {
            Step::Push(layer) => {
                depth += 1;
                let effect = !matches!(layer, Layer::Clip(_));
                if best.is_none_or(|(e, d, _)| (effect, depth) > (e, d)) {
                    best = Some((effect, depth, index));
                }
            }
            Step::Pop => depth = depth.saturating_sub(1),
            Step::Draw(_) => {}
        }
    }
    let Some((_, _, push)) = best else {
        return false;
    };
    let mut open = 0usize;
    let Some(pop) = steps[push..].iter().position(|step| {
        match step {
            Step::Push(_) => open += 1,
            Step::Pop => open -= 1,
            Step::Draw(_) => {}
        }
        open == 0
    }) else {
        return false;
    };
    steps.remove(push + pop);
    steps.remove(push);
    true
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
        assert!(Plan::new(4097, 2048, 0, vec![]).is_err());
        let nested = |depth| {
            let mut steps = vec![Step::Push(Layer::Opacity(0.5)); depth];
            steps.push(Step::Draw(0..1));
            steps.extend(vec![Step::Pop; depth]);
            steps
        };
        // The working budget scales with the frame: a 2x tablet nests as deep as a phone does.
        assert!(Plan::new(393 * 2, 852 * 2, 1, nested(4)).is_ok());
        assert!(Plan::new(1640, 2360, 1, nested(4)).is_ok());
        assert!(Plan::new(2732, 2048, 1, nested(4)).is_ok());
        assert!(Plan::new(1640, 2360, 1, nested(6)).is_err());
        // Over the budget, the deepest groups flatten instead of failing the frame.
        let (plan, flattened) = Plan::flattened(1640, 2360, 1, nested(8)).unwrap();
        assert!(flattened >= 1);
        assert!(
            plan.steps()
                .iter()
                .filter(|s| matches!(s, Step::Push(_)))
                .count()
                < 8
        );
        // Beside frames still held, a Retina desktop plan flattens to the room left, never fails.
        let frame = 2092 * 1576 * 4;
        let (plan, _) = Plan::flattened(2092, 1576, 1, nested(8)).unwrap();
        let (fitted, flattened) = plan.within(3 * frame);
        assert!(flattened >= 1);
        assert!(fitted.working_bytes() <= 3 * frame);
        let (bare, _) = fitted.within(0);
        assert!(!bare.steps().iter().any(|s| matches!(s, Step::Push(_))));
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
