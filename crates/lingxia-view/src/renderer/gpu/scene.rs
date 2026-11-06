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

/// Each pass reloads the full target, so many spans cost more than one full encode.
const MAX_SPANS: usize = 8;

/// Static commands that cost less to encode again each frame than a pass of their own.
fn cheap(commands: &[Paint]) -> bool {
    let glyphs: usize = commands
        .iter()
        .map(|paint| match paint {
            Paint::Text { glyphs, .. } => glyphs.len(),
            _ => 0,
        })
        .sum();
    commands.len() <= 24 && glyphs <= 48
}

/// Split a frame at its animated spans so commands outside them keep their scenes. A
/// boundary never sits inside an opacity or backdrop group: a span inside one widens to
/// the outermost group, since a group's pixels only combine correctly in one scene.
fn segments(display: &DisplayList) -> Option<Vec<Segment>> {
    let spans = display.animated_spans();
    if spans.is_empty() || spans.len() > MAX_SPANS || !SceneKey::eligible(display, 2048, 32768) {
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
        match ranges.last_mut() {
            Some(last) if range.start <= last.end || cheap(&commands[last.end..range.start]) => {
                last.end = last.end.max(range.end)
            }
            _ if range.is_empty() => {}
            _ => ranges.push(range),
        }
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
}

static BASES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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
        }
    }
}

impl SceneCache {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// `true` when no scene was encoded again, or while animating, no static segment was.
    pub(crate) fn prepare(
        &mut self,
        display: &DisplayList,
        glyphs: &mut crate::renderer::gpu::text::Glyphs<'_>,
    ) -> Result<bool, String> {
        if let Some(plan) = segments(display) {
            return self.prepare_segments(display, &plan, glyphs);
        }
        self.segmented = false;
        self.frame = None;
        self.segments.clear();
        self.base = None;
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
    ) -> Result<bool, String> {
        self.segmented = true;
        let frame = FrameKey::of(display);
        if self.frame.as_ref() != Some(&frame) {
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
            if next[index].is_some() {
                continue;
            }
            let mut entry = spare.next().unwrap_or_else(|| SegmentEntry {
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
        Ok(hit)
    }

    /// While animating over a static bottom segment: an id that changes whenever that
    /// segment's pixels may change, so its rendering can be kept between frames.
    pub(crate) fn base(&self) -> Option<u64> {
        self.base.filter(|_| self.segmented)
    }

    /// Scenes to draw in order; the first clears the target.
    pub(crate) fn scenes(&self) -> Box<dyn Iterator<Item = &vello_gpu::Scene> + '_> {
        if self.segmented {
            Box::new(self.segments.iter().map(|entry| &entry.entry.scene))
        } else {
            Box::new(std::iter::once(&self.current.scene))
        }
    }

    pub(crate) fn glyph_images(&self) -> Box<dyn Iterator<Item = &Arc<vello_gpu::Pixmap>> + '_> {
        if self.segmented {
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
        let opacity = if faded { ",opacity:0.8" } else { "" };
        let module = crate::session::tests::source(&format!(
            "function render(){{return h('column',{{style:{{width:96,height:96,backgroundColor:'#ffffff'}}}},h('text',{{}},'Before'),h('column',{{style:{{width:80,height:56,borderRadius:8,overflow:'hidden',backgroundColor:'#eeeeee'{opacity}}}}},h('column',{{'data-testid':'spin',style:{{width:30,height:30,backgroundColor:'#00ff00'}}}},h('text',{{}},'Go')),h('input',{{value:'Hi',autoFocus:true}}),h('column',{{style:{{height:10,backgroundColor:'#000000'}}}})),h('text',{{}},'After'))}}"
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
    fn animation_frames_reencode_only_their_spans() {
        let (_page, display) = spin_page(false);
        let mut cache = SceneCache::default();
        let mut prep = glifo::GlyphPrepCache::default();
        let mut glyphs = crate::renderer::gpu::text::Glyphs {
            prep: &mut prep,
            atlas: None,
        };
        assert!(!cache.prepare(&spun(&display, 0.0), &mut glyphs).unwrap());
        let plan = segments(&spun(&display, 0.0)).unwrap();
        assert_eq!(
            plan.iter().map(|s| s.animated).collect::<Vec<_>>(),
            [false, true, false]
        );
        assert_eq!(
            plan[1].clips.len(),
            1,
            "the span opens inside its parent's clip"
        );
        assert_eq!(plan[0].open_at_end, 1);
        assert_eq!(plan[2].open_at_end, 0);
        let base = cache.base();
        assert!(base.is_some());
        for angle in [10.0, 20.0, 30.0] {
            assert!(cache.prepare(&spun(&display, angle), &mut glyphs).unwrap());
            assert_eq!(cache.scenes().count(), 3);
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
        assert!(!cache.prepare(&hidden, &mut glyphs).unwrap());
        assert!(cache.prepare(&hidden, &mut glyphs).unwrap());
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
            "function render(){return h('column',{style:{width:96,height:96}},h('text',{},'Before'),h('column',{'data-testid':'a',style:{width:10,height:10,backgroundColor:'#00ff00'}}),h('column',{style:{width:10,height:10,backgroundColor:'#000000'}}),h('column',{'data-testid':'b',style:{width:10,height:10,backgroundColor:'#0000ff'}}),h('text',{},'After'))}",
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
        let plan = segments(&frame).unwrap();
        assert_eq!(
            plan.iter().map(|s| s.animated).collect::<Vec<_>>(),
            [false, true, false]
        );
    }

    #[test]
    fn a_span_inside_a_group_widens_to_the_group() {
        let (_page, display) = spin_page(true);
        let frame = spun(&display, 15.0);
        let plan = segments(&frame).unwrap();
        let animated: Vec<_> = plan.iter().filter(|s| s.animated).collect();
        assert_eq!(animated.len(), 1);
        let span = &frame.animated_spans()[0].commands;
        let range = &animated[0].range;
        assert!(range.start < span.start && span.end < range.end);
        assert!(matches!(frame.commands[range.start], Paint::Opacity { .. }));
        assert!(matches!(frame.commands[range.end - 1], Paint::PopOpacity));
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
        let mut draw =
            |scenes: Vec<&vello_gpu::Scene>, images: Vec<Arc<vello_gpu::Pixmap>>| -> Vec<u8> {
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
                let bindings = crate::renderer::gpu::images::ImageTextures::default()
                    .bindings(device, queue, images);
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
                readback(device, queue, &texture)
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
                let actual = draw(
                    cache.scenes().collect(),
                    cache.glyph_images().cloned().collect(),
                );
                let mut whole = Entry::new();
                whole.build(&frame, &mut glyphs).unwrap();
                let expected = draw(
                    vec![&whole.scene],
                    whole.glyph_images.values().cloned().collect(),
                );
                for (index, (a, e)) in actual.iter().zip(&expected).enumerate() {
                    assert!(
                        a.abs_diff(*e) <= 2,
                        "faded={faded} angle={angle} byte={index}: {a} vs {e}"
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
