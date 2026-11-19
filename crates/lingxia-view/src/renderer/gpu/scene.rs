use crate::{
    DisplayList,
    scene::{display::SceneKey, node::ClipSnapshot, paint::Paint},
};
use std::{collections::HashMap, ops::Range, sync::Arc};
use vello_cpu::kurbo::Affine;

struct Entry {
    scene: vello_gpu::Scene,
    key: Option<SceneKey>,
    glyph_images: HashMap<usize, Arc<vello_gpu::Pixmap>>,
}

impl Entry {
    fn new() -> Self {
        Self {
            scene: vello_gpu::Scene::new(1, 1),
            key: None,
            glyph_images: HashMap::new(),
        }
    }

    fn matches(&self, display: &DisplayList) -> bool {
        self.key.as_ref().is_some_and(|key| key.matches(display))
    }

    fn build(
        &mut self,
        display: &DisplayList,
        glyphs: &mut crate::renderer::gpu::text::Glyphs<'_>,
    ) -> Result<(), String> {
        self.key = None;
        self.glyph_images.clear();
        let images = display
            .bitmaps()
            .map(|bitmap| {
                let pixels = bitmap.pixels();
                (
                    Arc::as_ptr(pixels) as usize,
                    crate::renderer::gpu::text::source(pixels),
                )
            })
            .collect();
        display.sparse_scene(glyphs, &mut self.scene, &images, &mut self.glyph_images)
    }
}

/// A run of commands encoded into its own scene. Segments draw in order onto one target.
pub(crate) struct Segment {
    pub(crate) animated: bool,
    pub(crate) range: Range<usize>,
    pub(crate) background: bool,
    /// Clips open at the start, with the transform each was opened under.
    pub(crate) clips: Vec<(ClipSnapshot, Affine)>,
    pub(crate) transform: Affine,
    pub(crate) open_at_end: usize,
}

/// The texture binding the first pass over a kept base samples it from.
pub(crate) const UNDERLAY: vello_gpu::TextureId = vello_gpu::TextureId(u64::MAX);

/// What a target slot holds after a frame drawn over a kept base: that base, the static
/// content over it, and the device box `[x0, y0, x1, y1]` its moving parts painted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Drawn {
    pub(crate) base: u64,
    pub(crate) generation: u64,
    pub(crate) spans: [u16; 4],
}

/// The box to draw again over a target holding `held` so it shows `now`: what either frame's
/// moving parts painted, when all else matches and the box is a quarter of the frame or less.
pub(crate) fn damage(
    held: Option<Drawn>,
    now: Option<Drawn>,
    width: u32,
    height: u32,
) -> Option<[u16; 4]> {
    let (held, now) = (held?, now?);
    if held.base != now.base || held.generation != now.generation {
        return None;
    }
    let empty = |b: [u16; 4]| b[0] >= b[2] || b[1] >= b[3];
    let [a, b] = [held.spans, now.spans];
    let area = match (empty(a), empty(b)) {
        (true, true) => return None,
        (true, false) => b,
        (false, true) => a,
        (false, false) => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
    };
    let pixels = u64::from(area[2] - area[0]) * u64::from(area[3] - area[1]);
    (pixels * 4 <= u64::from(width) * u64::from(height)).then_some(area)
}

/// The frame's commands outside its animated spans.
fn still(display: &DisplayList) -> impl Iterator<Item = &Paint> {
    let spans: Vec<Range<usize>> = display
        .animated_spans()
        .iter()
        .map(|span| span.commands.clone())
        .collect();
    display
        .commands
        .iter()
        .enumerate()
        .filter(move |(index, _)| !spans.iter().any(|span| span.contains(index)))
        .map(|(_, paint)| paint)
}

/// The device box the frame's animated spans paint, padded for antialiasing; `None` when a
/// span paints unbounded or inside a group whose pixels spread past what it paints.
fn spans_box(display: &DisplayList) -> Option<[u16; 4]> {
    // A backdrop samples what lies under it, the moving parts included.
    if display
        .commands
        .iter()
        .any(|paint| matches!(paint, Paint::Backdrop { .. }))
    {
        return None;
    }
    let spans: Vec<Range<usize>> = display
        .animated_spans()
        .iter()
        .map(|span| span.commands.clone())
        .collect();
    let mut transform = Affine::IDENTITY;
    let mut spreading = 0usize;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (index, paint) in display.commands.iter().enumerate() {
        let inside = spans.iter().any(|span| span.contains(&index));
        match paint {
            Paint::Transform(matrix) => {
                transform = crate::scene::paint::checked_paint_transform(*matrix).ok()?;
                continue;
            }
            Paint::Backdrop { .. } | Paint::Blur { .. } | Paint::Mask { .. } => {
                if inside {
                    return None;
                }
                spreading += 1;
                continue;
            }
            Paint::PopBackdrop | Paint::PopBlur | Paint::PopMask(_) => {
                spreading = spreading.saturating_sub(1);
                continue;
            }
            Paint::Clip(_) | Paint::PopClip | Paint::Opacity { .. } | Paint::PopOpacity => {
                continue;
            }
            _ if !inside => continue,
            _ if spreading > 0 => return None,
            _ => {}
        }
        match display.paint_box(paint, transform).ok()? {
            Some(Some([x, y, w, h])) => {
                (x0, y0) = (x0.min(x), y0.min(y));
                (x1, y1) = (x1.max(x + w), y1.max(y + h));
            }
            Some(None) => {}
            None => return None,
        }
    }
    if x0 > x1 || y0 > y1 {
        return Some([0; 4]);
    }
    let scale = display.scale;
    let clamp = |v: f32, max: u16| v.clamp(0.0, f32::from(max)) as u16;
    Some([
        clamp((x0 * scale - 2.0).floor(), display.width),
        clamp((y0 * scale - 2.0).floor(), display.height),
        clamp((x1 * scale + 2.0).ceil(), display.width),
        clamp((y1 * scale + 2.0).ceil(), display.height),
    ])
}

/// Each pass reloads the full target, so many spans cost more than one full encode.
const MAX_SPANS: usize = 8;

/// What a pass of its own costs against encoding its commands again each frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PassCost {
    /// GLES: every pass's upload and replay stalls the driver.
    High,
    Low,
}

