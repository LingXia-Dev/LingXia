//! DirectWrite: the fonts, the shaping, and glyph rasterization.
//!
//! Shaping goes through `IDWriteTextLayout` rather than the analyzer, because
//! a layout applies the font's own feature set — `calt`, which is what turns
//! `!=` into one glyph — and reports the result through one callback interface
//! instead of three.
//!
//! Runs are shaped once and kept: a terminal repeats itself relentlessly, so
//! the same prompt, path and command would otherwise be re-shaped every frame.

use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_COLOR_GLYPH_RUN, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_FEATURE,
    DWRITE_FONT_FEATURE_TAG_CONTEXTUAL_ALTERNATES, DWRITE_FONT_FEATURE_TAG_STANDARD_LIGATURES,
    DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT, DWRITE_FONT_WEIGHT_BOLD,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_GLYPH_OFFSET, DWRITE_GLYPH_RUN, DWRITE_GLYPH_RUN_DESCRIPTION,
    DWRITE_MATRIX, DWRITE_MEASURING_MODE, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_RENDERING_MODE_NATURAL, DWRITE_STRIKETHROUGH, DWRITE_TEXT_RANGE,
    DWRITE_TEXTURE_CLEARTYPE_3x1, DWRITE_UNDERLINE, DWRITE_WORD_WRAPPING_NO_WRAP,
    DWriteCreateFactory, IDWriteColorGlyphRunEnumerator, IDWriteFactory, IDWriteFactory2,
    IDWriteFontCollection, IDWriteFontFace, IDWriteInlineObject, IDWritePixelSnapping_Impl,
    IDWriteTextFormat, IDWriteTextRenderer, IDWriteTextRenderer_Impl, IDWriteTypography,
};
use windows::core::{BOOL, HSTRING, IUnknown, Interface, PCWSTR, Ref, Result, implement, w};

/// Style variants a cell can ask for, as an index into the face table.
pub(super) const REGULAR: usize = 0;
pub(super) const BOLD: usize = 1;
pub(super) const ITALIC: usize = 2;
pub(super) const BOLD_ITALIC: usize = 3;

/// One glyph as the shaper placed it, in pixels from the run's origin.
#[derive(Clone, Copy)]
pub(super) struct ShapedGlyph {
    pub(super) index: u16,
    pub(super) x: f32,
}

/// A glyph bitmap waiting to be uploaded: premultiplied BGRA, tightly packed.
///
/// A plain glyph fills only the alpha channel and takes the run's color at
/// draw time; a color glyph — an emoji — carries its own and is drawn as it is.
pub(super) struct Rasterized {
    pub(super) width: u32,
    pub(super) height: u32,
    /// Offset from the pen position to the sprite's top-left, in pixels.
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) colored: bool,
    pub(super) pixels: Vec<u8>,
}

/// Cell metrics the grid is laid out on.
#[derive(Clone, Copy)]
pub(super) struct Metrics {
    pub(super) cell_width: f32,
    pub(super) line_height: f32,
    pub(super) baseline: f32,
    pub(super) underline_offset: f32,
    pub(super) underline_thickness: f32,
    pub(super) strike_offset: f32,
}

pub(super) struct Fonts {
    factory: IDWriteFactory,
    collection: IDWriteFontCollection,
    family: String,
    size: f32,
    ligatures: bool,
    formats: [IDWriteTextFormat; 4],
    faces: [IDWriteFontFace; 4],
    /// `calt`/`liga` switched off, for the "no ligatures" case.
    plain: Option<IDWriteTypography>,
    pub(super) metrics: Metrics,
    shaped: HashMap<(usize, String), Vec<ShapedGlyph>>,
}

impl Fonts {
    pub(super) fn new(candidates: &[String], size: f32, ligatures: bool) -> Result<Self> {
        let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        let collection = system_collection(&factory)?;
        let family = resolve_family(&collection, candidates)?;
        Self::build(factory, collection, family, size, ligatures)
    }

    /// Adopt a changed font configuration, reusing the factory and collection.
    pub(super) fn reload(
        &mut self,
        candidates: &[String],
        size: f32,
        ligatures: bool,
    ) -> Result<bool> {
        let family = resolve_family(&self.collection, candidates)?;
        if family == self.family && size == self.size && ligatures == self.ligatures {
            return Ok(false);
        }
        *self = Self::build(
            self.factory.clone(),
            self.collection.clone(),
            family,
            size,
            ligatures,
        )?;
        Ok(true)
    }

