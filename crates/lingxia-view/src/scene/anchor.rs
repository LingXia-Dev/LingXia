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
    /// The popover's direction, which names its inline sides.
    pub rtl: bool,
}

/// The side of its anchor a popover ended on, in the popover's own direction: what `side-*:`
/// styles name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogicalSide {
    Top,
    Bottom,
    Start,
    End,
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
        Self { side, align, rtl }
    }

    /// `side` named in this popover's direction.
    pub(crate) fn logical(&self, side: Side) -> LogicalSide {
        match (side, self.rtl) {
            (Side::Above, _) => LogicalSide::Top,
            (Side::Below, _) => LogicalSide::Bottom,
            (Side::Left, false) | (Side::Right, true) => LogicalSide::Start,
            (Side::Left, true) | (Side::Right, false) => LogicalSide::End,
        }
    }
}

/// How far to move a popover laid out at `natural` so it sits on `area`'s side of `anchor`,
/// flipping to the other side when only that fits, and kept inside `viewport`. Beside an anchor
/// with room on neither side it goes below or above instead of covering the anchor.
///
/// `margin` (top, right, bottom, left) is the popover's own: the one facing the anchor on the
/// authored side replaces the default gap, kept when the popover flips, and the ones along the
/// anchor's edge move it off the edge it aligns to, as a margin box would.
pub(crate) fn place(
    natural: [f32; 4],
    anchor: [f32; 4],
    viewport: [f32; 4],
    area: Area,
    margin: [f32; 4],
) -> ([f32; 2], Side) {
    let [_, _, width, height] = natural;
    let [m_top, m_right, m_bottom, m_left] = margin.map(|m| if m.is_finite() { m } else { 0.0 });
    let facing = match area.side {
        Side::Below => m_top,
        Side::Above => m_bottom,
        Side::Right => m_left,
        Side::Left => m_right,
    };
    let gap = if facing > 0.0 { facing } else { GAP };
    let left = viewport[0] + EDGE;
    let top = viewport[1] + EDGE;
    let right = viewport[0] + viewport[2] - EDGE;
    let bottom = viewport[1] + viewport[3] - EDGE;
    let along = |start: f32, extent: f32, size: f32, before: f32, after: f32| match area.align {
        Align::Start => start + before,
        Align::End => start + extent - size - after,
        Align::Center => start + (extent - size + before - after) / 2.0,
    };
    if matches!(area.side, Side::Left | Side::Right) {
        let after = anchor[0] + anchor[2] + gap;
        let before = anchor[0] - gap - width;
        let fits_after = after + width <= right;
        let fits_before = before >= left;
        let x = match (area.side, fits_before, fits_after) {
            (Side::Right, _, true) | (Side::Left, false, true) => Some((after, Side::Right)),
            (Side::Left, true, _) | (Side::Right, true, false) => Some((before, Side::Left)),
            _ => None,
        };
        if let Some((x, side)) = x {
            let y = along(anchor[1], anchor[3], height, m_top, m_bottom)
                .min(bottom - height)
                .max(top);
            return ([x - natural[0], y - natural[1]], side);
        }
    }
    let above_first = area.side == Side::Above;
    // Beside an anchor with no room: centred under or over it.
    let x = match area.side {
        Side::Left | Side::Right => anchor[0] + (anchor[2] - width) / 2.0,
        _ => along(anchor[0], anchor[2], width, m_left, m_right),
    };
    let x = x.min(right - width).max(left);
    let below = anchor[1] + anchor[3] + gap;
    let above = anchor[1] - gap - height;
    let fits_below = below + height <= bottom;
    let fits_above = above >= top;
    let low = || below.min(bottom - height).max(top);
    let (y, side) = match (above_first, fits_above, fits_below) {
        (true, true, _) | (false, true, false) => (above, Side::Above),
        (true, false, false) => (above.max(top), Side::Above),
        _ => (low(), Side::Below),
    };
    ([x - natural[0], y - natural[1]], side)
}

