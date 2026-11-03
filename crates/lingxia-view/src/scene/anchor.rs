//! A popover against its anchor: composition, not layout, places it, so it follows the anchor
//! through scrolling without a View render.

/// Space between the anchor and the popover, and between the popover and the viewport edge.
const GAP: f32 = 4.0;
const EDGE: f32 = 8.0;

/// CSS `position-area`, resolved to physical sides for the popover's direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Area {
    pub above: bool,
    pub align: Align,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Align {
    /// The popover's left edge on the anchor's.
    #[default]
    Left,
    Right,
    Center,
}

impl Area {
    pub(crate) fn parse(value: Option<&str>, rtl: bool) -> Self {
        let value = value.unwrap_or("bottom span-inline-end");
        let (side, span) = value.split_once(' ').unwrap_or((value, ""));
        // Spanning toward the inline end puts the popover's start edge on the anchor's.
        let start = if rtl { Align::Right } else { Align::Left };
        let end = if rtl { Align::Left } else { Align::Right };
        let align = match span {
            "span-inline-end" => start,
            "span-inline-start" => end,
            _ => Align::Center,
        };
        Self {
            above: side == "top",
            align,
        }
    }
}

/// How far to move a popover laid out at `natural` so it sits on `area`'s side of `anchor`,
/// flipping to the other side when only that fits, and kept inside `viewport`.
pub(crate) fn place(
    natural: [f32; 4],
    anchor: [f32; 4],
    viewport: [f32; 4],
    area: Area,
) -> [f32; 2] {
    let [_, _, width, height] = natural;
    let right = viewport[0] + viewport[2] - EDGE;
    let bottom = viewport[1] + viewport[3] - EDGE;
    let x = match area.align {
        Align::Left => anchor[0],
        Align::Right => anchor[0] + anchor[2] - width,
        Align::Center => anchor[0] + (anchor[2] - width) / 2.0,
    };
    let x = x.min(right - width).max(viewport[0] + EDGE);
    let below = anchor[1] + anchor[3] + GAP;
    let above = anchor[1] - GAP - height;
    let fits_below = below + height <= bottom;
    let fits_above = above >= viewport[1] + EDGE;
    let low = || below.min(bottom - height).max(viewport[1] + EDGE);
    let y = match (area.above, fits_above, fits_below) {
        (true, true, _) | (false, true, false) => above,
        (true, false, false) => above.max(viewport[1] + EDGE),
        _ => low(),
    };
    [x - natural[0], y - natural[1]]
}

#[cfg(test)]
mod tests {
    use super::{Align, Area, place};

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
        assert_eq!(end.align, Align::Right);
        assert_eq!(place(natural, anchor, viewport, end), [140.0, 444.0]);
        // In RTL the inline start is the right edge.
        assert_eq!(Area::parse(None, true).align, Align::Right);
        let top = Area::parse(Some("top"), false);
        assert_eq!(place(natural, anchor, viewport, top), [170.0, 296.0]);
        // No room above: it flips below.
        let high = [200.0, 20.0, 100.0, 40.0];
        assert_eq!(place(natural, high, viewport, top), [170.0, 64.0]);
    }
}
