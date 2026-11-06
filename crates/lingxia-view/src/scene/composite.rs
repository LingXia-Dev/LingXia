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

/// A device-pixel box `[x0, y0, x1, y1]` within the frame.
pub type Extent = [u16; 4];

/// Preflighted program. Draw spans refer to the caller's immutable display list.
pub struct Plan {
    steps: Vec<Step>,
    /// Each push's surface box, in push order: all its group can paint, and every backdrop
    /// region read from it.
    extents: Vec<Extent>,
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
        steps: Vec<Step>,
    ) -> Result<(Self, usize), String> {
        let extents = vec![[0, 0, width, height]; pushes(&steps)];
        Self::flattened_within(width, height, draw_count, steps, extents)
    }

    /// [`Plan::flattened`] with each push's content box.
    pub fn flattened_within(
        width: u16,
        height: u16,
        draw_count: usize,
        mut steps: Vec<Step>,
        mut extents: Vec<Extent>,
    ) -> Result<(Self, usize), String> {
        let mut flattened = 0;
        loop {
            match Self::bounded(width, height, draw_count, steps.clone(), extents.clone()) {
                Err(error) if error == OVER_BUDGET => {
                    if !flatten_deepest(&mut steps, &mut extents) {
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
    pub fn extents(&self) -> &[Extent] {
        &self.extents
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
        let extents = vec![[0, 0, width, height]; pushes(&steps)];
        Self::bounded(width, height, draw_count, steps, extents)
    }

    /// [`Plan::new`] with each push's content box, which backdrop regions then widen.
    pub fn bounded(
        width: u16,
        height: u16,
        draw_count: usize,
        steps: Vec<Step>,
        mut extents: Vec<Extent>,
    ) -> Result<Self, String> {
        if extents.len() != pushes(&steps) {
            return Err("UI_COMPOSITE: extent count".into());
        }
        for extent in &mut extents {
            *extent = within_frame(*extent, width, height);
        }
        let pixels = u64::from(width) * u64::from(height);
        if width == 0 || height == 0 || width > 4096 || height > 4096 || steps.len() > 4096 {
            return Err("UI_COMPOSITE: frame or program limit exceeded".into());
        }
        let mut depth = 1u64;
        let mut peak = depth;
        let mut layers: Vec<(&Layer, usize)> = Vec::new();
        let mut ordinal = 0;
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
                            let region_bytes = region.as_ref().map_or(0, |region| {
                                u64::from(region.width) * u64::from(region.height) * 4
                            });
                            let merges = layers
                                .iter()
                                .rev()
                                .take_while(|(layer, _)| !layer.is_root())
                                .filter(|(layer, _)| !matches!(layer, Layer::Clip(_)))
                                .count() as u64;
                            // The capture reads the region from every surface back to the
                            // backdrop root.
                            let reads = layers
                                .iter()
                                .rposition(|(layer, _)| layer.is_root())
                                .unwrap_or(0);
                            if let Some(region) = &region {
                                let read = [
                                    region.x,
                                    region.y,
                                    region.x + region.width,
                                    region.y + region.height,
                                ];
                                for (_, open) in &layers[reads..] {
                                    extents[*open] = union(extents[*open], read);
                                }
                            }
                            working_bytes =
                                working_bytes.max((depth + 1) * frame_bytes + region_bytes);
                            if merges > 0 {
                                working_bytes = working_bytes.max((depth + 2) * frame_bytes);
                            }
                        }
                        // A clip draws as a clip path into the surface it sits in.
                        Layer::Clip(_) => {}
                        _ => working_bytes = working_bytes.max((depth + 1) * frame_bytes),
                    }
                    layers.push((layer, ordinal));
                    ordinal += 1;
                    if !matches!(layer, Layer::Clip(_)) {
                        depth += 1;
                        peak = peak.max(depth);
                    }
                }
                Step::Pop if layers.is_empty() => {
                    return Err("UI_COMPOSITE: unmatched pop".into());
                }
                Step::Pop => {
                    if !matches!(layers.pop(), Some((Layer::Clip(_), _))) {
                        working_bytes = working_bytes.max((depth + 1) * frame_bytes);
                        depth -= 1;
                    }
                }
            }
        }
        if !layers.is_empty() {
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
            extents,
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
            let mut extents = plan.extents.clone();
            if !flatten_deepest(&mut steps, &mut extents) {
                break;
            }
            match Self::bounded(plan.width, plan.height, plan.draw_count, steps, extents) {
                Ok(next) => plan = next,
                Err(_) => break,
            }
            flattened += 1;
        }
        (plan, flattened)
    }

    pub fn render<B: Backend>(&self, backend: &mut B) -> Result<B::Surface, String> {
        let frame = [0, 0, self.width, self.height];
        let size = (self.width, self.height);
        let mut extents = self.extents.iter();
        let mut stack = vec![Level {
            layer: None,
            surface: backend.blank(frame)?,
            extent: frame,
            clips: Vec::new(),
        }];
        let mut opened: Vec<Opened> = Vec::new();
        let in_place = if B::INLINE_BACKDROPS {
            self.backdrops_in_place()
        } else {
            vec![false; self.steps.len()]
        };
        for (index, step) in self.steps.iter().enumerate() {
            match step {
                Step::Draw(range) => {
                    let level = stack.last_mut().unwrap();
                    backend.draw(
                        &mut level.surface,
                        level.extent,
                        range.clone(),
                        &level.clips,
                    )?
                }
                Step::Push(Layer::Clip(clip)) => {
                    extents.next();
                    stack.last_mut().unwrap().clips.push(*clip);
                    opened.push(Opened::Clip);
                }
                Step::Push(layer)
                    if in_place[index]
                        && stack
                            .iter()
                            .rposition(|level| level.layer.as_ref().is_some_and(Layer::is_root))
                            .unwrap_or(0)
                            + 1
                            >= stack.len() =>
                {
                    extents.next();
                    let live = stack.last_mut().unwrap();
                    backend.backdrop_into(
                        &mut live.surface,
                        live.extent,
                        layer,
                        size,
                        &live.clips,
                    )?;
                    opened.push(Opened::InPlace);
                }
                Step::Push(layer) => {
                    let pushed = *extents.next().unwrap();
                    let extent = if B::BOUNDED { pushed } else { frame };
                    let reused = if matches!(layer, Layer::Backdrop { .. }) {
                        backend.reuse(layer, size, extent)?
                    } else {
                        None
                    };
                    let surface = if let Some(surface) = reused {
                        surface
                    } else if matches!(layer, Layer::Backdrop { .. }) {
                        let nearest = stack
                            .iter()
                            .rposition(|level| level.layer.as_ref().is_some_and(Layer::is_root))
                            .unwrap_or(0);
                        if nearest + 1 >= stack.len() {
                            // Nothing to merge: the backdrop is read straight from the live surface.
                            let live = stack.last().unwrap();
                            backend.filter(&live.surface, live.extent, layer, size, extent)?
                        } else {
                            let top = stack.last().unwrap();
                            let mut capture = (backend.snapshot(&top.surface)?, top.extent);
                            // Merge unfinished groups from inner to outer. Each is composited
                            // once; never replay the display prefix.
                            for index in (nearest..stack.len() - 1).rev() {
                                let parent = &stack[index];
                                let mut merged = backend.snapshot(&parent.surface)?;
                                backend.composite(
                                    &mut merged,
                                    parent.extent,
                                    &capture.0,
                                    capture.1,
                                    stack[index + 1].layer.as_ref().unwrap(),
                                    &parent.clips,
                                )?;
                                capture = (merged, parent.extent);
                            }
                            backend.filter(&capture.0, capture.1, layer, size, extent)?
                        }
                    } else {
                        backend.blank(extent)?
                    };
                    stack.push(Level {
                        layer: Some(layer.clone()),
                        surface,
                        extent,
                        clips: Vec::new(),
                    });
                    opened.push(Opened::Surface);
                }
                Step::Pop => {
                    match opened.pop().unwrap() {
                        Opened::Clip => {
                            stack.last_mut().unwrap().clips.pop();
                            continue;
                        }
                        Opened::InPlace => continue,
                        Opened::Surface => {}
                    }
                    let level = stack.pop().unwrap();
                    let parent = stack.last_mut().unwrap();
                    backend.composite(
                        &mut parent.surface,
                        parent.extent,
                        &level.surface,
                        level.extent,
                        level.layer.as_ref().unwrap(),
                        &parent.clips,
                    )?;
                }
            }
        }
        Ok(stack.pop().unwrap().surface)
    }
}