/// Static commands that cost less to encode again each frame than a pass of their own. Glyphs
/// come from the atlas, so on GLES a short label's runs cost less than a pass.
fn cheap(commands: &[Paint], cost: PassCost) -> bool {
    let glyphs: usize = commands
        .iter()
        .map(|paint| match paint {
            Paint::Text { glyphs, .. } => glyphs.len(),
            _ => 0,
        })
        .sum();
    let limit = match cost {
        PassCost::High => 128,
        PassCost::Low => 48,
    };
    commands.len() <= 24 && glyphs <= limit
}

/// Split a frame at its animated spans so commands outside them keep their scenes. A
/// boundary never sits inside an opacity or backdrop group: a span inside one widens to
/// the outermost group, since a group's pixels only combine correctly in one scene.
fn segments(display: &DisplayList, cost: PassCost) -> Option<Vec<Segment>> {
    let spans = display.animated_spans();
    if spans.is_empty()
        || spans.len() > MAX_SPANS
        || !SceneKey::eligible(display, 2048, 32768)
        || display
            .commands
            .iter()
            .any(|paint| matches!(paint, Paint::WebHole { .. }))
    {
        return None;
    }
    let commands = &display.commands;
    let count = commands.len();
    let mut outer_group = vec![None; count + 1];
    let mut group_end = HashMap::new();
    let mut groups = Vec::new();
    for (index, paint) in commands.iter().enumerate() {
        outer_group[index] = groups.first().copied();
        match paint {
            Paint::Opacity { .. }
            | Paint::Backdrop { .. }
            | Paint::Mask { .. }
            | Paint::Blur { .. } => groups.push(index),
            Paint::PopOpacity | Paint::PopBackdrop | Paint::PopMask(_) | Paint::PopBlur => {
                group_end.insert(groups.pop()?, index);
            }
            _ => {}
        }
    }
    if !groups.is_empty() {
        return None;
    }
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut sorted: Vec<_> = spans.iter().map(|span| span.commands.clone()).collect();
    sorted.sort_by_key(|range| range.start);
    for mut range in sorted {
        if range.start > range.end || range.end > count {
            return None;
        }
        while let Some(group) = outer_group[range.start].or(outer_group[range.end]) {
            range = range.start.min(group)..range.end.max(group_end[&group] + 1);
        }
        let first = ranges.is_empty();
        match ranges.last_mut() {
            Some(last)
                if range.start <= last.end || cheap(&commands[last.end..range.start], cost) =>
            {
                last.end = last.end.max(range.end)
            }
            // An empty first span still ends the base, which then stays the same while it is clipped out.
            _ if range.is_empty() && !first => {}
            _ => ranges.push(range),
        }
    }
    // On GLES a cheap static tail costs less encoded again with the last span than a pass.
    if cost == PassCost::High
        && let Some(last) = ranges.last_mut()
        && cheap(&commands[last.end..], cost)
    {
        last.end = count;
    }
    let mut cuts = vec![(0, false)];
    for range in &ranges {
        if range.start > 0 {
            cuts.push((range.start, true));
        } else {
            cuts[0].1 = true;
        }
        cuts.push((range.end, false));
    }
    if cuts.last().is_some_and(|(at, _)| *at == count) && cuts.len() > 1 {
        cuts.pop();
    }
    // Layer state before each cut, from one walk over the commands.
    let mut states = Vec::with_capacity(cuts.len() + 1);
    let (mut clips, mut transform) = (Vec::new(), Affine::IDENTITY);
    let mut positions = cuts.iter().map(|(at, _)| *at).chain([count]).peekable();
    for index in 0..=count {
        while positions.next_if_eq(&index).is_some() {
            states.push((clips.clone(), transform));
        }
        match commands.get(index) {
            Some(Paint::Transform(matrix)) => transform = *matrix,
            Some(Paint::Clip(clip)) => clips.push((*clip, transform)),
            Some(Paint::PopClip) => {
                clips.pop()?;
            }
            _ => {}
        }
    }
    Some(
        cuts.iter()
            .enumerate()
            .map(|(index, &(start, animated))| {
                let end = cuts.get(index + 1).map_or(count, |(at, _)| *at);
                Segment {
                    animated,
                    range: start..end,
                    background: index == 0,
                    clips: states[index].0.clone(),
                    transform: states[index].1,
                    open_at_end: states[index + 1].0.len(),
                }
            })
            .collect(),
    )
}

/// What a static segment's pixels depend on beyond the frame's size and images.
#[derive(PartialEq)]
struct SegmentKey {
    background: bool,
    clips: Vec<(ClipSnapshot, Affine)>,
    /// `None` when the segment sets its own transform first.
    transform: Option<Affine>,
    commands: Vec<Paint>,
}

impl SegmentKey {
    fn of(display: &DisplayList, segment: &Segment) -> Self {
        let commands = &display.commands[segment.range.clone()];
        Self {
            background: segment.background,
            clips: segment.clips.clone(),
            transform: (!matches!(commands.first(), Some(Paint::Transform(_))))
                .then_some(segment.transform),
            commands: commands.to_vec(),
        }
    }
}

#[derive(PartialEq)]
struct FrameKey {
    size: [u16; 2],
    scale: f32,
    images: HashMap<String, usize>,
}

impl FrameKey {
    fn of(display: &DisplayList) -> Self {
        Self {
            size: [display.width, display.height],
            scale: display.scale,
            images: display
                .images
                .iter()
                .map(|(src, image)| (src.clone(), Arc::as_ptr(image.pixels()) as usize))
                .collect(),
        }
    }
}

struct SegmentEntry {
    entry: Entry,
    key: Option<SegmentKey>,
}