    fn build(
        factory: IDWriteFactory,
        collection: IDWriteFontCollection,
        family: String,
        size: f32,
        ligatures: bool,
    ) -> Result<Self> {
        let name = HSTRING::from(&family);
        let styles = [
            (DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL),
            (DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_STYLE_NORMAL),
            (DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_ITALIC),
            (DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_STYLE_ITALIC),
        ];
        let mut formats = Vec::with_capacity(4);
        let mut faces = Vec::with_capacity(4);
        for (weight, style) in styles {
            formats.push(unsafe {
                factory.CreateTextFormat(
                    PCWSTR(name.as_ptr()),
                    Some(&collection),
                    weight,
                    style,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size,
                    w!(""),
                )?
            });
            faces.push(face_for(&collection, &name, weight, style)?);
        }
        let formats: [IDWriteTextFormat; 4] = formats.try_into().map_err(|_| missing())?;
        let faces: [IDWriteFontFace; 4] = faces.try_into().map_err(|_| missing())?;
        let metrics = measure(&faces[REGULAR], size);
        let plain = if ligatures {
            None
        } else {
            Some(plain_typography(&factory)?)
        };
        Ok(Self {
            factory,
            collection,
            family,
            size,
            ligatures,
            formats,
            faces,
            plain,
            metrics,
            shaped: HashMap::new(),
        })
    }

    pub(super) fn family(&self) -> &str {
        &self.family
    }

    /// Shape one run of same-styled text, cached by its text and style.
    pub(super) fn shape(&mut self, text: &str, style: usize) -> &[ShapedGlyph] {
        let key = (style, text.to_string());
        if !self.shaped.contains_key(&key) {
            let width = self.metrics.cell_width * text.chars().count().max(1) as f32;
            let glyphs = self.shape_run(text, style, width).unwrap_or_else(|error| {
                log::warn!("terminal shaping failed for {text:?}: {error}");
                Vec::new()
            });
            self.shaped.insert(key.clone(), glyphs);
        }
        &self.shaped[&key]
    }

