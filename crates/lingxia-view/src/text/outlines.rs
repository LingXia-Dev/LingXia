//! Glyphs whose outlines the font data does not carry (Apple's hvgl faces): shaped from the
//! font's own tables like any face, outlined by the platform and filled as cached paths.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use vello_cpu::Glyph;
use vello_cpu::kurbo::{Affine, BezPath, Shape};

pub use parley::{FontData, fontique::Blob};

/// A platform's glyph outlines for faces it can draw and the font data cannot.
pub trait GlyphOutlines: Send + Sync {
    /// Glyph `gid` of `font` in font units with y up, at the normalized variation `coords`
    /// (F2Dot14 bits); `None` when there is no outline for it. `file` is where the face was
    /// registered from, when it was.
    fn outline(
        &self,
        font: &FontData,
        file: Option<&std::path::Path>,
        gid: u16,
        coords: &[i16],
    ) -> Option<BezPath>;
}

fn files() -> std::sync::MutexGuard<'static, HashMap<u64, std::path::PathBuf>> {
    static FILES: OnceLock<Mutex<HashMap<u64, std::path::PathBuf>>> = OnceLock::new();
    FILES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// The file blob `id` was read from, for a platform that opens faces by file.
pub(crate) fn note_file(id: u64, path: &std::path::Path) {
    files().insert(id, path.to_owned());
}

static PROVIDER: OnceLock<Box<dyn GlyphOutlines>> = OnceLock::new();

/// Installs the platform's outlines; the first call wins.
pub fn set_glyph_outlines(provider: Box<dyn GlyphOutlines>) {
    let _ = PROVIDER.set(provider);
}

/// Glyph paths kept per face; past it the least recently used quarter goes.
const FACE_GLYPHS: usize = 2048;

type FaceKey = (u64, u32);

struct Face {
    /// No glyf, CFF or CFF2: only the platform can outline it.
    external: bool,
    units_per_em: f64,
    glyphs: HashMap<(u16, Vec<i16>), (Arc<BezPath>, u64)>,
    tick: u64,
}

fn faces() -> std::sync::MutexGuard<'static, HashMap<FaceKey, Face>> {
    static FACES: OnceLock<Mutex<HashMap<FaceKey, Face>>> = OnceLock::new();
    FACES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

fn with_face<T>(font: &parley::FontData, work: impl FnOnce(&mut Face) -> T) -> T {
    let mut faces = faces();
    let face = faces
        .entry((font.data.id(), font.index))
        .or_insert_with(|| describe(font));
    work(face)
}

fn describe(font: &parley::FontData) -> Face {
    use skrifa::raw::{TableProvider, types::Tag};
    let parsed = skrifa::FontRef::from_index(font.data.as_ref(), font.index).ok();
    let has = |tag: &[u8; 4]| {
        parsed
            .as_ref()
            .is_some_and(|f| f.table_data(Tag::new(tag)).is_some())
    };
    Face {
        external: !has(b"glyf")
            && !has(b"CFF ")
            && !has(b"CFF2")
            && (has(b"hvgl") || has(&crate::text::fonts::HIDDEN_HVGL)),
        units_per_em: parsed
            .as_ref()
            .and_then(|f| f.head().ok())
            .map_or(1000.0, |head| f64::from(head.units_per_em())),
        glyphs: HashMap::new(),
        tick: 0,
    }
}

/// Whether `font`'s glyphs are drawn from platform outlines.
pub(crate) fn external(font: &parley::FontData) -> bool {
    with_face(font, |face| face.external)
}

fn outline(font: &parley::FontData, gid: u16, coords: &[i16]) -> Option<(Arc<BezPath>, f64)> {
    let provider = PROVIDER.get()?;
    let cached = with_face(font, |face| {
        face.tick += 1;
        let tick = face.tick;
        let units = face.units_per_em;
        face.glyphs.get_mut(&(gid, coords.to_vec())).map(|entry| {
            entry.1 = tick;
            (entry.0.clone(), units)
        })
    });
    if cached.is_some() {
        return cached;
    }
    let file = files().get(&font.data.id()).cloned();
    let path = Arc::new(provider.outline(font, file.as_deref(), gid, coords)?);
    Some(with_face(font, |face| {
        if face.glyphs.len() >= FACE_GLYPHS {
            let mut ticks: Vec<u64> = face.glyphs.values().map(|(_, t)| *t).collect();
            ticks.sort_unstable();
            let cut = ticks[FACE_GLYPHS / 4];
            face.glyphs.retain(|_, (_, t)| *t > cut);
        }
        face.glyphs
            .insert((gid, coords.to_vec()), (path.clone(), face.tick));
        (path, face.units_per_em)
    }))
}

/// Font units to the run's pixels for one glyph: its origin, the synthesized slant, the size.
fn placement(glyph: &Glyph, size: f32, units_per_em: f64, skew: f64) -> Affine {
    let scale = f64::from(size) / units_per_em;
    Affine::translate((f64::from(glyph.x), f64::from(glyph.y)))
        * Affine::new([1.0, 0.0, -skew, 1.0, 0.0, 0.0])
        * Affine::scale_non_uniform(scale, -scale)
}

/// The run's glyphs as one path in run coordinates; `None` without platform outlines.
pub(crate) fn run_path(
    font: &parley::FontData,
    size: f32,
    coords: &[i16],
    skew: f64,
    glyphs: &[Glyph],
) -> Option<BezPath> {
    let mut run = BezPath::new();
    for glyph in glyphs {
        let Ok(gid) = u16::try_from(glyph.id) else {
            continue;
        };
        let Some((path, units)) = outline(font, gid, coords) else {
            continue;
        };
        let placed = placement(glyph, size, units, skew) * path.as_ref().clone();
        run.extend(placed.elements().iter().copied());
    }
    (!run.elements().is_empty()).then_some(run)
}

/// Ink bounds of the run, `[x0, y0, x1, y1]`, from the outlines.
pub(crate) fn run_ink(
    font: &parley::FontData,
    size: f32,
    coords: &[i16],
    skew: f64,
    glyphs: &[Glyph],
) -> Option<[f32; 4]> {
    let bounds = run_path(font, size, coords, skew, glyphs)?.bounding_box();
    Some([
        bounds.x0 as f32,
        bounds.y0 as f32,
        bounds.x1 as f32,
        bounds.y1 as f32,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_lands_at_its_origin_scaled_flipped_and_slanted() {
        let glyph = Glyph {
            id: 1,
            x: 10.0,
            y: 20.0,
        };
        let place = placement(&glyph, 16.0, 1000.0, 0.25);
        // The baseline origin stays put; a point one em up lands one em above, slanted right.
        let origin = place * vello_cpu::kurbo::Point::new(0.0, 0.0);
        let top = place * vello_cpu::kurbo::Point::new(0.0, 1000.0);
        assert_eq!((origin.x, origin.y), (10.0, 20.0));
        assert!((top.y - 4.0).abs() < 1e-9 && (top.x - 14.0).abs() < 1e-9);
    }
}