/// Keeps at most two eligible sparse scenes for recurring caret phases, or, while
/// something animates, one scene per segment with the static ones reused.
pub(crate) struct SceneCache {
    current: Entry,
    previous: Option<Entry>,
    segmented: bool,
    frame: Option<FrameKey>,
    segments: Vec<SegmentEntry>,
    /// Identifies the bottom static segment's content while it stays unchanged.
    base: Option<u64>,
    /// The renderer's render count when these scenes were last all encoded.
    pub(crate) encoded_at: u64,
    /// The video frame-size epoch these scenes were encoded under. Scenes key on content, never
    /// on texture contents: a new video frame keeps them valid, a new frame size does not.
    pub(crate) video_epoch: u64,
    /// Changes whenever `still` does, so equal values mean equal pixels outside the spans.
    generation: u64,
    /// The commands outside the animated spans when `generation` last changed.
    still: Vec<Paint>,
    spans: Option<[u16; 4]>,
    /// Everything over the base as one segment, for a frame redrawn only where it moved.
    over: Option<Segment>,
    damage: Option<Entry>,
    /// The frame draws as the damage scene alone.
    damaged: bool,
    /// Every kept segment is encoded again on next use, keeping what identifies its pixels.
    stale: bool,
}

static BASES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static GENERATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Default for SceneCache {
    fn default() -> Self {
        Self {
            current: Entry::new(),
            previous: None,
            segmented: false,
            frame: None,
            segments: Vec::new(),
            base: None,
            encoded_at: 0,
            video_epoch: 0,
            generation: 0,
            still: Vec::new(),
            spans: None,
            over: None,
            damage: None,
            damaged: false,
            stale: false,
        }
    }
}

impl SceneCache {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Encodes every scene again on next use, as glyphs age out of the atlas. Unchanged content
    /// keeps its identity, so a kept base and the targets drawn over it stay valid.
    pub(crate) fn refresh(&mut self) {
        self.current = Entry::new();
        self.previous = None;
        self.stale = true;
    }

    /// `true` when no scene was encoded again, or while animating, no static segment was.
    #[cfg(test)]
    pub(crate) fn prepare(
        &mut self,
        display: &DisplayList,
        glyphs: &mut crate::renderer::gpu::text::Glyphs<'_>,
    ) -> Result<bool, String> {
        self.prepare_over(display, glyphs, false, PassCost::High)
    }

    /// [`Self::prepare`]; with `underlay`, the pass over a kept base starts from its pixels.
    pub(crate) fn prepare_over(
        &mut self,
        display: &DisplayList,
        glyphs: &mut crate::renderer::gpu::text::Glyphs<'_>,
        underlay: bool,
        cost: PassCost,
    ) -> Result<bool, String> {
        self.damaged = false;
        if let Some(plan) = segments(display, cost) {
            // The segment over a kept base starts from its pixels.
            let underlay = (underlay && !plan[0].animated).then_some(1);
            return self.prepare_segments(display, &plan, glyphs, underlay);
        }
        self.segmented = false;
        self.frame = None;
        self.segments.clear();
        self.base = None;
        self.over = None;
        if self.current.matches(display) {
            return Ok(true);
        }
        if self
            .previous
            .as_ref()
            .is_some_and(|entry| entry.matches(display))
        {
            std::mem::swap(&mut self.current, self.previous.as_mut().unwrap());
            return Ok(true);
        }
        let key = SceneKey::capture(display);
        if key.is_none() {
            self.previous = None;
            self.current.build(display, glyphs)?;
            return Ok(false);
        }
        let mut next = self.previous.take().unwrap_or_else(Entry::new);
        next.build(display, glyphs)?;
        let bitmap_bytes: usize = next
            .glyph_images
            .values()
            .map(|pixels| usize::from(pixels.width()) * usize::from(pixels.height()) * 4)
            .sum();
        next.key = (bitmap_bytes <= 8 * 1024 * 1024).then_some(key).flatten();
        let cacheable = next.key.is_some();
        let old = std::mem::replace(&mut self.current, next);
        // Drop an oversized uncached scene instead of retaining its allocations as a spare.
        self.previous = (cacheable && old.key.is_some()).then_some(old);
        Ok(false)
    }