    fn shape_run(&self, text: &str, style: usize, width: f32) -> Result<Vec<ShapedGlyph>> {
        let utf16: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let layout = self.factory.CreateTextLayout(
                &utf16,
                &self.formats[style],
                width.max(1.0),
                self.metrics.line_height.max(1.0),
            )?;
            // A terminal run is one line by definition. Left to wrap, the
            // layout restarts the pen at x=0 on the next line and the run
            // draws on top of itself.
            layout.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            if let Some(plain) = &self.plain {
                layout.SetTypography(
                    plain,
                    DWRITE_TEXT_RANGE {
                        startPosition: 0,
                        length: utf16.len() as u32,
                    },
                )?;
            }
            let collector = windows::core::ComObject::new(GlyphCollector::default());
            layout.Draw(
                None,
                &collector.to_interface::<IDWriteTextRenderer>(),
                0.0,
                0.0,
            )?;
            Ok(collector.collected())
        }
    }

    /// Rasterize one glyph to 8-bit coverage, with its offset from the pen.
    /// Rasterize one glyph, with its offset from the pen.
    ///
    /// A color glyph — an emoji — is a stack of colored layers rather than one
    /// coverage mask, so it is composited here and drawn untinted.
    pub(super) fn rasterize(&self, glyph: u16, style: usize) -> Result<Option<Rasterized>> {
        let run = self.glyph_run(glyph, style);
        if let Some(colored) = self.rasterize_color(&run)? {
            return Ok(Some(colored));
        }
        let Some((bounds, coverage)) = self.coverage(&run)? else {
            return Ok(None);
        };
        let width = (bounds.right - bounds.left) as u32;
        let height = (bounds.bottom - bounds.top) as u32;
        // Alpha only: the run's color arrives at draw time.
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for (texel, alpha) in pixels.chunks_exact_mut(4).zip(coverage) {
            texel[3] = alpha;
        }
        Ok(Some(Rasterized {
            width,
            height,
            left: bounds.left,
            top: bounds.top,
            colored: false,
            pixels,
        }))
    }

    fn glyph_run(&self, glyph: u16, style: usize) -> GlyphRun {
        GlyphRun {
            glyph,
            advance: 0.0,
            offset: DWRITE_GLYPH_OFFSET::default(),
            face: self.faces[style].clone(),
            size: self.size,
        }
    }

    /// Grayscale coverage for a run, and the bounds it occupies.
    ///
    /// ClearType, not aliased: `ALIASED_1x1` is only valid for the aliased
    /// rendering mode and reports empty bounds otherwise. The three subpixel
    /// channels collapse to one value, since the grid is drawn with grayscale
    /// antialiasing.
    fn coverage(&self, run: &GlyphRun) -> Result<Option<(RECT, Vec<u8>)>> {
        unsafe {
            let analysis = self.factory.CreateGlyphRunAnalysis(
                &run.as_dwrite(),
                1.0,
                None,
                DWRITE_RENDERING_MODE_NATURAL,
                DWRITE_MEASURING_MODE_NATURAL,
                0.0,
                0.0,
            )?;
            let bounds = analysis.GetAlphaTextureBounds(DWRITE_TEXTURE_CLEARTYPE_3x1)?;
            let width = (bounds.right - bounds.left).max(0) as u32;
            let height = (bounds.bottom - bounds.top).max(0) as u32;
            if width == 0 || height == 0 {
                return Ok(None);
            }
            let mut subpixels = vec![0u8; (width * height * 3) as usize];
            analysis.CreateAlphaTexture(DWRITE_TEXTURE_CLEARTYPE_3x1, &bounds, &mut subpixels)?;
            let coverage = subpixels
                .chunks_exact(3)
                .map(|rgb| ((u32::from(rgb[0]) + u32::from(rgb[1]) + u32::from(rgb[2])) / 3) as u8)
                .collect();
            Ok(Some((bounds, coverage)))
        }
    }

    /// Composite a color glyph's layers, or `None` when the glyph has none.
    fn rasterize_color(&self, run: &GlyphRun) -> Result<Option<Rasterized>> {
        let Some(factory) = self.factory.cast::<IDWriteFactory2>().ok() else {
            return Ok(None);
        };
        let layers = unsafe {
            match factory.TranslateColorGlyphRun(
                0.0,
                0.0,
                &run.as_dwrite(),
                None,
                DWRITE_MEASURING_MODE_NATURAL,
                None,
                0,
            ) {
                Ok(layers) => layers,
                // The documented "this glyph is not colored" answer.
                Err(_) => return Ok(None),
            }
        };

        // Two passes: the first finds the union of the layers' bounds, the
        // second draws into a buffer that size. A layer's extent is not known
        // until it is analyzed, and they do not all share one.
        let mut rendered: Vec<(RECT, Vec<u8>, [f32; 3])> = Vec::new();
        let mut union: Option<RECT> = None;
        loop {
            let color = unsafe { &*factory_current_run(&layers)? };
            let tint = [color.runColor.r, color.runColor.g, color.runColor.b];
            let layer = GlyphRun::from_dwrite(&color.glyphRun, run);
            if let Some((bounds, coverage)) = self.coverage(&layer)? {
                union = Some(match union {
                    Some(previous) => RECT {
                        left: previous.left.min(bounds.left),
                        top: previous.top.min(bounds.top),
                        right: previous.right.max(bounds.right),
                        bottom: previous.bottom.max(bounds.bottom),
                    },
                    None => bounds,
                });
                rendered.push((bounds, coverage, tint));
            }
            if !unsafe { layers.MoveNext()?.as_bool() } {
                break;
            }
        }

        let Some(union) = union else {
            return Ok(None);
        };
        let width = (union.right - union.left) as u32;
        let height = (union.bottom - union.top) as u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for (bounds, coverage, tint) in rendered {
            let layer_width = (bounds.right - bounds.left) as u32;
            for (index, alpha) in coverage.into_iter().enumerate() {
                if alpha == 0 {
                    continue;
                }
                let x = (bounds.left - union.left) as u32 + index as u32 % layer_width;
                let y = (bounds.top - union.top) as u32 + index as u32 / layer_width;
                let target = ((y * width + x) * 4) as usize;
                let alpha = f32::from(alpha) / 255.0;
                // Premultiplied source-over, so layers stack the way the font
                // intends without a second blend at draw time.
                let texel = &mut pixels[target..target + 4];
                for (channel, value) in [tint[2], tint[1], tint[0]].into_iter().enumerate() {
                    texel[channel] = blend_channel(texel[channel], value * alpha, alpha);
                }
                texel[3] = blend_channel(texel[3], alpha, alpha);
            }
        }
        Ok(Some(Rasterized {
            width,
            height,
            left: union.left,
            top: union.top,
            colored: true,
            pixels,
        }))
    }
}

/// A glyph run owned by us, so it can be handed to DirectWrite twice — once
/// to ask whether it is colored, once to rasterize each layer.
struct GlyphRun {
    glyph: u16,
    advance: f32,
    offset: DWRITE_GLYPH_OFFSET,
    face: IDWriteFontFace,
    size: f32,
}

