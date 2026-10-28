//! CoreText outlines for faces whose glyphs live only in `hvgl` (PingFang UI): the core
//! shapes them from the font tables and fills these paths.
use lingxia_ui::text::outlines::{FontData, GlyphOutlines};
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType, CFURL};
use objc2_core_graphics::{CGPath, CGPathElement, CGPathElementType};
use objc2_core_text::{
    CTFont, CTFontDescriptor, CTFontManagerCreateFontDescriptorsFromURL, kCTFontVariationAttribute,
};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Mutex;
use vello_cpu::kurbo::BezPath;

/// A CoreText font for one face at one variation, sized to its units per em.
struct Face {
    font: CFRetained<CTFont>,
}

// SAFETY: CoreText fonts are immutable and documented thread-safe.
unsafe impl Send for Face {}
unsafe impl Sync for Face {}

/// Blob id, face index and normalized variation coordinates.
type FaceKey = (u64, u32, Vec<i16>);

#[derive(Default)]
pub(crate) struct CoreTextOutlines {
    faces: Mutex<HashMap<FaceKey, Face>>,
}

impl GlyphOutlines for CoreTextOutlines {
    fn outline(
        &self,
        font: &FontData,
        file: Option<&std::path::Path>,
        gid: u16,
        coords: &[i16],
    ) -> Option<BezPath> {
        let mut faces = self.faces.lock().unwrap_or_else(|p| p.into_inner());
        let key = (font.data.id(), font.index, coords.to_vec());
        if !faces.contains_key(&key) {
            let face = face(font, file?, coords)?;
            faces.insert(key.clone(), face);
        }
        let face = faces.get(&key)?;
        // SAFETY: a valid font and glyph; a null transform is the identity.
        let path = unsafe { face.font.path_for_glyph(gid, std::ptr::null()) }?;
        Some(bez_path(&path))
    }
}

/// CoreText opens these faces by file only: from bytes it lists none of them.
fn face(font: &FontData, file: &std::path::Path, coords: &[i16]) -> Option<Face> {
    let bytes = font.data.as_ref();
    let url = CFURL::from_file_path(file)?;
    let descriptors = unsafe { CTFontManagerCreateFontDescriptorsFromURL(&url) }?;
    // SAFETY: CoreText returns an array of font descriptors.
    let descriptors: CFRetained<CFArray<CTFontDescriptor>> =
        unsafe { CFRetained::cast_unchecked(descriptors) };
    let mut descriptor = descriptors.get(font.index as usize)?;
    let parsed = skrifa::FontRef::from_index(bytes, font.index).ok()?;
    if coords.iter().any(|c| *c != 0) {
        let variations = user_values(&parsed, coords);
        let keys: Vec<CFRetained<CFNumber>> = variations
            .iter()
            .map(|(tag, _)| CFNumber::new_i64(*tag))
            .collect();
        let values: Vec<CFRetained<CFNumber>> = variations
            .iter()
            .map(|(_, v)| CFNumber::new_f64(*v))
            .collect();
        let keys: Vec<&CFNumber> = keys.iter().map(|k| &**k).collect();
        let values: Vec<&CFNumber> = values.iter().map(|v| &**v).collect();
        let variation = CFDictionary::from_slices(&keys, &values);
        let attribute: &CFString = unsafe { kCTFontVariationAttribute };
        let attributes = CFDictionary::<CFString, CFType>::from_slices(
            &[attribute],
            &[variation.as_ref() as &CFType],
        );
        descriptor = unsafe { descriptor.copy_with_attributes(attributes.as_opaque()) };
    }
    let units = {
        use skrifa::raw::TableProvider;
        f64::from(parsed.head().ok()?.units_per_em())
    };
    let font_ref = unsafe { CTFont::with_font_descriptor(&descriptor, units, std::ptr::null()) };
    Some(Face { font: font_ref })
}

/// CoreText takes variations in user units; recover each axis' user value from its
/// normalized coordinate through the font's own mapping (avar included).
fn user_values(font: &skrifa::FontRef, coords: &[i16]) -> Vec<(i64, f64)> {
    use skrifa::MetadataProvider;
    let axes = font.axes();
    axes.iter()
        .enumerate()
        .filter_map(|(i, axis)| {
            let target = *coords.get(i)?;
            let (mut low, mut high) = (axis.min_value(), axis.max_value());
            for _ in 0..24 {
                let mid = (low + high) / 2.0;
                let at = axes.location([(axis.tag(), mid)]).coords()[i].to_bits();
                if at < target {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Some((
                i64::from(u32::from_be_bytes(axis.tag().to_be_bytes())),
                f64::from((low + high) / 2.0),
            ))
        })
        .collect()
}

fn bez_path(path: &CGPath) -> BezPath {
    unsafe extern "C-unwind" fn element(info: *mut c_void, element: NonNull<CGPathElement>) {
        // SAFETY: `info` is the BezPath below; CoreGraphics passes valid elements.
        let path = unsafe { &mut *info.cast::<BezPath>() };
        let element = unsafe { element.as_ref() };
        let point = |i: usize| {
            let p = unsafe { *element.points.as_ptr().add(i) };
            (p.x, p.y)
        };
        match element.r#type {
            CGPathElementType::MoveToPoint => path.move_to(point(0)),
            CGPathElementType::AddLineToPoint => path.line_to(point(0)),
            CGPathElementType::AddQuadCurveToPoint => path.quad_to(point(0), point(1)),
            CGPathElementType::AddCurveToPoint => path.curve_to(point(0), point(1), point(2)),
            CGPathElementType::CloseSubpath => path.close_path(),
            _ => {}
        }
    }
    let mut out = BezPath::new();
    // SAFETY: `out` outlives the synchronous apply.
    unsafe { CGPath::apply(Some(path), (&raw mut out).cast(), Some(element)) };
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pingfang_ui_glyphs_outline_through_core_text() {
        let path = std::path::Path::new(
            "/System/Library/PrivateFrameworks/FontServices.framework/Versions/A/Resources/Reserved/PingFangUI.ttc",
        );
        let Ok(bytes) = std::fs::read(path) else {
            eprintln!("skipped: no PingFang UI on this host");
            return;
        };
        let font = FontData::new(
            lingxia_ui::text::outlines::Blob::new(std::sync::Arc::new(bytes)),
            0,
        );
        let outlines = CoreTextOutlines::default();
        // 首 in .PingFang UI Text SC (the probe's glyph 980).
        let glyph = outlines
            .outline(&font, Some(path), 980, &[])
            .expect("an outline");
        assert!(glyph.elements().len() > 4);
        let weighted = outlines
            .outline(&font, Some(path), 980, &[8192])
            .expect("a weighted outline");
        assert_ne!(glyph, weighted, "the variation reaches CoreText");
        assert!(
            CoreTextOutlines::default()
                .outline(&font, None, 980, &[])
                .is_none(),
            "no file, no face"
        );
    }
}
