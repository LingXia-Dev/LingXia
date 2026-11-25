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

/// Where `place` put a popover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    /// How far to move it from where it was laid out.
    pub shift: [f32; 2],
    pub side: Side,
    /// The height it may take on that side, when its content wants more: it is capped to it and
    /// scrolls inside.
    pub cap: Option<f32>,
}

/// The part of an editor a popover sits against: `<name>:caret`, `<name>:selection`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnchorPart {
    Caret = 0,
    Selection = 1,
}

impl AnchorPart {
    pub(crate) fn split(target: &str) -> (&str, Option<Self>) {
        if let Some(name) = target.strip_suffix(":caret") {
            (name, Some(Self::Caret))
        } else if let Some(name) = target.strip_suffix(":selection") {
            (name, Some(Self::Selection))
        } else {
            (target, None)
        }
    }
}

/// What a popover sits against.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Anchoring {
    /// The anchor's identity.
    pub target: String,
    pub area: Area,
    /// The popover's own margin (top, right, bottom, left).
    pub margin: [f32; 4],
    /// The editor's caret or selection rather than its box.
    pub part: Option<AnchorPart>,
}

/// How far to move a popover laid out at `natural` so it sits on `area`'s side of `anchor`,
/// kept inside `viewport` and never over the anchor. It takes the authored side when its
/// content (`wanted` tall) fits there, else the other side when it fits there, else the roomier
/// side with its height capped to that side's room. Beside an anchor with room on neither side
/// it goes below or above instead.
///
/// `margin` (top, right, bottom, left) is the popover's own: the one facing the anchor on the
/// authored side replaces the default gap, kept when the popover flips, and the ones along the
/// anchor's edge move it off the edge it aligns to, as a margin box would.
pub(crate) fn place(
    natural: [f32; 4],
    wanted: f32,
    anchor: [f32; 4],
    viewport: [f32; 4],
    area: Area,
    margin: [f32; 4],
) -> Placement {
    let [_, _, width, height] = natural;
    let wanted = wanted.max(height);
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
    let shift = |x: f32, y: f32| [x - natural[0], y - natural[1]];
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
            let room = bottom - top;
            let cap = (wanted > room + SLACK).then(|| room.max(0.0).floor());
            return Placement {
                shift: shift(x, y),
                side,
                cap,
            };
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
    let room_below = (bottom - below).max(0.0);
    let room_above = (anchor[1] - gap - top).max(0.0);
    let (fits_below, fits_above) = (wanted <= room_below + SLACK, wanted <= room_above + SLACK);
    let go_above = match (fits_above, fits_below) {
        (true, true) => above_first,
        (true, false) => true,
        (false, true) => false,
        // Neither fits: the roomier side, the authored one on a tie.
        (false, false) if above_first => room_above >= room_below,
        (false, false) => room_above > room_below,
    };
    let (room, fits) = if go_above {
        (room_above, fits_above)
    } else {
        (room_below, fits_below)
    };
    let y = if go_above {
        anchor[1] - gap - height
    } else {
        below
    };
    Placement {
        shift: shift(x, y),
        side: if go_above { Side::Above } else { Side::Below },
        cap: (!fits).then(|| room.floor()),
    }
}

/// What a fit tolerates, so rounding never makes a list that nearly fits scroll by a pixel.
const SLACK: f32 = 1.0;

/// Below this a capped popover is a sliver; the page scrolls its anchor to make this much room.
pub(crate) const MIN_ROOM: f32 = 96.0;

/// The anchor with `MIN_ROOM` (and the gap) beside it on `side`: what the page reveals to give
/// a capped popover room for about two rows.
pub(crate) fn room_rect(anchor: [f32; 4], side: Side) -> [f32; 4] {
    let extra = GAP + MIN_ROOM + EDGE;
    match side {
        Side::Above => [anchor[0], anchor[1] - extra, anchor[2], anchor[3] + extra],
        _ => [anchor[0], anchor[1], anchor[2], anchor[3] + extra],
    }
}

/// The value of an inset that puts an edge on the anchor's centre line.
pub(crate) const ANCHOR_CENTER: &str = "anchor(center)";

/// An absolutely positioned box inside an anchored popover with an inset at `anchor(center)`:
/// its edge on the anchor's centre line once the popover is placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AnchorInset {
    /// The anchored popover it sits in.
    pub popover: usize,
    /// Per axis, the edge of the containing padding box the inset was laid out from, as
    /// laid out: a zero inset there, which composition moves to the centre line.
    pub base: [Option<f32>; 2],
}

