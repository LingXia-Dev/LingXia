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

const PROBLEMS: [u8; 4] = [153, 27, 27, 235];
/// Diagnostics the banner names before it stops; the terminal has them all.
const PROBLEM_LINES: usize = 3;

/// The dev session's problems banner across the top of a `width`-wide page, below `top`
/// (logical px): the report's count and first diagnostics, over the build still running.
pub(crate) fn problems(renderer: &mut Renderer, report: &str, width: f32, top: f32) -> Vec<Paint> {
    let lines = banner_lines(report, width);
    let x = MARGIN;
    let y = top + MARGIN;
    let height = PAD * 2.0 + LINE * lines.len() as f32;
    let mut paints = vec![
        Paint::Transform(Affine::IDENTITY),
        rect(
            PROBLEMS,
            [x, y, (width - MARGIN * 2.0).max(0.0), height],
            6.0,
        ),
    ];
    for (index, line) in lines.iter().enumerate() {
        text(
            renderer,
            line,
            [x + PAD, y + PAD + LINE * index as f32],
            &mut paints,
        );
    }
    paints
}

/// The report's first line (its count) and first diagnostics, each cut to what fits `width`;
/// a diagnostic's indented detail lines stay in the terminal.
fn banner_lines(report: &str, width: f32) -> Vec<String> {
    // Monospace glyphs advance about 0.6 em.
    let fits = (((width - MARGIN * 2.0 - PAD * 2.0) / (FONT * 0.6)).floor() as usize).max(8);
    let cut = |line: &str| {
        if line.chars().count() <= fits {
            line.to_owned()
        } else {
            let mut kept: String = line.chars().take(fits - 1).collect();
            kept.push('…');
            kept
        }
    };
    let mut lines = report
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with(char::is_whitespace));
    let mut shown: Vec<String> = lines.by_ref().take(PROBLEM_LINES + 1).map(cut).collect();
    if lines.next().is_some() {
        shown.push(cut("… more in the terminal"));
    }
    shown.push(cut(
        "The last good build keeps running; save a fix to reload.",
    ));
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_problems_banner_names_the_count_and_first_diagnostics_and_cuts_long_lines() {
        let report = "5 type errors\n\
            pages/a.tsx:1:1: TS2345: one\n  detail of one\n\
            pages/b.tsx:2:1: TS2345: two\npages/c.tsx:3:1: TS2345: three\n\
            pages/d.tsx:4:1: TS2345: four\npages/e.tsx:5:1: TS2345: five";
        let lines = banner_lines(report, 400.0);
        assert_eq!(lines[0], "5 type errors");
        assert_eq!(lines[1], "pages/a.tsx:1:1: TS2345: one");
        assert_eq!(lines[3], "pages/c.tsx:3:1: TS2345: three");
        assert_eq!(lines[4], "… more in the terminal");
        assert!(lines.last().unwrap().starts_with("The last good build"));
        assert!(!lines.iter().any(|line| line.contains("detail")));
        let narrow = banner_lines(&format!("1 type error\n{}", "x".repeat(200)), 200.0);
        assert!(narrow[1].ends_with('…') && narrow[1].chars().count() < 200);
    }
}
