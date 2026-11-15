//! An underline's style and colour: dotted and dashed lines break into marks, and a decoration
//! colour paints the line without touching the glyphs.
use crate::ViewSession;
use crate::scene::border::LineStyle;
use crate::scene::paint::Paint;

const BLUE: [u8; 4] = [0, 0, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

struct Line {
    color: [u8; 4],
    bounds: [f32; 4],
    style: LineStyle,
    /// Runs of red pixels along the line's middle row.
    marks: usize,
}

/// The first decoration line `render` draws on a white page, and the colours of its glyphs.
fn line(render: &str) -> (Line, Vec<[u8; 4]>) {
    let source = super::source(&format!(
        "function render(){{return h('page',{{style:{{backgroundColor:'#ffffff'}}}},{render})}}"
    ));
    let display = ViewSession::new(&source)
        .unwrap()
        .layout(800, 200, 1.0)
        .unwrap();
    let glyphs = display
        .commands
        .iter()
        .filter_map(|paint| match paint {
            Paint::Text { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    let (color, bounds, style) = display
        .commands
        .iter()
        .find_map(|paint| match paint {
            Paint::Rect { color, bounds, .. } if bounds[3] <= 12.0 => {
                Some((*color, *bounds, LineStyle::Solid))
            }
            Paint::Border {
                color,
                bounds,
                style,
                ..
            } => Some((*color, *bounds, *style)),
            _ => None,
        })
        .expect("a decoration line");
    let image = display.rasterize().unwrap();
    let pixels = image.pixels.data_as_u8_slice();
    let row = (bounds[1] + bounds[3] / 2.0) as usize;
    let red = |x: usize| {
        let at = (row * 800 + x) * 4;
        pixels[at] > 160 && pixels[at + 1] < 120 && pixels[at + 2] < 120
    };
    let (from, to) = (bounds[0] as usize, (bounds[0] + bounds[2]) as usize);
    let marks = (from..to)
        .filter(|x| red(*x) && (*x == from || !red(*x - 1)))
        .count();
    (
        Line {
            color,
            bounds,
            style,
            marks,
        },
        glyphs,
    )
}

fn text(style: &str) -> String {
    format!(
        "h('text',{{style:{{fontSize:100,color:'#0000ff',textDecorationLine:'underline',{style}}}}},'mmmmm')"
    )
}

#[test]
fn a_decoration_colour_paints_the_line_and_leaves_the_glyphs() {
    let (plain, _) = line(&text(""));
    assert_eq!((plain.color, plain.style), (BLUE, LineStyle::Solid));
    let (red, glyphs) = line(&text("textDecorationColor:'#ff0000'"));
    assert_eq!(red.color, RED);
    assert_eq!(red.bounds, plain.bounds, "the same line, in another colour");
    assert_eq!(glyphs, [BLUE]);
    assert_eq!(red.marks, 1, "a solid line is one unbroken mark");
    assert_eq!(
        line(&text("textDecorationColor:'currentColor'")).0.color,
        BLUE
    );
    // A role with an alpha resolves as it does for a fill.
    let (role, _) = line(&text("textDecorationColor:'accent/50'"));
    let (fill, _) = line("h('view',{style:{width:100,height:2,backgroundColor:'accent/50'}})");
    assert_eq!(role.color, fill.color);
    assert!(role.color[3] > 0 && role.color[3] < 255);
}

#[test]
fn dotted_and_dashed_lines_break_into_even_marks_at_the_line_s_place() {
    let (solid, _) = line(&text("textDecorationColor:'#ff0000'"));
    let (dotted, _) = line(&text(
        "textDecorationStyle:'dotted',textDecorationColor:'#ff0000'",
    ));
    let (dashed, _) = line(&text(
        "textDecorationStyle:'dashed',textDecorationColor:'#ff0000'",
    ));
    assert_eq!(
        (dotted.style, dashed.style),
        (LineStyle::Dotted, LineStyle::Dashed)
    );
    assert_eq!(dotted.bounds, solid.bounds);
    assert_eq!(dashed.bounds, solid.bounds);
    // Dots repeat every two thicknesses and dashes every six.
    let [_, _, width, size] = solid.bounds;
    let near = |marks: usize, period: f32| {
        let expected = width / (period * size);
        (marks as f32 - expected).abs() <= expected * 0.15 + 1.0
    };
    assert!(near(dotted.marks, 2.0), "{} dots", dotted.marks);
    assert!(near(dashed.marks, 6.0), "{} dashes", dashed.marks);
    assert!(dotted.marks > dashed.marks && dashed.marks > 1);
}

#[test]
fn a_link_styles_its_own_underline_and_a_run_inside_a_line_does_not_restyle_it() {
    let (link, _) = line(
        "h('text',{style:{fontSize:100,color:'#0000ff'}},'Read ',h('link',{onClick(){},style:{textDecorationStyle:'dotted',textDecorationColor:'#ff0000'}},'terms'))",
    );
    assert_eq!((link.color, link.style), (RED, LineStyle::Dotted));
    assert!(link.marks > 3);
    let (outer, _) = line(
        "h('text',{style:{fontSize:100,color:'#0000ff',textDecorationLine:'underline'}},h('span',{style:{textDecorationStyle:'dotted',textDecorationColor:'#ff0000'}},'mmmm'))",
    );
    assert_eq!((outer.color, outer.style), (BLUE, LineStyle::Solid));
}
