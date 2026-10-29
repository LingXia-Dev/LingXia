//! A popover against its anchor: composition, not layout, places it, so it follows the anchor
//! through scrolling without a View render.

/// Space between the anchor and the popover, and between the popover and the viewport edge.
const GAP: f32 = 4.0;
const EDGE: f32 = 8.0;

/// How far to move a popover laid out at `natural` so it sits below `anchor`, or above it when
/// only that fits, aligned to the anchor's start and kept inside `viewport`.
pub(crate) fn place(natural: [f32; 4], anchor: [f32; 4], viewport: [f32; 4]) -> [f32; 2] {
    let [_, _, width, height] = natural;
    let right = viewport[0] + viewport[2] - EDGE;
    let bottom = viewport[1] + viewport[3] - EDGE;
    let x = anchor[0].min(right - width).max(viewport[0] + EDGE);
    let below = anchor[1] + anchor[3] + GAP;
    let above = anchor[1] - GAP - height;
    let y = if below + height <= bottom || above < viewport[1] + EDGE {
        below.min(bottom - height).max(viewport[1] + EDGE)
    } else {
        above
    };
    [x - natural[0], y - natural[1]]
}

#[cfg(test)]
mod tests {
    use super::place;

    #[test]
    fn a_popover_sits_below_its_anchor_flips_above_and_stays_on_screen() {
        let viewport = [0.0, 0.0, 400.0, 800.0];
        // Room below: under the anchor, aligned to its start.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 300.0],
                [16.0, 100.0, 360.0, 44.0],
                viewport
            ),
            [16.0, 148.0]
        );
        // No room below, room above: over it.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 300.0],
                [16.0, 600.0, 360.0, 44.0],
                viewport
            ),
            [16.0, 296.0]
        );
        // Wider than the space right of the anchor: pulled in from the edge.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 100.0],
                [300.0, 10.0, 80.0, 30.0],
                viewport
            ),
            [112.0, 44.0]
        );
        // Room on neither side: as low as fits, never off the top.
        assert_eq!(
            place(
                [0.0, 0.0, 280.0, 700.0],
                [16.0, 300.0, 360.0, 44.0],
                viewport
            ),
            [16.0, 92.0]
        );
    }
}