    fn prepare_segments(
        &mut self,
        display: &DisplayList,
        plan: &[Segment],
        glyphs: &mut crate::renderer::gpu::text::Glyphs<'_>,
        underlay: Option<usize>,
    ) -> Result<bool, String> {
        self.segmented = true;
        let frame = FrameKey::of(display);
        let fresh = self.frame.as_ref() != Some(&frame);
        if fresh {
            self.segments.iter_mut().for_each(|entry| entry.key = None);
            self.frame = Some(frame);
        }
        let mut keys: Vec<_> = plan
            .iter()
            .map(|segment| (!segment.animated).then(|| SegmentKey::of(display, segment)))
            .collect();
        let mut old: Vec<_> = std::mem::take(&mut self.segments)
            .into_iter()
            .map(Some)
            .collect();
        let mut next: Vec<Option<SegmentEntry>> = keys
            .iter()
            .map(|key| {
                let key = key.as_ref()?;
                old.iter_mut()
                    .find(|entry| entry.as_ref().is_some_and(|e| e.key.as_ref() == Some(key)))?
                    .take()
            })
            .collect();
        let hit = keys
            .iter()
            .zip(&next)
            .all(|(key, entry)| key.is_none() || entry.is_some());
        let base_kept = !plan[0].animated && next[0].is_some();
        let mut spare = old.into_iter().flatten();
        let images = display
            .bitmaps()
            .map(|bitmap| {
                let pixels = bitmap.pixels();
                (
                    Arc::as_ptr(pixels) as usize,
                    crate::renderer::gpu::text::source(pixels),
                )
            })
            .collect();
        for (index, segment) in plan.iter().enumerate() {
            if next[index].is_some() && !self.stale {
                continue;
            }
            let mut entry = next[index]
                .take()
                .or_else(|| spare.next())
                .unwrap_or_else(|| SegmentEntry {
                    entry: Entry::new(),
                    key: None,
                });
            entry.key = None;
            entry.entry.glyph_images.clear();
            display.sparse_segment(
                glyphs,
                &mut entry.entry.scene,
                &images,
                &mut entry.entry.glyph_images,
                segment,
                underlay == Some(index),
            )?;
            entry.key = keys[index].take();
            next[index] = Some(entry);
        }
        self.segments = next.into_iter().flatten().collect();
        let bitmap_bytes: usize = self
            .glyph_images()
            .map(|pixels| usize::from(pixels.width()) * usize::from(pixels.height()) * 4)
            .sum();
        if bitmap_bytes > 8 * 1024 * 1024 {
            self.segments.iter_mut().for_each(|entry| entry.key = None);
        }
        self.base = if plan[0].animated {
            None
        } else if base_kept && self.base.is_some() {
            self.base
        } else {
            Some(BASES.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        };
        let stale = std::mem::take(&mut self.stale);
        // Static runs folded into a span are encoded with it, so their keys cannot tell.
        if fresh || self.generation == 0 || !still(display).eq(self.still.iter()) {
            self.generation = GENERATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.still = still(display).cloned().collect();
        }
        self.spans = spans_box(display);
        self.over = plan
            .get(1)
            .filter(|_| !plan[0].animated)
            .map(|first| Segment {
                animated: true,
                range: first.range.start..display.commands.len(),
                background: false,
                clips: first.clips.clone(),
                transform: first.transform,
                open_at_end: plan.last().map_or(0, |last| last.open_at_end),
            });
        Ok(hit && !stale)
    }

    /// What a target holds once this frame is drawn over its kept base.
    pub(crate) fn drawn(&self) -> Option<Drawn> {
        self.over.as_ref()?;
        Some(Drawn {
            base: self.base()?,
            generation: self.generation,
            spans: self.spans?,
        })
    }

    /// Encodes everything over the kept base inside `area` as the frame's one scene, to draw
    /// over a target that holds the same frame outside it.
    pub(crate) fn prepare_damage(
        &mut self,
        display: &DisplayList,
        glyphs: &mut crate::renderer::gpu::text::Glyphs<'_>,
        area: [u16; 4],
    ) -> Result<(), String> {
        let over = self
            .over
            .as_ref()
            .ok_or("UI_SURFACE: nothing over the base")?;
        let entry = self.damage.get_or_insert_with(Entry::new);
        entry.glyph_images.clear();
        let images = display
            .bitmaps()
            .map(|bitmap| {
                let pixels = bitmap.pixels();
                (
                    Arc::as_ptr(pixels) as usize,
                    crate::renderer::gpu::text::source(pixels),
                )
            })
            .collect();
        display.sparse_damage(
            glyphs,
            &mut entry.scene,
            &images,
            &mut entry.glyph_images,
            over,
            area,
        )?;
        self.damaged = true;
        Ok(())
    }

    /// While animating over a static bottom segment: an id that changes whenever that
    /// segment's pixels may change, so its rendering can be kept between frames.
    pub(crate) fn base(&self) -> Option<u64> {
        self.base.filter(|_| self.segmented)
    }

    /// Scenes to draw in order; the first clears the target.
    pub(crate) fn scenes(&self) -> Box<dyn Iterator<Item = &vello_gpu::Scene> + '_> {
        if let Some(damage) = self.damage.as_ref().filter(|_| self.damaged) {
            Box::new(std::iter::once(&damage.scene))
        } else if self.segmented {
            Box::new(self.segments.iter().map(|entry| &entry.entry.scene))
        } else {
            Box::new(std::iter::once(&self.current.scene))
        }
    }

