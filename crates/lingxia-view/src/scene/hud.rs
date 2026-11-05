//! The frame-statistics HUD: drawn by the core over the page, never part of its nodes,
//! semantics or hit testing.
use crate::layout::Renderer;
use crate::scene::paint::Paint;
use parley::PositionedLayoutItem;
use parley::StyleProperty;
use vello_cpu::Glyph;
use vello_cpu::kurbo::Affine;

/// What the HUD draws, sampled on the main thread for the worker.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Hud {
    pub lines: Vec<String>,
    /// Frame times in ms, oldest first.
    pub spark: Vec<f64>,
    /// The display refresh interval in ms.
    pub refresh: f64,
    /// Recent pointer positions in device px: as they arrived, and as of the frames that showed them.
    pub finger: Vec<[f32; 2]>,
    pub shown: Vec<[f32; 2]>,
}

const WIDTH: f32 = 188.0;
const PAD: f32 = 6.0;
const FONT: f32 = 11.0;
const LINE: f32 = 14.0;
const SPARK_HEIGHT: f32 = 32.0;
const MARGIN: f32 = 8.0;

const PANEL: [u8; 4] = [0, 0, 0, 191];
const TEXT: [u8; 4] = [255, 255, 255, 255];
const GUIDE: [u8; 4] = [255, 255, 255, 96];
const GOOD: [u8; 4] = [52, 211, 153, 255];
const SLOW: [u8; 4] = [251, 191, 36, 255];
const JANK: [u8; 4] = [248, 113, 113, 255];

fn rect(color: [u8; 4], bounds: [f32; 4], radius: f32) -> Paint {
    Paint::Rect {
        color,
        bounds,
        radius,
        corners: None,
    }
}

/// Paints for the top-right corner of a `width`-wide page, below `top` (logical px).
pub(crate) fn paints(
    renderer: &mut Renderer,
    hud: &Hud,
    width: f32,
    top: f32,
    scale: f32,
) -> Vec<Paint> {
    let height = PAD * 3.0 + LINE * hud.lines.len() as f32 + SPARK_HEIGHT;
    let x = (width - WIDTH - MARGIN).max(0.0);
    let y = top + MARGIN;
    let mut paints = vec![
        Paint::Transform(Affine::IDENTITY),
        rect(PANEL, [x, y, WIDTH, height], 6.0),
    ];
    for (index, line) in hud.lines.iter().enumerate() {
        text(
            renderer,
            line,
            [x + PAD, y + PAD + LINE * index as f32],
            &mut paints,
        );
    }
    // Bars scale to three refresh intervals; guides mark one and two.
    let spark_top = y + PAD * 2.0 + LINE * hud.lines.len() as f32;
    let inner = WIDTH - PAD * 2.0;
    let refresh = if hud.refresh > 0.0 {
        hud.refresh
    } else {
        16.667
    };
    let full = refresh * 3.0;
    let bar = inner / crate::session::host::HUD_SPARK as f32;
    let base = spark_top + SPARK_HEIGHT;
    for (index, ms) in hud.spark.iter().enumerate() {
        let tall = (ms.min(full) / full) as f32 * SPARK_HEIGHT;
        let color = if *ms > refresh * 2.0 {
            JANK
        } else if *ms > refresh * 1.5 {
            SLOW
        } else {
            GOOD
        };
        paints.push(rect(
            color,
            [
                x + PAD + bar * index as f32,
                base - tall,
                (bar - 1.0).max(1.0),
                tall.max(1.0),
            ],
            0.0,
        ));
    }
    for multiple in [1.0, 2.0] {
        let at = base - (multiple / 3.0) as f32 * SPARK_HEIGHT;
        paints.push(rect(GUIDE, [x + PAD, at, inner, 1.0], 0.0));
    }
    // The gap between the finger's dots and the frames' dots is the input latency, drawn.
    let scale = scale.max(f32::MIN_POSITIVE);
    for (trail, color, radius) in [(&hud.finger, JANK, 4.0), (&hud.shown, GOOD, 3.0)] {
        for [px, py] in trail {
            let (cx, cy) = (px / scale, py / scale);
            paints.push(rect(
                color,
                [cx - radius, cy - radius, radius * 2.0, radius * 2.0],
                radius,
            ));
        }
    }
    paints
}

fn text(renderer: &mut Renderer, line: &str, [x, y]: [f32; 2], paints: &mut Vec<Paint>) {
    use parley::style::{FontFamily, FontFamilyName, GenericFamily};
    let mut builder = renderer
        .text
        .ranged_builder(&mut renderer.fonts, line, 1.0, true);
    builder.push_default(StyleProperty::FontSize(FONT));
    builder.push_default(FontFamily::Single(FontFamilyName::Generic(
        GenericFamily::Monospace,
    )));
    let mut layout = builder.build(line);
    layout.break_all_lines(None);
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let baseline = glyph_run.baseline();
            let mut advance = glyph_run.offset();
            let glyphs: Vec<Glyph> = glyph_run
                .glyphs()
                .map(|g| {
                    let glyph = Glyph {
                        id: g.id,
                        x: x + advance + g.x,
                        y: y + baseline - g.y,
                    };
                    advance += g.advance;
                    glyph
                })
                .collect();
            paints.push(Paint::Text {
                color: TEXT,
                font: run.font().clone(),
                size: run.font_size(),
                coordinates: run
                    .normalized_coords()
                    .iter()
                    .map(|v| v.to_bits())
                    .collect(),
                skew: 0.0,
                glyphs,
                ink: None,
                gradient: None,
            });
        }
    }
}
