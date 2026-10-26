//! `position: sticky` inside a scroll: composition, not layout, offsets the box,
//! so scrolling never reruns layout or the View.

/// A sticky box's scroll, insets (top, right, bottom, left) and containing block.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Sticky {
    /// Nearest scroll ancestor; its content box is the sticky viewport.
    pub scroll: usize,
    pub insets: [Option<f32>; 4],
    /// The parent's content box in unscrolled coordinates; `None` when the
    /// parent is the scroll itself, whose content bounds nothing.
    pub container: Option<[f32; 4]>,
}

/// How far to move along one axis so `[start, size]` keeps `before`/`after`
/// from the viewport edges without leaving `[low, high]`.
fn axis(
    [start, size]: [f32; 2],
    [view_start, view_size]: [f32; 2],
    before: Option<f32>,
    after: Option<f32>,
    container: Option<[f32; 2]>,
) -> f32 {
    let mut shift = 0.0f32;
    if let Some(inset) = before {
        shift = shift.max(view_start + inset - start);
    }
    if let Some(inset) = after {
        let end = start + size;
        let limit = view_start + view_size - inset;
        if end > limit && shift == 0.0 {
            shift = limit - end;
        }
    }
    match container {
        // Stick only while some of the containing block is still in view.
        Some([low, high]) => shift
            .min((high - start - size).max(0.0))
            .max((low - start).min(0.0)),
        None => shift,
    }
}

impl Sticky {
    /// The offset for a box at `natural` (already scrolled with its ancestors).
    pub fn shift(
        &self,
        natural: [f32; 4],
        viewport: [f32; 4],
        container: Option<[f32; 4]>,
    ) -> [f32; 2] {
        let [top, right, bottom, left] = self.insets;
        [
            axis(
                [natural[0], natural[2]],
                [viewport[0], viewport[2]],
                left,
                right,
                container.map(|c| [c[0], c[0] + c[2]]),
            ),
            axis(
                [natural[1], natural[3]],
                [viewport[1], viewport[3]],
                top,
                bottom,
                container.map(|c| [c[1], c[1] + c[3]]),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticks_to_the_viewport_until_its_container_leaves() {
        let sticky = Sticky {
            scroll: 0,
            insets: [Some(0.0), None, None, None],
            container: None,
        };
        let viewport = [0.0, 100.0, 300.0, 400.0];
        assert_eq!(
            sticky.shift([0.0, 150.0, 300.0, 40.0], viewport, None),
            [0.0, 0.0]
        );
        assert_eq!(
            sticky.shift([0.0, 60.0, 300.0, 40.0], viewport, None),
            [0.0, 40.0]
        );
        // The container ends 20px below the header's top: it may move 20px only.
        assert_eq!(
            sticky.shift(
                [0.0, 60.0, 300.0, 40.0],
                viewport,
                Some([0.0, 0.0, 300.0, 120.0])
            ),
            [0.0, 20.0]
        );
        let bottom = Sticky {
            insets: [None, None, Some(10.0), None],
            ..sticky
        };
        assert_eq!(
            bottom.shift([0.0, 480.0, 300.0, 40.0], viewport, None),
            [0.0, -30.0]
        );
    }
}
