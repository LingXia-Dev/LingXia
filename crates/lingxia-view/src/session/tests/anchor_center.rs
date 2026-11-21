//! An inset at `anchor(center)` puts a box inside an anchored popover on its anchor's centre
//! line, however the popover was aligned or pushed in, never past the popover's straight edge.
use crate::ViewSession;

const CARET: f32 = 12.0;
const RADIUS: f32 = 8.0;

/// The anchor's, the popover's and the caret's boxes for an anchor at `left` (`width` wide,
/// 200 down), a popover `area` and `wide`, and a caret placed by `caret` (style entries).
fn boxes(dir: &str, left: u32, width: u32, area: &str, wide: u32, caret: &str) -> [[f32; 4]; 3] {
    let module = super::source(&format!(
        "function render(){{return h('page',{{dir:'{dir}'}},h('view',{{style:{{height:200}}}}),h('view',{{id:'a','data-testid':'a',style:{{marginInlineStart:{left},width:{width},height:20}}}}),h('view',{{id:'tip',popover:'manual',open:true,anchor:'a','data-testid':'tip',style:{{positionArea:'{area}',width:{wide},height:60,borderRadius:{RADIUS}}}}},h('view',{{'data-testid':'caret',style:{{position:'absolute',width:{CARET},height:{CARET},{caret}}}}})))}}"
    ));
    let mut page = ViewSession::new(&module).unwrap();
    let display = page.layout(300, 400, 1.0).unwrap();
    ["a", "tip", "caret"].map(|id| {
        display
            .nodes
            .iter()
            .find(|n| n.test_id == id)
            .unwrap()
            .bounds
    })
}

const BELOW: &str = "top:-6,left:'anchor(center)',marginLeft:-6";

fn centre_x(b: [f32; 4]) -> f32 {
    b[0] + b[2] / 2.0
}

#[test]
fn a_caret_points_at_the_anchor_centred_start_and_end_aligned() {
    for (area, left, wide) in [
        ("bottom", 100, 80),
        ("bottom span-inline-end", 100, 160),
        ("bottom span-inline-start", 200, 160),
    ] {
        let [anchor, tip, caret] = boxes("ltr", left, 40, area, wide, BELOW);
        assert_eq!(centre_x(caret), centre_x(anchor), "{area}: {tip:?}");
        assert_eq!(caret[1], tip[1] - 6.0, "{area}: its top inset holds");
    }
}

#[test]
fn a_caret_follows_its_anchor_into_a_popover_pushed_in_from_either_screen_edge() {
    // Centred under the anchor the popover would cross the edge; it is pushed in, the caret not.
    for left in [10, 250] {
        let [anchor, tip, caret] = boxes("ltr", left, 30, "bottom", 120, BELOW);
        assert_ne!(centre_x(tip), centre_x(anchor), "pushed in: {tip:?}");
        assert_eq!(centre_x(caret), centre_x(anchor), "{left}");
    }
}

#[test]
fn a_caret_stops_at_the_popovers_corner() {
    // The anchor's centre lies left of where the popover's straight edge begins.
    let [anchor, tip, caret] = boxes("ltr", 0, 12, "bottom", 120, BELOW);
    assert!(centre_x(anchor) < tip[0] + RADIUS + CARET / 2.0);
    assert_eq!(
        caret[0],
        tip[0] + RADIUS,
        "kept {RADIUS} in from the corner"
    );
    let [_, tip, caret] = boxes("ltr", 288, 12, "bottom", 120, BELOW);
    assert_eq!(caret[0] + CARET, tip[0] + tip[2] - RADIUS);
}

#[test]
fn an_inline_start_inset_at_anchor_center_is_the_right_edge_in_rtl() {
    let caret = "top:-6,insetInlineStart:'anchor(center)',marginInlineStart:-6";
    for area in ["bottom", "bottom span-inline-end"] {
        let [anchor, tip, caret] = boxes("rtl", 60, 40, area, 160, caret);
        assert_eq!(centre_x(caret), centre_x(anchor), "{area}: {tip:?}");
    }
}

#[test]
fn a_top_inset_at_anchor_center_follows_the_anchor_down_the_side_of_a_popover() {
    let caret = "left:-6,top:'anchor(center)',marginTop:-6";
    let [anchor, tip, centred] = boxes("ltr", 40, 40, "inline-end", 80, caret);
    assert_eq!(
        centred[1] + CARET / 2.0,
        anchor[1] + anchor[3] / 2.0,
        "{tip:?}"
    );
    assert_eq!(centred[0], tip[0] - 6.0);
    // Top-aligned with a 20 pt anchor, the centre line is inside the corner: it stops there.
    let [_, tip, kept] = boxes("ltr", 40, 40, "inline-end span-block-end", 80, caret);
    assert_eq!(kept[1], tip[1] + RADIUS);
}
