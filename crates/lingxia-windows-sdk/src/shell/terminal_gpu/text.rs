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

use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_FEATURE, DWRITE_FONT_FEATURE_TAG_CONTEXTUAL_ALTERNATES,
    DWRITE_FONT_FEATURE_TAG_STANDARD_LIGATURES, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE,
    DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_GLYPH_OFFSET, DWRITE_GLYPH_RUN,
    DWRITE_GLYPH_RUN_DESCRIPTION, DWRITE_MATRIX, DWRITE_MEASURING_MODE,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_RENDERING_MODE_NATURAL, DWRITE_STRIKETHROUGH,
    DWRITE_TEXT_RANGE, DWRITE_TEXTURE_CLEARTYPE_3x1, DWRITE_UNDERLINE, DWriteCreateFactory,
    IDWriteFactory, IDWriteFontCollection, IDWriteFontFace, IDWriteInlineObject,
    IDWritePixelSnapping_Impl, IDWriteTextFormat, IDWriteTextRenderer, IDWriteTextRenderer_Impl,
    IDWriteTypography,
};
use windows::core::{BOOL, HSTRING, IUnknown, PCWSTR, Ref, Result, implement, w};

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

/// A glyph bitmap waiting to be uploaded: 8-bit coverage, tightly packed.
pub(super) struct Rasterized {
    pub(super) width: u32,
    pub(super) height: u32,
    /// Offset from the pen position to the sprite's top-left, in pixels.
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) coverage: Vec<u8>,
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
    pub(super) fn rasterize(&self, glyph: u16, style: usize) -> Result<Option<Rasterized>> {
        let advance = 0.0f32;
        let offset = DWRITE_GLYPH_OFFSET::default();
        unsafe {
            let run = DWRITE_GLYPH_RUN {
                fontFace: std::mem::transmute_copy(&self.faces[style]),
                fontEmSize: self.size,
                glyphCount: 1,
                glyphIndices: &glyph,
                glyphAdvances: &advance,
                glyphOffsets: &offset,
                isSideways: false.into(),
                bidiLevel: 0,
            };
            let analysis = self.factory.CreateGlyphRunAnalysis(
                &run,
                1.0,
                None,
                DWRITE_RENDERING_MODE_NATURAL,
                DWRITE_MEASURING_MODE_NATURAL,
                0.0,
                0.0,
            )?;
            // ClearType, not aliased: `ALIASED_1x1` is only valid for the
            // aliased rendering mode and reports empty bounds otherwise. The
            // three subpixel channels collapse to one coverage value, since
            // the grid is drawn with grayscale antialiasing.
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
            Ok(Some(Rasterized {
                width,
                height,
                left: bounds.left,
                top: bounds.top,
                coverage,
            }))
        }
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
