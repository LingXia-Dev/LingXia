//! A popover against its anchor: composition, not layout, places it, so it follows the anchor
//! through scrolling without a View render.

/// Space between the anchor and the popover, and between the popover and the viewport edge.
const GAP: f32 = 4.0;
const EDGE: f32 = 8.0;

/// CSS `position-area`, resolved to physical sides for the popover's direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Area {
    pub side: Side,
    pub align: Align,
}

/// The side of the anchor the popover sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Side {
    #[default]
    Below,
    Above,
    Left,
    Right,
}

/// Along the anchor's side: which of the popover's edges meets the anchor's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Align {
    /// The popover's left edge on the anchor's (its top edge, beside the anchor).
    #[default]
    Start,
    End,
    Center,
}

impl Area {
    pub(crate) fn parse(value: Option<&str>, rtl: bool) -> Self {
        let value = value.unwrap_or("bottom span-inline-end");
        let (side, span) = value.split_once(' ').unwrap_or((value, ""));
        // Spanning toward the inline end puts the popover's start edge on the anchor's.
        let (start, end) = if rtl {
            (Align::End, Align::Start)
        } else {
            (Align::Start, Align::End)
        };
        let align = match span {
            "span-inline-end" => start,
            "span-inline-start" => end,
            "span-block-end" => Align::Start,
            "span-block-start" => Align::End,
            _ => Align::Center,
        };
        let side = match side {
            "top" => Side::Above,
            "inline-end" if rtl => Side::Left,
            "inline-end" => Side::Right,
            "inline-start" if rtl => Side::Right,
            "inline-start" => Side::Left,
            _ => Side::Below,
        };
        Self { side, align }
    }
}

/// How far to move a popover laid out at `natural` so it sits on `area`'s side of `anchor`,
/// flipping to the other side when only that fits, and kept inside `viewport`. Beside an anchor
/// with room on neither side it goes below or above instead of covering the anchor.
pub(crate) fn place(
    natural: [f32; 4],
    anchor: [f32; 4],
    viewport: [f32; 4],
    area: Area,
) -> [f32; 2] {
    let [_, _, width, height] = natural;
    let left = viewport[0] + EDGE;
    let top = viewport[1] + EDGE;
    let right = viewport[0] + viewport[2] - EDGE;
    let bottom = viewport[1] + viewport[3] - EDGE;
    let along = |start: f32, extent: f32, size: f32| match area.align {
        Align::Start => start,
        Align::End => start + extent - size,
        Align::Center => start + (extent - size) / 2.0,
    };
    if matches!(area.side, Side::Left | Side::Right) {
        let after = anchor[0] + anchor[2] + GAP;
        let before = anchor[0] - GAP - width;
        let fits_after = after + width <= right;
        let fits_before = before >= left;
        let x = match (area.side, fits_before, fits_after) {
            (Side::Right, _, true) | (Side::Left, false, true) => Some(after),
            (Side::Left, true, _) | (Side::Right, true, false) => Some(before),
            _ => None,
        };
        if let Some(x) = x {
            let y = along(anchor[1], anchor[3], height)
                .min(bottom - height)
                .max(top);
            return [x - natural[0], y - natural[1]];
        }
    }
    let above_first = area.side == Side::Above;
    // Beside an anchor with no room: centred under or over it.
    let x = match area.side {
        Side::Left | Side::Right => anchor[0] + (anchor[2] - width) / 2.0,
        _ => along(anchor[0], anchor[2], width),
    };
    let x = x.min(right - width).max(left);
    let below = anchor[1] + anchor[3] + GAP;
    let above = anchor[1] - GAP - height;
    let fits_below = below + height <= bottom;
    let fits_above = above >= top;
    let low = || below.min(bottom - height).max(top);
    let y = match (above_first, fits_above, fits_below) {
        (true, true, _) | (false, true, false) => above,
        (true, false, false) => above.max(top),
        _ => low(),
    };
    [x - natural[0], y - natural[1]]
}