impl AnchorInset {
    /// `record`'s, when it is absolutely positioned inside an anchored popover (`popover`
    /// finds the nearest one above `index`) and an inset names `anchor(center)`;
    /// `padding_box` is its containing block's (left, top, right, bottom).
    pub(crate) fn of(
        record: &crate::protocol::Record,
        rtl: bool,
        popover: impl Fn(usize) -> Option<usize>,
        index: usize,
        padding_box: impl Fn() -> Option<[f32; 4]>,
    ) -> Option<Self> {
        use crate::protocol::field;
        let centred = |id: usize| record.string(id) == Some(ANCHOR_CENTER);
        let side = |physical, left| {
            crate::layout::style::side(
                record,
                physical,
                field::INSET_INLINE_START,
                field::INSET_INLINE_END,
                left,
                rtl,
            )
        };
        let [left, right] = [
            side(field::INSET_LEFT, true),
            side(field::INSET_RIGHT, false),
        ];
        let (top, bottom) = (field::INSET_TOP, field::INSET_BOTTOM);
        if ![left, right, top, bottom].into_iter().any(centred)
            || !matches!(record.string(field::POSITION), Some("absolute" | "fixed"))
        {
            return None;
        }
        let popover = popover(index)?;
        let [l, t, r, b] = padding_box()?;
        let pick = |start: usize, end: usize, low: f32, high: f32| {
            if centred(start) {
                Some(low)
            } else if centred(end) {
                Some(high)
            } else {
                None
            }
        };
        Some(Self {
            popover,
            base: [pick(left, right, l, r), pick(top, bottom, t, b)],
        })
    }

    /// The translation (subtracted from layout) that moves a box laid out at `laid` onto the
    /// anchor's `centre` line, keeping it inside the popover's straight edge: at least the
    /// corner `radius` from each corner. `translation` is its parent's.
    pub(crate) fn resolve(
        &self,
        laid: [f32; 4],
        mut translation: [f32; 2],
        centre: [f32; 2],
        popover: [f32; 4],
        radius: f32,
    ) -> [f32; 2] {
        for axis in 0..2 {
            let Some(base) = self.base[axis] else {
                continue;
            };
            let (start, size, extent) = (laid[axis], laid[axis + 2], popover[axis + 2]);
            let mut shift = centre[axis] - (base - translation[axis]);
            let at = start - translation[axis] + shift;
            let (low, high) = (
                popover[axis] + radius,
                popover[axis] + extent - radius - size,
            );
            let kept = if low <= high {
                at.clamp(low, high)
            } else {
                popover[axis] + (extent - size) / 2.0
            };
            shift += kept - at;
            translation[axis] -= shift;
        }
        translation
    }
}

#[cfg(test)]
mod tests {
    use super::Placement;
    use super::{Align, Area, LogicalSide, Side, place};

    fn place_at(
        natural: [f32; 4],
        anchor: [f32; 4],
        viewport: [f32; 4],
        area: Area,
        margin: [f32; 4],
    ) -> [f32; 2] {
        place(natural, natural[3], anchor, viewport, area, margin).shift
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
    }

    #[test]
    fn with_room_on_neither_side_a_popover_takes_the_roomier_one_capped_and_never_covers_its_anchor()
     {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let natural = [0.0, 0.0, 280.0, 700.0];
        let at = |anchor: [f32; 4], area| place(natural, natural[3], anchor, viewport, area, NONE);
        // 444 below, 288 above: under the anchor, capped to the 444.
        let low = at([16.0, 300.0, 360.0, 44.0], Area::default());
        assert_eq!(
            low,
            Placement {
                shift: [16.0, 348.0],
                side: Side::Below,
                cap: Some(444.0)
            }
        );
        // 244 below, 488 above: over it, capped to the 488.
        let high = at([16.0, 500.0, 360.0, 44.0], Area::default());
        assert_eq!((high.side, high.cap), (Side::Above, Some(488.0)));
        // Laid out at the cap, it is placed the same way and wants the same cap: no oscillation.
        let capped = place(
            [0.0, 0.0, 280.0, 488.0],
            700.0,
            [16.0, 500.0, 360.0, 44.0],
            viewport,
            Area::default(),
            NONE,
        );
        assert_eq!(capped.side, Side::Above);
        assert_eq!(capped.cap, Some(488.0));
        assert_eq!(capped.shift[1] + 488.0, 496.0, "ends 4 px over the anchor");
        // Asked for above with equal room: the authored side wins the tie.
        let tie = at([16.0, 394.0, 360.0, 12.0], Area::parse(Some("top"), false));
        assert_eq!(tie.side, Side::Above);
        // When it fits on one side it is not capped.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 200.0],
                200.0,
                [16.0, 300.0, 360.0, 44.0],
                viewport,
                Area::default(),
                NONE
            )
            .cap,
            None
        );
    }

    #[test]
    fn placement_reports_the_side_it_ended_on_named_in_the_popover_direction() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        let natural = [0.0, 0.0, 120.0, 100.0];
        let side =
            |anchor, area: Area| place(natural, natural[3], anchor, viewport, area, NONE).side;
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