    pub(crate) fn glyph_images(&self) -> Box<dyn Iterator<Item = &Arc<vello_gpu::Pixmap>> + '_> {
        if let Some(damage) = self.damage.as_ref().filter(|_| self.damaged) {
            Box::new(damage.glyph_images.values())
        } else if self.segmented {
            Box::new(
                self.segments
                    .iter()
                    .flat_map(|entry| entry.entry.glyph_images.values()),
            )
        } else {
            Box::new(self.current.glyph_images.values())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{Visual, interpolate::Transform};
    use std::collections::BTreeMap;

    /// A rotating child inside a rounded clip between static siblings, one a focused
    /// input whose caret blinks; with `faded`, the clip is translucent.
    fn spin_page(faded: bool) -> (crate::ViewSession, DisplayList) {
        spin_page_on(faded, ",backgroundColor:'#ffffff'")
    }

    /// [`spin_page`] with `background` as the root's background style.
    fn spin_page_on(faded: bool, background: &str) -> (crate::ViewSession, DisplayList) {
        let opacity = if faded { ",opacity:0.8" } else { "" };
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('view',{{style:{{width:96,height:96{background}}}}},h('text',{{}},'Before'),h('view',{{style:{{width:80,height:56,borderRadius:8,overflow:'hidden',backgroundColor:'#eeeeee'{opacity}}}}},h('view',{{'data-testid':'spin',style:{{width:30,height:30,backgroundColor:'#00ff00'}}}},h('text',{{}},'Go')),h('input',{{value:'Hi',autoFocus:true}}),h('view',{{style:{{height:10,backgroundColor:'#000000'}}}})),h('text',{{}},'After'))}}"
        ));
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.layout(96, 96, 1.0).unwrap();
        (page, display)
    }

    fn spun(display: &DisplayList, angle: f64) -> DisplayList {
        let spin = display
            .nodes
            .iter()
            .position(|n| n.test_id == "spin")
            .unwrap();
        let visual = Visual {
            transforms: vec![Transform::Rotate(angle)],
            ..Default::default()
        };
        let mut frame = display
            .retained
            .compose_animated(
                &HashMap::new(),
                &BTreeMap::from([(display.nodes[spin].identity.clone(), visual)]),
            )
            .unwrap();
        frame.bind_images(&display.images).unwrap();
        for (node, laid) in frame.nodes.iter_mut().zip(&display.nodes) {
            if let (Some(input), Some(laid)) = (&mut node.input, &laid.input) {
                input.edit = laid.edit.clone();
            }
        }
        frame.decorate_inputs();
        assert!(frame.caret_paint.is_some());
        frame
    }

    #[test]
    fn a_span_clipped_out_keeps_the_base_and_the_damage_history() {
        let module = crate::session::tests::source(
            "function render(){return h('view',{style:{width:200,height:120,backgroundColor:'#ffffff'}},h('text',{},'Top'),h('view',{style:{height:4,overflow:'hidden',backgroundColor:'#eeeeee'}},h('view',{'data-testid':'bar',style:{width:'40%',height:'100%',backgroundColor:'#00ff00'}})),h('view',{'data-testid':'spin',style:{width:30,height:30,backgroundColor:'#ff0000'}}),h('text',{},'After'))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.layout(200, 120, 1.0).unwrap();
        let id = |test_id: &str| {
            display
                .nodes
                .iter()
                .find(|node| node.test_id == test_id)
                .unwrap()
                .identity
                .clone()
        };
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        // The bar slides across its track and out of it, as an indeterminate progress bar does.
        let mut drawn = Vec::new();
        for x in [-0.5, -1.0, 2.5, 1.0] {
            let visuals = BTreeMap::from([
                (
                    id("bar"),
                    Visual {
                        transforms: vec![Transform::Relative([x, 0.0])],
                        ..Default::default()
                    },
                ),
                (
                    id("spin"),
                    Visual {
                        transforms: vec![Transform::Rotate(30.0)],
                        ..Default::default()
                    },
                ),
            ]);
            let frame = display
                .retained
                .compose_animated(&HashMap::new(), &visuals)
                .unwrap();
            cache
                .prepare_over(&frame, &mut glyphs, true, PassCost::Low)
                .unwrap();
            drawn.push(cache.drawn().unwrap());
        }
        for (x, next) in [-1.0, 2.5, 1.0].iter().zip(&drawn[1..]) {
            assert_eq!(
                (next.base, next.generation),
                (drawn[0].base, drawn[0].generation),
                "x={x}"
            );
        }
    }

    #[test]
    fn animation_frames_reencode_only_their_spans() {
        let (_page, display) = spin_page(false);
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        assert!(!cache.prepare(&spun(&display, 0.0), &mut glyphs).unwrap());
        let plan = segments(&spun(&display, 0.0), PassCost::High).unwrap();
        assert_eq!(
            plan.iter().map(|s| s.animated).collect::<Vec<_>>(),
            [false, true],
            "the cheap static tail draws with the span"
        );
        let separate = segments(&spun(&display, 0.0), PassCost::Low).unwrap();
        assert_eq!(
            separate.iter().map(|s| s.animated).collect::<Vec<_>>(),
            [false, true, false],
            "where passes are cheap the static tail keeps its own scene"
        );
        assert_eq!(
            plan[1].clips.len(),
            1,
            "the span opens inside its parent's clip"
        );
        assert_eq!(plan[0].open_at_end, 1);
        assert_eq!(plan[1].open_at_end, 0);
        let base = cache.base();
        assert!(base.is_some());
        for angle in [10.0, 20.0, 30.0] {
            assert!(cache.prepare(&spun(&display, angle), &mut glyphs).unwrap());
            assert_eq!(cache.scenes().count(), 2);
            assert_eq!(cache.base(), base, "the bottom segment's pixels are kept");
        }
        let shown = spun(&display, 40.0);
        let mut hidden = spun(&display, 40.0);
        hidden.hide_caret();
        let (a, b) = (&shown.animated_spans()[0], &hidden.animated_spans()[0]);
        assert!(
            shown.commands[a.commands.clone()] == hidden.commands[b.commands.clone()],
            "spans follow their commands past the removed caret"
        );
        assert!(
            cache.prepare(&hidden, &mut glyphs).unwrap(),
            "the caret blinks in the span's pass, with the static tail folded into it"
        );
        assert_eq!(
            cache.base(),
            base,
            "the caret blinks above the bottom segment"
        );
        assert!(!cache.prepare(&display, &mut glyphs).unwrap());
        assert_eq!(cache.scenes().count(), 1);
        assert_eq!(cache.base(), None, "a still page keeps no base");
        assert!(cache.segments.is_empty());
    }

    #[test]
    fn spans_split_by_a_few_static_paints_share_one_pass() {
        let module = crate::session::tests::source(
            "function render(){return h('view',{style:{width:96,height:96}},h('text',{},'Before'),h('view',{'data-testid':'a',style:{width:10,height:10,backgroundColor:'#00ff00'}}),h('view',{style:{width:10,height:10,backgroundColor:'#000000'}}),h('view',{'data-testid':'b',style:{width:10,height:10,backgroundColor:'#0000ff'}}),h('text',{},'After'))}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let display = page.layout(96, 96, 1.0).unwrap();
        let visuals = display
            .nodes
            .iter()
            .filter(|n| n.test_id == "a" || n.test_id == "b")
            .map(|n| {
                let visual = Visual {
                    transforms: vec![Transform::Rotate(20.0)],
                    ..Default::default()
                };
                (n.identity.clone(), visual)
            })
            .collect::<BTreeMap<_, _>>();
        let frame = display
            .retained
            .compose_animated(&HashMap::new(), &visuals)
            .unwrap();
        assert_eq!(frame.animated_spans().len(), 2);
        let plan = segments(&frame, PassCost::High).unwrap();
        assert_eq!(
            plan.iter().map(|s| s.animated).collect::<Vec<_>>(),
            [false, true]
        );
    }

    #[test]
    fn a_span_inside_a_group_widens_to_the_group() {
        let (_page, display) = spin_page(true);
        let frame = spun(&display, 15.0);
        let plan = segments(&frame, PassCost::High).unwrap();
        let animated: Vec<_> = plan.iter().filter(|s| s.animated).collect();
        assert_eq!(animated.len(), 1);
        let span = &frame.animated_spans()[0].commands;
        let range = &animated[0].range;
        assert!(range.start < span.start && span.end < range.end);
        assert!(matches!(frame.commands[range.start], Paint::Opacity { .. }));
        assert!(
            frame.commands[span.end..range.end]
                .iter()
                .any(|paint| matches!(paint, Paint::PopOpacity)),
            "the group closes inside the widened span"
        );
    }

    #[cfg(feature = "gpu")]
    #[test]
    #[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
    fn segmented_frames_match_one_scene() {
        use crate::renderer::{backdrop_tests::readback, gpu::pass};
        let domain = crate::renderer::gpu::Context::default();
        let mut context = domain.lock().unwrap();
        let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
        let device = &context.devices[id].device;
        let queue = &context.devices[id].queue;
        let mut renderer = pass::SceneRenderer::default();
        let size = vello_gpu::RenderSize {
            width: 96,
            height: 96,
        };
        let depth = vello_gpu::Renderer::create_depth_texture_view(device, &size);
        let mut draw = |scenes: Vec<&vello_gpu::Scene>,
                        images: Vec<Arc<vello_gpu::Pixmap>>,
                        underlay: Option<&wgpu::TextureView>|
         -> (Vec<u8>, wgpu::TextureView) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 96,
                    height: 96,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let mut bindings = crate::renderer::gpu::images::ImageTextures::default()
                .bindings(device, queue, images);
            if let Some(base) = underlay {
                bindings.insert(UNDERLAY, base.clone());
            }
            for (index, scene) in scenes.into_iter().enumerate() {
                renderer
                    .draw(
                        device,
                        queue,
                        scene,
                        &bindings,
                        pass::Target {
                            size: size.clone(),
                            view: &view,
                            depth: Some(&depth),
                            init: if index == 0 {
                                vello_gpu::TargetInit::Clear(Default::default())
                            } else {
                                vello_gpu::TargetInit::SrcOver
                            },
                        },
                    )
                    .unwrap();
            }
            (readback(device, queue, &texture), view)
        };
        for faded in [false, true] {
            let (_page, display) = spin_page(faded);
            let mut cache = SceneCache::default();
            let mut prep = glifo::GlyphPrepCache::default();
            let mut glyphs = crate::renderer::gpu::text::Glyphs {
                prep: &mut prep,
                atlas: None,
            };
            for angle in [0.0, 25.0, 50.0] {
                let frame = spun(&display, angle);
                cache.prepare(&frame, &mut glyphs).unwrap();
                assert!(cache.segmented);
                let (actual, _) = draw(
                    cache.scenes().collect(),
                    cache.glyph_images().cloned().collect(),
                    None,
                );
                let mut whole = Entry::new();
                whole.build(&frame, &mut glyphs).unwrap();
                let (expected, _) = draw(
                    vec![&whole.scene],
                    whole.glyph_images.values().cloned().collect(),
                    None,
                );
                // The kept base drawn alone, then sampled by the pass over it.
                let images: Vec<_> = cache.glyph_images().cloned().collect();
                let (_, base) = draw(cache.scenes().take(1).collect(), images.clone(), None);
                cache
                    .prepare_over(&frame, &mut glyphs, true, PassCost::High)
                    .unwrap();
                let (underlaid, _) = draw(cache.scenes().skip(1).collect(), images, Some(&base));
                for (index, ((a, e), u)) in actual.iter().zip(&expected).zip(&underlaid).enumerate()
                {
                    assert!(
                        a.abs_diff(*e) <= 2,
                        "faded={faded} angle={angle} byte={index}: {a} vs {e}"
                    );
                    assert_eq!(u, a, "faded={faded} angle={angle} byte={index}: underlay");
                }
            }
        }
    }

    /// `bg-clip-text`: the gradient shows through the glyphs and nowhere else, whichever way the
    /// glyphs are drawn.
    #[cfg(feature = "gpu")]
    #[test]
    #[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
    fn clipped_text_shows_its_gradient_through_its_glyphs() {
        use crate::renderer::{backdrop_tests::readback, gpu::pass};
        // The readback helper reads 96 x 96.
        const W: u32 = 96;
        const H: u32 = 96;
        // Authored colours, then what Tailwind's `bg-linear-to-r from-accent to-danger` lowers to.
        let pages = [
            "{direction:'right',colorSpace:'srgb',stops:[{offset:0,color:'#ff0000'},{offset:1,color:'#0000ff'}]}",
            "{direction:'right',colorSpace:'oklab',stops:[{offset:0,color:'accent'},{offset:1,color:'danger'}]}",
        ]
        .map(|gradient| {
            let module = crate::session::tests::source(&format!(
                "function render(){{return h('view',{{style:{{width:96,height:96,backgroundColor:'#ffffff'}}}},h('text',{{style:{{fontSize:40,fontWeight:700,color:'#00000000',backgroundClip:'text',backgroundImage:{gradient}}}}},'WW'))}}"
            ));
            let mut page = crate::ViewSession::new(&module).unwrap();
            let display = page.layout(W as u16, H as u16, 1.0).unwrap();
            (page, display)
        });
        let domain = crate::renderer::gpu::Context::default();
        let mut context = domain.lock().unwrap();
        let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
        let device = &context.devices[id].device;
        let queue = &context.devices[id].queue;
        let size = vello_gpu::RenderSize {
            width: W as _,
            height: H as _,
        };
        let depth = vello_gpu::Renderer::create_depth_texture_view(device, &size);
        for (which, atlas) in [(0, false), (0, true), (1, false), (1, true)] {
            let display = &pages[which].1;
            let mut renderer = pass::SceneRenderer::default();
            let mut prep = glifo::GlyphPrepCache::default();
            let mut entry = Entry::new();
            if atlas {
                renderer
                    .with_resources(device, W as u16, H as u16, |resources| {
                        entry.build(
                            display,
                            &mut crate::renderer::gpu::text::Glyphs {
                                prep: &mut prep,
                                atlas: Some(resources),
                            },
                        )
                    })
                    .unwrap()
                    .unwrap();
            } else {
                entry
                    .build(
                        display,
                        &mut crate::renderer::gpu::text::Glyphs {
                            prep: &mut prep,
                            atlas: None,
                        },
                    )
                    .unwrap();
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: W,
                    height: H,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bindings = crate::renderer::gpu::images::ImageTextures::default().bindings(
                device,
                queue,
                entry.glyph_images.values().cloned().collect::<Vec<_>>(),
            );
            renderer
                .draw(
                    device,
                    queue,
                    &entry.scene,
                    &bindings,
                    pass::Target {
                        size: size.clone(),
                        view: &view,
                        depth: Some(&depth),
                        init: vello_gpu::TargetInit::Clear(Default::default()),
                    },
                )
                .unwrap();
            let data = readback(device, queue, &texture);
            let (mut coloured, mut white) = (0, 0);
            for pixel in data.chunks_exact(4) {
                let [low, high] = [
                    pixel[..3].iter().min().unwrap(),
                    pixel[..3].iter().max().unwrap(),
                ];
                if *low > 240 {
                    white += 1;
                } else if high - low > 80 {
                    coloured += 1;
                }
            }
            assert!(
                coloured > 150,
                "page {which} atlas={atlas}: {coloured} coloured, {white} white"
            );
            assert!(
                white > coloured,
                "page {which} atlas={atlas}: the box is not filled"
            );
        }
    }

    #[test]
    fn a_static_change_folded_into_a_span_ends_the_damage_history() {
        let (_page, display) = spin_page(false);
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        let first_frame = spun(&display, 0.0);
        cache
            .prepare_over(&first_frame, &mut glyphs, true, PassCost::High)
            .unwrap();
        let first = cache.drawn().unwrap();
        // The last text, after the span: High cost encodes it with the span.
        let mut changed = spun(&display, 10.0);
        let span_end = changed.animated_spans()[0].commands.end;
        let tail = changed.commands[span_end..]
            .iter()
            .rposition(|paint| matches!(paint, Paint::Text { .. }))
            .unwrap();
        if let Paint::Text { color, .. } = &mut changed.commands[span_end + tail] {
            *color = [255, 0, 0, 255];
        }
        let plan = segments(&changed, PassCost::High).unwrap();
        assert!(
            plan.last().unwrap().animated
                && plan.last().unwrap().range.end == changed.commands.len()
        );
        cache
            .prepare_over(&changed, &mut glyphs, true, PassCost::High)
            .unwrap();
        let next = cache.drawn().unwrap();
        assert_eq!(next.base, first.base, "the base is unchanged");
        assert_ne!(
            next.generation, first.generation,
            "the tail drawn with the span changed outside its box"
        );
    }

    #[test]
    fn damage_needs_the_same_base_and_static_content_and_a_small_box() {
        let drawn = |base, generation, spans| {
            Some(Drawn {
                base,
                generation,
                spans,
            })
        };
        let held = drawn(1, 1, [10, 10, 20, 20]);
        assert_eq!(
            damage(held, drawn(1, 1, [30, 10, 40, 20]), 100, 100),
            Some([10, 10, 40, 20])
        );
        assert_eq!(damage(held, drawn(2, 1, [10, 10, 20, 20]), 100, 100), None);
        assert_eq!(damage(held, drawn(1, 2, [10, 10, 20, 20]), 100, 100), None);
        assert_eq!(damage(None, drawn(1, 1, [10, 10, 20, 20]), 100, 100), None);
        assert_eq!(
            damage(held, drawn(1, 1, [0, 0, 0, 0]), 100, 100),
            Some([10, 10, 20, 20]),
            "a span that paints nothing leaves the held box to clear"
        );
        assert_eq!(
            damage(held, drawn(1, 1, [50, 50, 70, 70]), 100, 100),
            None,
            "over a quarter of the frame draws it whole"
        );
    }

    #[test]
    fn spans_box_covers_the_turning_child_and_tracks_the_static_content() {
        let (_page, display) = spin_page(false);
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        cache
            .prepare_over(&spun(&display, 0.0), &mut glyphs, true, PassCost::Low)
            .unwrap();
        let first = cache.drawn().unwrap();
        let [x0, y0, x1, y1] = first.spans;
        assert!(x0 < x1 && y0 < y1 && x1 <= 96 && y1 <= 96);
        assert!(u32::from(x1 - x0) * u32::from(y1 - y0) < 96 * 96 / 4);
        cache
            .prepare_over(&spun(&display, 45.0), &mut glyphs, true, PassCost::Low)
            .unwrap();
        let turned = cache.drawn().unwrap();
        assert_eq!(
            (turned.base, turned.generation),
            (first.base, first.generation)
        );
        assert!(
            turned.spans[2] - turned.spans[0] > x1 - x0,
            "a turned square spans wider"
        );
        cache.refresh();
        assert!(
            !cache
                .prepare_over(&spun(&display, 60.0), &mut glyphs, true, PassCost::Low)
                .unwrap(),
            "a refresh encodes the kept segments again"
        );
        let refreshed = cache.drawn().unwrap();
        assert_eq!(
            (refreshed.base, refreshed.generation),
            (first.base, first.generation),
            "unchanged content keeps its kept base and the targets drawn over it"
        );
        cache.clear();
        cache
            .prepare_over(&spun(&display, 45.0), &mut glyphs, true, PassCost::Low)
            .unwrap();
        assert_ne!(cache.drawn().unwrap().generation, first.generation);
    }

    #[test]
    fn a_still_video_keeps_the_damage_box_and_a_moved_one_ends_the_history() {
        let (_page, display) = spin_page(false);
        let video = |x: f32| Paint::Video {
            player: "clip".into(),
            bounds: [x, 0.0, 48.0, 27.0],
            radius: 0.0,
            fit: crate::scene::image::Fit::Cover,
            position: [0.5, 0.5],
        };
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        let mut frame = |angle: f64, x: f32| {
            let mut playing = spun(&display, angle);
            playing.commands.push(video(x));
            cache
                .prepare_over(&playing, &mut glyphs, true, PassCost::Low)
                .unwrap();
            cache.drawn()
        };
        let first = frame(0.0, 0.0).expect("a video's frame changes are gated by the presenter");
        let turned = frame(45.0, 0.0).unwrap();
        assert_eq!(
            (turned.base, turned.generation),
            (first.base, first.generation)
        );
        let moved = frame(45.0, 4.0).unwrap();
        assert_ne!(
            moved.generation, first.generation,
            "a moved video is static content"
        );
    }

    #[test]
    #[ignore = "requires a GPU adapter; run explicitly with --features gpu -- --ignored"]
    fn a_damage_frame_over_the_frame_before_last_matches_the_whole_frame() {
        use crate::renderer::{backdrop_tests::readback, gpu::pass};
        let domain = crate::renderer::gpu::Context::default();
        let mut context = domain.lock().unwrap();
        let id = pollster::block_on(context.device(None)).expect("GPU adapter required");
        let device = &context.devices[id].device;
        let queue = &context.devices[id].queue;
        let mut renderer = pass::SceneRenderer::default();
        let size = vello_gpu::RenderSize {
            width: 96,
            height: 96,
        };
        let depth = vello_gpu::Renderer::create_depth_texture_view(device, &size);
        let texture = || {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 96,
                    height: 96,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
        };
        let mut draw = |scenes: Vec<&vello_gpu::Scene>,
                        images: Vec<Arc<vello_gpu::Pixmap>>,
                        underlay: Option<&wgpu::TextureView>,
                        onto: &wgpu::Texture,
                        clear: bool| {
            let view = onto.create_view(&Default::default());
            let mut bindings = crate::renderer::gpu::images::ImageTextures::default()
                .bindings(device, queue, images);
            if let Some(base) = underlay {
                bindings.insert(UNDERLAY, base.clone());
            }
            for (index, scene) in scenes.into_iter().enumerate() {
                renderer
                    .draw(
                        device,
                        queue,
                        scene,
                        &bindings,
                        pass::Target {
                            size: size.clone(),
                            view: &view,
                            depth: Some(&depth),
                            init: if index == 0 && clear {
                                vello_gpu::TargetInit::Clear(Default::default())
                            } else {
                                vello_gpu::TargetInit::SrcOver
                            },
                        },
                    )
                    .unwrap();
            }
        };
        // The base starts with the opaque frame fill, so a translucent page still covers the box.
        let backgrounds = [
            "",
            ",backgroundColor:'accent/12'",
            ",backgroundColor:'#ffffff'",
        ];
        for (cost, background) in [PassCost::High, PassCost::Low]
            .into_iter()
            .flat_map(|cost| backgrounds.map(|background| (cost, background)))
        {
            for faded in [false, true] {
                let (_page, display) = spin_page_on(faded, background);
                let mut cache = SceneCache::default();
                let mut prep = glifo::GlyphPrepCache::default();
                let mut glyphs = crate::renderer::gpu::text::Glyphs {
                    prep: &mut prep,
                    atlas: None,
                };
                // One target holds the 10° frame drawn whole over the kept base.
                cache
                    .prepare_over(&spun(&display, 10.0), &mut glyphs, true, cost)
                    .unwrap();
                let images: Vec<_> = cache.glyph_images().cloned().collect();
                let base = texture();
                draw(
                    cache.scenes().take(1).collect(),
                    images.clone(),
                    None,
                    &base,
                    true,
                );
                let base = base.create_view(&Default::default());
                let target = texture();
                draw(
                    cache.scenes().skip(1).collect(),
                    images,
                    Some(&base),
                    &target,
                    true,
                );
                let held = cache.drawn();
                // 30° goes to the other target; 50° comes back to this one.
                for angle in [30.0, 50.0] {
                    cache
                        .prepare_over(&spun(&display, angle), &mut glyphs, true, cost)
                        .unwrap();
                }
                let images: Vec<_> = cache.glyph_images().cloned().collect();
                let whole = texture();
                draw(
                    cache.scenes().skip(1).collect(),
                    images,
                    Some(&base),
                    &whole,
                    true,
                );
                let area = damage(held, cache.drawn(), 96, 96)
                    .unwrap_or_else(|| panic!("cost={cost:?} {background} faded={faded}: no box"));
                cache
                    .prepare_damage(&spun(&display, 50.0), &mut glyphs, area)
                    .unwrap();
                let images: Vec<_> = cache.glyph_images().cloned().collect();
                draw(
                    cache.scenes().collect(),
                    images,
                    Some(&base),
                    &target,
                    false,
                );
                let (actual, expected) = (
                    readback(device, queue, &target),
                    readback(device, queue, &whole),
                );
                for (index, (a, e)) in actual.iter().zip(&expected).enumerate() {
                    assert_eq!(
                        a, e,
                        "cost={cost:?} {background} faded={faded} byte={index}"
                    );
                }
            }
        }
    }

    #[test]
    fn recurring_caret_phases_reuse_two_scenes_and_clear_releases_them() {
        let module = crate::session::tests::source(
            "function render(){return h('input',{value:'Hello',autoFocus:true})}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let visible = page.layout(300, 100, 1.0).unwrap();
        let mut hidden = page.prepare_native(300, 100, 1.0, false, None, Ok).unwrap();
        hidden.hide_caret();
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        assert!(!cache.prepare(&visible, &mut glyphs).unwrap());
        assert!(!cache.prepare(&hidden, &mut glyphs).unwrap());
        for _ in 0..4 {
            assert!(cache.prepare(&visible, &mut glyphs).unwrap());
            assert!(cache.prepare(&hidden, &mut glyphs).unwrap());
        }
        let changed = page.layout(320, 100, 1.0).unwrap();
        assert!(!cache.prepare(&changed, &mut glyphs).unwrap());
        assert!(
            !cache.prepare(&visible, &mut glyphs).unwrap(),
            "third scene evicts the oldest"
        );
        cache.clear();
        assert!(cache.previous.is_none());
        assert!(cache.current.key.is_none());
        assert!(!cache.prepare(&visible, &mut glyphs).unwrap());
    }

    #[test]
    fn oversized_viewports_bypass_and_discard_cached_scenes() {
        let module =
            crate::session::tests::source("function render(){return h('text',{},'Hello')}");
        let mut page = crate::ViewSession::new(&module).unwrap();
        let small = page.layout(200, 100, 1.0).unwrap();
        let large = page.layout(4096, 2048, 1.0).unwrap();
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        assert!(!cache.prepare(&small, &mut glyphs).unwrap());
        assert!(cache.prepare(&small, &mut glyphs).unwrap());
        for _ in 0..2 {
            assert!(!cache.prepare(&large, &mut glyphs).unwrap());
            assert!(cache.previous.is_none());
            assert!(cache.current.key.is_none());
        }
        assert!(!cache.prepare(&small, &mut glyphs).unwrap());
        assert!(cache.previous.is_none());
    }

    #[test]
    fn selection_changes_invalidate_scene_even_when_text_is_unchanged() {
        let module = crate::session::tests::source(
            "function render(){return h('input',{value:'Hello',autoFocus:true})}",
        );
        let mut page = crate::ViewSession::new(&module).unwrap();
        let frame = page.layout(300, 100, 1.0).unwrap();
        let id = frame.nodes[0].identity.clone();
        let token = frame.nodes[0].input.as_ref().unwrap().edit_token.unwrap();
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        assert!(!cache.prepare(&frame, &mut glyphs).unwrap());
        for anchor in [0, 1] {
            page.edit_input(
                token,
                &id,
                &[crate::text::input::Edit::Selection { anchor, focus: 3 }],
                false,
            )
            .unwrap();
            let selected = page.prepare_native(300, 100, 1.0, false, None, Ok).unwrap();
            assert!(!cache.prepare(&selected, &mut glyphs).unwrap());
            assert!(cache.prepare(&selected, &mut glyphs).unwrap());
        }
    }
}