#[cfg(test)]
mod tests {
    use super::{Align, Area, Side, place};

    #[test]
    fn a_popover_sits_below_its_anchor_flips_above_and_stays_on_screen() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        // Room below: under the anchor, aligned to its start.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 300.0],
                [16.0, 100.0, 360.0, 44.0],
                viewport,
                Area::default()
            ),
            [16.0, 148.0]
        );
        // No room below, room above: over it.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 300.0],
                [16.0, 600.0, 360.0, 44.0],
                viewport,
                Area::default()
            ),
            [16.0, 296.0]
        );
        // Wider than the space right of the anchor: pulled in from the edge.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 100.0],
                [300.0, 10.0, 80.0, 30.0],
                viewport,
                Area::default()
            ),
            [112.0, 44.0]
        );
        // Room on neither side: as low as fits, never off the top.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 700.0],
                [16.0, 300.0, 360.0, 44.0],
                viewport,
                Area::default()
            ),
            [16.0, 92.0]
        );
    }

    #[test]
    fn position_area_picks_the_side_and_the_edge_in_the_inline_direction() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let anchor = [200.0, 400.0, 100.0, 40.0];
        let natural = [0.0, 0.0, 160.0, 100.0];
        let end = Area::parse(Some("bottom span-inline-start"), false);
        assert_eq!(end.align, Align::End);
        assert_eq!(place(natural, anchor, viewport, end), [140.0, 444.0]);
        // In RTL the inline start is the right edge.
        assert_eq!(Area::parse(None, true).align, Align::End);
        let top = Area::parse(Some("top"), false);
        assert_eq!(place(natural, anchor, viewport, top), [170.0, 296.0]);
        // No room above: it flips below.
        let high = [200.0, 20.0, 100.0, 40.0];
        assert_eq!(place(natural, high, viewport, top), [170.0, 64.0]);
    }

    #[test]
    fn a_popover_sits_beside_its_anchor_flips_sides_and_falls_back_under_it() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let natural = [0.0, 0.0, 160.0, 100.0];
        let anchor = [40.0, 400.0, 60.0, 40.0];
        // At the inline end, centred on the anchor's height, 4 px off its edge.
        let end = Area::parse(Some("inline-end"), false);
        assert_eq!((end.side, end.align), (Side::Right, Align::Center));
        assert_eq!(place(natural, anchor, viewport, end), [104.0, 370.0]);
        // Spanning toward the block end: tops level. Toward the block start: bottoms level.
        let down = Area::parse(Some("inline-end span-block-end"), false);
        assert_eq!(place(natural, anchor, viewport, down), [104.0, 400.0]);
        let up = Area::parse(Some("inline-end span-block-start"), false);
        assert_eq!(place(natural, anchor, viewport, up), [104.0, 340.0]);
        // In RTL the inline end is the left side.
        assert_eq!(Area::parse(Some("inline-end"), true).side, Side::Left);
        assert_eq!(Area::parse(Some("inline-start"), true).side, Side::Right);
        // No room at the end, room at the start: it flips.
        let near_end = [300.0, 400.0, 60.0, 40.0];
        assert_eq!(place(natural, near_end, viewport, end), [136.0, 370.0]);
        let start = Area::parse(Some("inline-start"), false);
        assert_eq!(
            place(natural, anchor, viewport, start),
            [104.0, 370.0],
            "flips to the end"
        );
        // Kept on screen along the block axis.
        let low = [40.0, 760.0, 60.0, 30.0];
        assert_eq!(place(natural, low, viewport, end), [104.0, 692.0]);
        // Room on neither side (a wide anchor on a phone): under it, centred, never over it.
        let wide = [100.0, 400.0, 200.0, 40.0];
        assert_eq!(place(natural, wide, viewport, end), [120.0, 444.0]);
        // And over it when there is no room below either.
        let wide_low = [100.0, 720.0, 200.0, 40.0];
        assert_eq!(place(natural, wide_low, viewport, end), [120.0, 616.0]);
    }
}