/// What an open push made.
enum Opened {
    Surface,
    /// A clip on the surface it sits in.
    Clip,
    /// A backdrop drawn into the surface it sits in, its content after it.
    InPlace,
}

/// One surface of the composition stack and the clips open on it.
struct Level<S> {
    layer: Option<Layer>,
    surface: S,
    extent: Extent,
    /// Clip groups opened since this surface: they cut what is drawn or composited into it.
    clips: Vec<ClipSnapshot>,
}

impl Plan {
    /// Per step: a backdrop group at full opacity with no backdrop inside it. Its filtered image
    /// and content can draw straight into the surface it sits in.
    fn backdrops_in_place(&self) -> Vec<bool> {
        let mut result = vec![false; self.steps.len()];
        for (index, step) in self.steps.iter().enumerate() {
            let Step::Push(Layer::Backdrop { opacity, .. }) = step else {
                continue;
            };
            if *opacity < 1.0 {
                continue;
            }
            let mut depth = 0usize;
            let mut nested = false;
            for inner in &self.steps[index + 1..] {
                match inner {
                    Step::Push(Layer::Backdrop { .. }) => {
                        nested = true;
                        break;
                    }
                    Step::Push(_) => depth += 1,
                    Step::Pop if depth == 0 => break,
                    Step::Pop => depth -= 1,
                    Step::Draw(_) => {}
                }
            }
            result[index] = !nested;
        }
        result
    }
}