impl GlyphRun {
    fn as_dwrite(&self) -> DWRITE_GLYPH_RUN {
        DWRITE_GLYPH_RUN {
            fontFace: unsafe { std::mem::transmute_copy(&self.face) },
            fontEmSize: self.size,
            glyphCount: 1,
            glyphIndices: &self.glyph,
            glyphAdvances: &self.advance,
            glyphOffsets: &self.offset,
            isSideways: false.into(),
            bidiLevel: 0,
        }
    }

    /// One glyph of a color layer, keeping the face the layer names.
    fn from_dwrite(run: &DWRITE_GLYPH_RUN, fallback: &GlyphRun) -> Self {
        let glyph = if run.glyphIndices.is_null() || run.glyphCount == 0 {
            fallback.glyph
        } else {
            unsafe { *run.glyphIndices }
        };
        let face = unsafe { (*run.fontFace).clone() }.unwrap_or_else(|| fallback.face.clone());
        Self {
            glyph,
            advance: 0.0,
            offset: DWRITE_GLYPH_OFFSET::default(),
            face,
            size: run.fontEmSize,
        }
    }
}

/// Source-over on one premultiplied channel, in 8-bit.
fn blend_channel(destination: u8, source: f32, alpha: f32) -> u8 {
    let destination = f32::from(destination) / 255.0;
    ((source + destination * (1.0 - alpha)).clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The enumerator's current layer. Wrapped because the raw call is the only
/// place a null would reach the compositing loop.
fn factory_current_run(
    layers: &IDWriteColorGlyphRunEnumerator,
) -> Result<*const DWRITE_COLOR_GLYPH_RUN> {
    let run = unsafe { layers.GetCurrentRun()? };
    if run.is_null() {
        Err(missing())
    } else {
        Ok(run)
    }
}

fn missing() -> windows::core::Error {
    windows::core::Error::from_thread()
}

fn system_collection(factory: &IDWriteFactory) -> Result<IDWriteFontCollection> {
    let mut collection = None;
    unsafe { factory.GetSystemFontCollection(&mut collection, false)? };
    collection.ok_or_else(missing)
}

/// The first installed candidate.
///
/// A family that is not installed is skipped rather than substituted:
/// DirectWrite would otherwise hand back something else silently, which is
/// the failure a user cannot see.
fn resolve_family(collection: &IDWriteFontCollection, candidates: &[String]) -> Result<String> {
    candidates
        .iter()
        .map(String::as_str)
        .chain(["Cascadia Mono", "Consolas", "Courier New"])
        .find(|name| family_installed(collection, name))
        .map(str::to_string)
        .ok_or_else(missing)
}

fn family_installed(collection: &IDWriteFontCollection, name: &str) -> bool {
    let name = HSTRING::from(name);
    let mut index = 0u32;
    let mut exists = BOOL(0);
    unsafe {
        collection
            .FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists)
            .is_ok()
            && exists.as_bool()
    }
}

fn face_for(
    collection: &IDWriteFontCollection,
    name: &HSTRING,
    weight: DWRITE_FONT_WEIGHT,
    style: DWRITE_FONT_STYLE,
) -> Result<IDWriteFontFace> {
    let mut index = 0u32;
    let mut exists = BOOL(0);
    unsafe {
        collection.FindFamilyName(PCWSTR(name.as_ptr()), &mut index, &mut exists)?;
        if !exists.as_bool() {
            return Err(missing());
        }
        collection
            .GetFontFamily(index)?
            .GetFirstMatchingFont(weight, DWRITE_FONT_STRETCH_NORMAL, style)?
            .CreateFontFace()
    }
}

/// Cell metrics from the face's own design metrics.
///
/// The cell width is `M`'s advance: a monospace face gives every glyph the
/// same advance, and taking it from a real glyph rather than from a layout
/// keeps the grid independent of any shaping decision.
fn measure(face: &IDWriteFontFace, size: f32) -> Metrics {
    unsafe {
        let mut font = Default::default();
        face.GetMetrics(&mut font);
        let scale = size / f32::from(font.designUnitsPerEm).max(1.0);
        let mut glyph = [0u16; 1];
        let _ = face.GetGlyphIndices(&('M' as u32), 1, glyph.as_mut_ptr());
        let mut glyph_metrics = [Default::default(); 1];
        let advance = face
            .GetDesignGlyphMetrics(glyph.as_ptr(), 1, glyph_metrics.as_mut_ptr(), false)
            .map(|()| glyph_metrics[0].advanceWidth as f32)
            .unwrap_or_else(|_| f32::from(font.designUnitsPerEm) * 0.6);

        let ascent = f32::from(font.ascent) * scale;
        let descent = f32::from(font.descent) * scale;
        let gap = f32::from(font.lineGap) * scale;
        Metrics {
            cell_width: (advance * scale).max(1.0).round(),
            line_height: (ascent + descent + gap).max(1.0).round(),
            baseline: ascent.round(),
            underline_offset: -f32::from(font.underlinePosition) * scale,
            underline_thickness: (f32::from(font.underlineThickness) * scale).max(1.0),
            strike_offset: -f32::from(font.strikethroughPosition) * scale,
        }
    }
}

/// Turning the features off is what disables ligatures; a layout's own
/// "ligature" flags never reach `calt`, which is the feature that draws them.
fn plain_typography(factory: &IDWriteFactory) -> Result<IDWriteTypography> {
    unsafe {
        let typography = factory.CreateTypography()?;
        for tag in [
            DWRITE_FONT_FEATURE_TAG_CONTEXTUAL_ALTERNATES,
            DWRITE_FONT_FEATURE_TAG_STANDARD_LIGATURES,
        ] {
            typography.AddFontFeature(DWRITE_FONT_FEATURE {
                nameTag: tag,
                parameter: 0,
            })?;
        }
        Ok(typography)
    }
}

/// Collects the glyph runs an `IDWriteTextLayout` draws — the only way to see
/// what the font's features did to the text.
#[implement(IDWriteTextRenderer)]
#[derive(Default)]
struct GlyphCollector {
    glyphs: RefCell<Vec<ShapedGlyph>>,
}

impl GlyphCollector {
    fn collected(&self) -> Vec<ShapedGlyph> {
        std::mem::take(&mut self.glyphs.borrow_mut())
    }
}

impl IDWritePixelSnapping_Impl for GlyphCollector_Impl {
    fn IsPixelSnappingDisabled(&self, _context: *const core::ffi::c_void) -> Result<BOOL> {
        Ok(BOOL(1))
    }

    fn GetCurrentTransform(
        &self,
        _context: *const core::ffi::c_void,
        transform: *mut DWRITE_MATRIX,
    ) -> Result<()> {
        unsafe {
            *transform = DWRITE_MATRIX {
                m11: 1.0,
                m22: 1.0,
                ..Default::default()
            };
        }
        Ok(())
    }

    fn GetPixelsPerDip(&self, _context: *const core::ffi::c_void) -> Result<f32> {
        Ok(1.0)
    }
}

impl IDWriteTextRenderer_Impl for GlyphCollector_Impl {
    fn DrawGlyphRun(
        &self,
        _context: *const core::ffi::c_void,
        origin_x: f32,
        _origin_y: f32,
        _measuring: DWRITE_MEASURING_MODE,
        run: *const DWRITE_GLYPH_RUN,
        _description: *const DWRITE_GLYPH_RUN_DESCRIPTION,
        _effect: Ref<'_, IUnknown>,
    ) -> Result<()> {
        let run = unsafe { &*run };
        let count = run.glyphCount as usize;
        if count == 0 || run.glyphIndices.is_null() {
            return Ok(());
        }
        let indices = unsafe { std::slice::from_raw_parts(run.glyphIndices, count) };
        let advances = (!run.glyphAdvances.is_null())
            .then(|| unsafe { std::slice::from_raw_parts(run.glyphAdvances, count) });
        let mut pen = origin_x;
        let mut glyphs = self.glyphs.borrow_mut();
        for (position, index) in indices.iter().enumerate() {
            glyphs.push(ShapedGlyph {
                index: *index,
                x: pen,
            });
            pen += advances.map_or(0.0, |advances| advances[position]);
        }
        Ok(())
    }

    fn DrawUnderline(
        &self,
        _context: *const core::ffi::c_void,
        _x: f32,
        _y: f32,
        _underline: *const DWRITE_UNDERLINE,
        _effect: Ref<'_, IUnknown>,
    ) -> Result<()> {
        Ok(())
    }

    fn DrawStrikethrough(
        &self,
        _context: *const core::ffi::c_void,
        _x: f32,
        _y: f32,
        _strikethrough: *const DWRITE_STRIKETHROUGH,
        _effect: Ref<'_, IUnknown>,
    ) -> Result<()> {
        Ok(())
    }

    fn DrawInlineObject(
        &self,
        _context: *const core::ffi::c_void,
        _x: f32,
        _y: f32,
        _object: Ref<'_, IDWriteInlineObject>,
        _sideways: BOOL,
        _right_to_left: BOOL,
        _effect: Ref<'_, IUnknown>,
    ) -> Result<()> {
        Ok(())
    }
}