#[cfg(test)]
mod tests {
    use super::{Align, Area, LogicalSide, Side, place};

    fn place_at(
        natural: [f32; 4],
        anchor: [f32; 4],
        viewport: [f32; 4],
        area: Area,
        margin: [f32; 4],
    ) -> [f32; 2] {
        place(natural, anchor, viewport, area, margin).0
    }

    const NONE: [f32; 4] = [0.0; 4];

    #[test]
    fn a_popover_sits_below_its_anchor_flips_above_and_stays_on_screen() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        // Room below: under the anchor, aligned to its start.
        assert_eq!(
            place_at(
                [0.0, 0.0, 280.0, 300.0],
                [16.0, 100.0, 360.0, 44.0],
                viewport,
                Area::default(),
                NONE
            ),
            [16.0, 148.0]
        );
        // No room below, room above: over it.
        assert_eq!(
            place_at(
                [0.0, 0.0, 280.0, 300.0],
                [16.0, 600.0, 360.0, 44.0],
                viewport,
                Area::default(),
                NONE
            ),
            [16.0, 296.0]
        );
        // Wider than the space right of the anchor: pulled in from the edge.
        assert_eq!(
            place_at(
                [0.0, 0.0, 280.0, 100.0],
                [300.0, 10.0, 80.0, 30.0],
                viewport,
                Area::default(),
                NONE
            ),
            [112.0, 44.0]
        );
        // Room on neither side: as low as fits, never off the top.
        assert_eq!(
            place_at(
                [0.0, 0.0, 280.0, 700.0],
                [16.0, 300.0, 360.0, 44.0],
                viewport,
                Area::default(),
                NONE
            ),
            [16.0, 92.0]
        );
    }

    #[test]
    fn placement_reports_the_side_it_ended_on_named_in_the_popover_direction() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let natural = [0.0, 0.0, 120.0, 100.0];
        let side = |anchor, area: Area| place(natural, anchor, viewport, area, NONE).1;
        // Asked for above, no room there: it ends below.
        let top = Area::parse(Some("top"), false);
        assert_eq!(side([16.0, 20.0, 80.0, 30.0], top), Side::Below);
        assert_eq!(side([16.0, 400.0, 80.0, 30.0], top), Side::Above);
        // Asked for the inline start, no room left of the anchor: it ends at its right.
        let start = Area::parse(Some("inline-start"), false);
        assert_eq!(side([20.0, 400.0, 40.0, 30.0], start), Side::Right);
        assert_eq!(start.logical(Side::Right), LogicalSide::End);
        // In RTL the inline start is the right side, and a flip to the left is its end.
        let rtl = Area::parse(Some("inline-start"), true);
        assert_eq!(rtl.side, Side::Right);
        assert_eq!(rtl.logical(Side::Right), LogicalSide::Start);
        assert_eq!(rtl.logical(Side::Left), LogicalSide::End);
        assert_eq!(rtl.logical(Side::Above), LogicalSide::Top);
    }

    #[test]
    fn position_area_picks_the_side_and_the_edge_in_the_inline_direction() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let anchor = [200.0, 400.0, 100.0, 40.0];
        let natural = [0.0, 0.0, 160.0, 100.0];
        let end = Area::parse(Some("bottom span-inline-start"), false);
        assert_eq!(end.align, Align::End);
        assert_eq!(
            place_at(natural, anchor, viewport, end, NONE),
            [140.0, 444.0]
        );
        // In RTL the inline start is the right edge.
        assert_eq!(Area::parse(None, true).align, Align::End);
        let top = Area::parse(Some("top"), false);
        assert_eq!(
            place_at(natural, anchor, viewport, top, NONE),
            [170.0, 296.0]
        );
        // No room above: it flips below.
        let high = [200.0, 20.0, 100.0, 40.0];
        assert_eq!(place_at(natural, high, viewport, top, NONE), [170.0, 64.0]);
    }

    #[test]
    fn a_popover_sits_beside_its_anchor_flips_sides_and_falls_back_under_it() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let natural = [0.0, 0.0, 160.0, 100.0];
        let anchor = [40.0, 400.0, 60.0, 40.0];
        // At the inline end, centred on the anchor's height, 4 px off its edge.
        let end = Area::parse(Some("inline-end"), false);
        assert_eq!((end.side, end.align), (Side::Right, Align::Center));
        assert_eq!(
            place_at(natural, anchor, viewport, end, NONE),
            [104.0, 370.0]
        );
        // Spanning toward the block end: tops level. Toward the block start: bottoms level.
        let down = Area::parse(Some("inline-end span-block-end"), false);
        assert_eq!(
            place_at(natural, anchor, viewport, down, NONE),
            [104.0, 400.0]
        );
        let up = Area::parse(Some("inline-end span-block-start"), false);
        assert_eq!(
            place_at(natural, anchor, viewport, up, NONE),
            [104.0, 340.0]
        );
        // In RTL the inline end is the left side.
        assert_eq!(Area::parse(Some("inline-end"), true).side, Side::Left);
        assert_eq!(Area::parse(Some("inline-start"), true).side, Side::Right);
        // No room at the end, room at the start: it flips.
        let near_end = [300.0, 400.0, 60.0, 40.0];
        assert_eq!(
            place_at(natural, near_end, viewport, end, NONE),
            [136.0, 370.0]
        );
        let start = Area::parse(Some("inline-start"), false);
        assert_eq!(
            place_at(natural, anchor, viewport, start, NONE),
            [104.0, 370.0],
            "flips to the end"
        );
        // Kept on screen along the block axis.
        let low = [40.0, 760.0, 60.0, 30.0];
        assert_eq!(place_at(natural, low, viewport, end, NONE), [104.0, 692.0]);
        // Room on neither side (a wide anchor on a phone): under it, centred, never over it.
        let wide = [100.0, 400.0, 200.0, 40.0];
        assert_eq!(place_at(natural, wide, viewport, end, NONE), [120.0, 444.0]);
        // And over it when there is no room below either.
        let wide_low = [100.0, 720.0, 200.0, 40.0];
        assert_eq!(
            place_at(natural, wide_low, viewport, end, NONE),
            [120.0, 616.0]
        );
    }

    #[test]
    fn a_popover_margin_sets_the_gap_on_its_anchor_side_and_keeps_it_through_a_flip() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let natural = [0.0, 0.0, 160.0, 100.0];
        let anchor = [200.0, 400.0, 100.0, 40.0];
        let below = Area::parse(Some("bottom"), false);
        // The top margin is the gap under the anchor; the others centre the margin box.
        assert_eq!(
            place_at(natural, anchor, viewport, below, [12.0, 0.0, 0.0, 0.0]),
            [170.0, 452.0]
        );
        assert_eq!(
            place_at(natural, anchor, viewport, below, [12.0; 4]),
            [170.0, 452.0]
        );
        // Aligned to the anchor's start: the start margin moves it off that edge.
        let start = Area::parse(None, false);
        assert_eq!(
            place_at(natural, anchor, viewport, start, [0.0, 0.0, 0.0, 10.0]),
            [210.0, 444.0]
        );
        // No room below: over the anchor, the same gap.
        let low = [200.0, 700.0, 100.0, 40.0];
        assert_eq!(
            place_at(natural, low, viewport, below, [12.0, 0.0, 0.0, 0.0]),
            [170.0, 588.0]
        );
        // Beside it: the facing margin.
        let end = Area::parse(Some("inline-end"), false);
        assert_eq!(
            place_at(
                natural,
                [40.0, 400.0, 60.0, 40.0],
                viewport,
                end,
                [0.0, 0.0, 0.0, 16.0]
            ),
            [116.0, 370.0]
        );
        // A zero or broken margin keeps the default gap.
        assert_eq!(
            place_at(natural, anchor, viewport, below, [f32::NAN, 0.0, 0.0, 0.0]),
            [170.0, 444.0]
        );
    }
}