fn pushes(steps: &[Step]) -> usize {
    steps
        .iter()
        .filter(|step| matches!(step, Step::Push(_)))
        .count()
}

/// `extent` clamped to the frame, at least one pixel: a group that paints nothing still has
/// a surface.
fn within_frame([x0, y0, x1, y1]: Extent, width: u16, height: u16) -> Extent {
    let (x1, y1) = (x1.clamp(1, width.max(1)), y1.clamp(1, height.max(1)));
    [x0.min(x1 - 1), y0.min(y1 - 1), x1, y1]
}

fn union(a: Extent, b: Extent) -> Extent {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// Surfaces cover an [`Extent`] of the frame. A backend that is not `BOUNDED` gets the whole
/// frame for every surface.
pub trait Backend {
    type Surface;
    const BOUNDED: bool = false;
    fn blank(&mut self, extent: Extent) -> Result<Self::Surface, String>;
    /// Draws `span` into `target`, cut by `clips` (device pixels, outermost first).
    fn draw(
        &mut self,
        target: &mut Self::Surface,
        extent: Extent,
        span: Range<usize>,
        clips: &[ClipSnapshot],
    ) -> Result<(), String>;
    /// Must remain unchanged by subsequent writes to the original surface.
    fn snapshot(&mut self, source: &Self::Surface) -> Result<Self::Surface, String>;
    fn composite(
        &mut self,
        target: &mut Self::Surface,
        target_extent: Extent,
        source: &Self::Surface,
        source_extent: Extent,
        layer: &Layer,
        clips: &[ClipSnapshot],
    ) -> Result<(), String>;
    /// Initializes the owner's surface with its filtered, border-box-clipped backdrop. Reads
    /// only the backdrop's region of `source`, before anything else writes to it.
    fn filter(
        &mut self,
        source: &Self::Surface,
        source_extent: Extent,
        layer: &Layer,
        frame: (u16, u16),
        extent: Extent,
    ) -> Result<Self::Surface, String>;
    /// Whether [`Self::backdrop_into`] draws opaque-group backdrops straight into their parent.
    const INLINE_BACKDROPS: bool = false;
    /// Draws backdrop `layer`'s filtered image into `target`, read from `target` itself first,
    /// for a group that needs no surface of its own.
    fn backdrop_into(
        &mut self,
        _target: &mut Self::Surface,
        _target_extent: Extent,
        _layer: &Layer,
        _frame: (u16, u16),
        _clips: &[ClipSnapshot],
    ) -> Result<(), String> {
        Err("UI_COMPOSITE: backdrops need a surface on this backend".into())
    }
    /// The owner's surface for backdrop `layer` from an earlier frame's filtering, when nothing
    /// beneath it changed: no capture is needed.
    fn reuse(
        &mut self,
        _layer: &Layer,
        _frame: (u16, u16),
        _extent: Extent,
    ) -> Result<Option<Self::Surface>, String> {
        Ok(None)
    }
}

/// Removes the deepest group, preferring opacity and backdrop groups over clips, which keep
/// content inside its box. False when no group is left.
fn flatten_deepest(steps: &mut Vec<Step>, extents: &mut Vec<Extent>) -> bool {
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
    extents.remove(pushes(&steps[..push]));
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
