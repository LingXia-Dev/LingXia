//! Shaped text extents and glyph ink bounds.
use parley::Layout;
use vello_cpu::Glyph;

pub(crate) fn text_extent(layout: &Layout<()>, lines: Option<usize>) -> (f32, f32) {
    let Some(limit) = lines else {
        return (layout.width(), layout.height());
    };
    let mut width: f32 = 0.0;
    let mut height = 0.0;
    for line in layout.lines().take(limit) {
        let metrics = line.metrics();
        width = width.max(metrics.advance - metrics.hanging_advance);
        height = metrics.block_max_coord;
    }
    (width, height)
}

pub(crate) fn glyph_ink(
    font: &parley::FontData,
    size: f32,
    coordinates: &[i16],
    glyphs: &[Glyph],
    skew: f32,
    fringe: f32,
) -> Option<[f32; 4]> {
    use skrifa::{
        FontRef, GlyphId, MetadataProvider,
        instance::{NormalizedCoord, Size},
    };
    let font = FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let coordinates: Vec<_> = coordinates
        .iter()
        .map(|value| NormalizedCoord::from_bits(*value))
        .collect();
    let metrics = font.glyph_metrics(Size::new(size), coordinates.as_slice());
    let mut bounds: Option<[f32; 4]> = None;
    for glyph in glyphs {
        let ink = metrics.bounds(GlyphId::new(glyph.id))?;
        if ink.x_min == ink.x_max || ink.y_min == ink.y_max {
            continue;
        }
        let next = [
            glyph.x + ink.x_min + (skew * ink.y_min).min(skew * ink.y_max),
            glyph.y - ink.y_max,
            glyph.x + ink.x_max + (skew * ink.y_min).max(skew * ink.y_max),
            glyph.y - ink.y_min,
        ];
        bounds = Some(match bounds {
            Some(old) => [
                old[0].min(next[0]),
                old[1].min(next[1]),
                old[2].max(next[2]),
                old[3].max(next[3]),
            ],
            None => next,
        });
    }
    // Keep an antialiasing fringe; missing font metrics conservatively disable culling.
    Some(
        bounds
            .map(|b| {
                [
                    b[0] - fringe,
                    b[1] - fringe,
                    b[2] - b[0] + 2.0 * fringe,
                    b[3] - b[1] + 2.0 * fringe,
                ]
            })
            .unwrap_or([0.0; 4]),
    )
}
