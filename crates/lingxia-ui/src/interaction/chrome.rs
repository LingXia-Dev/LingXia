//! Geometry for platform chrome drawn over a presented frame: whether a point
//! shows through a node's clips, and the selection handles' targets. Adapters
//! draw the chrome; these answers are the shared policy.
use serde::Deserialize;

/// Handle hit radius and the knob's offset below the line, in logical pixels.
const HANDLE_RADIUS: f64 = 22.0;
const HANDLE_KNOB: f64 = 8.0;
/// A handle grabs only touches at most this far above the line bottom.
const HANDLE_REACH_ABOVE: f64 = 4.0;

/// How adapters draw press and keyboard-focus feedback for the page's theme.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackStyle {
    /// ARGB over an unstyled pressed node's clipped shape. A node with a
    /// `pressedStyle` repaints itself and gets no overlay.
    pub press_overlay: u32,
    /// ARGB stroke of the keyboard focus ring.
    pub focus_ring: u32,
    /// Ring stroke width, logical pixels.
    pub focus_ring_width: f32,
    /// ARGB of text selection handles.
    pub selection_handle: u32,
}

impl FeedbackStyle {
    pub fn for_theme(dark: bool) -> Self {
        if dark {
            Self {
                press_overlay: 0x33FF_FFFF,
                focus_ring: 0xFFE6_EBF5,
                focus_ring_width: 2.0,
                selection_handle: 0xFF8C_9BFF,
            }
        } else {
            Self {
                press_overlay: 0x2200_0000,
                focus_ring: 0xFF17_2033,
                focus_ring_width: 2.0,
                selection_handle: 0xFF46_5CDA,
            }
        }
    }
}

#[derive(Deserialize)]
struct Clip {
    bounds: Option<[f64; 4]>,
    #[serde(default)]
    radius: f64,
    radii: Option<serde_json::Value>,
}

#[derive(Default, Deserialize)]
struct Geometry {
    anchor: Option<[f64; 4]>,
    caret: Option<[f64; 4]>,
    content_bounds: Option<[f64; 4]>,
}

#[derive(Default, Deserialize)]
struct Input {
    #[serde(default)]
    geometry: Option<Geometry>,
}

/// The fields of a frame snapshot node that chrome geometry reads.
#[derive(Deserialize)]
pub struct ChromeNode {
    bounds: Option<[f64; 4]>,
    visible_bounds: Option<[f64; 4]>,
    #[serde(default)]
    radius: f64,
    radii: Option<serde_json::Value>,
    #[serde(default)]
    clips: Vec<Clip>,
    #[serde(default)]
    input: Option<Input>,
    /// A static-text selection exposes the same geometry as an editor's.
    #[serde(default)]
    text_selection: Option<Geometry>,
}

impl ChromeNode {
    fn geometry(&self) -> Option<&Geometry> {
        self.input
            .as_ref()
            .and_then(|input| input.geometry.as_ref())
            .or(self.text_selection.as_ref())
    }
}

impl ChromeNode {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("UI_CHROME: {e}"))
    }
}

/// Per-corner radii (top-left, top-right, bottom-right, bottom-left), CSS-scaled
/// to fit; `None` for malformed radii.
fn radii(
    corners: Option<&serde_json::Value>,
    radius: f64,
    width: f64,
    height: f64,
) -> Option<[[f64; 2]; 4]> {
    let uniform = radius.min(width.min(height) / 2.0).max(0.0);
    let mut result = [[uniform; 2]; 4];
    if let Some(corners) = corners {
        let corners = corners.as_array().filter(|c| c.len() == 4)?;
        for (slot, pair) in result.iter_mut().zip(corners) {
            let pair = pair.as_array().filter(|p| p.len() == 2)?;
            let (rx, ry) = (pair[0].as_f64()?, pair[1].as_f64()?);
            if !rx.is_finite() || !ry.is_finite() || rx < 0.0 || ry < 0.0 {
                return None;
            }
            *slot = if rx == 0.0 || ry == 0.0 {
                [0.0; 2]
            } else {
                [rx, ry]
            };
        }
    }
    let sums = [
        (result[0][0] + result[1][0], width),
        (result[3][0] + result[2][0], width),
        (result[0][1] + result[3][1], height),
        (result[1][1] + result[2][1], height),
    ];
    let factor = sums
        .iter()
        .filter(|(sum, _)| *sum > 0.0)
        .fold(1.0f64, |factor, (sum, side)| factor.min(side / sum));
    for corner in &mut result {
        corner[0] *= factor;
        corner[1] *= factor;
    }
    Some(result)
}

fn inside(
    bounds: Option<[f64; 4]>,
    radius: f64,
    corners: Option<&serde_json::Value>,
    x: f64,
    y: f64,
) -> bool {
    let Some([left, top, width, height]) = bounds else {
        return false;
    };
    if ![x, y, left, top, width, height]
        .iter()
        .all(|v| v.is_finite())
        || width <= 0.0
        || height <= 0.0
        || x < left
        || y < top
        || x >= left + width
        || y >= top + height
    {
        return false;
    }
    let Some(radii) = radii(corners, radius, width, height) else {
        return false;
    };
    for (index, [rx, ry]) in radii.iter().enumerate() {
        if *rx == 0.0 || *ry == 0.0 {
            continue;
        }
        let right = index == 1 || index == 2;
        let bottom = index >= 2;
        let cx = if right { left + width - rx } else { left + rx };
        let cy = if bottom { top + height - ry } else { top + ry };
        if (if right { x > cx } else { x < cx }) && (if bottom { y > cy } else { y < cy }) {
            let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
            return dx * dx + dy * dy <= 1.0;
        }
    }
    true
}

/// Whether chrome anchored at a logical point shows through every ancestor clip.
pub fn within_clips(node: &ChromeNode, x: f64, y: f64) -> bool {
    node.clips
        .iter()
        .all(|clip| inside(clip.bounds, clip.radius, clip.radii.as_ref(), x, y))
}

/// Whether chrome anchored at a logical point would show: inside the node's
/// visible bounds (and its own corners when `own_radius`) and every clip.
pub fn contains(node: &ChromeNode, x: f64, y: f64, own_radius: bool) -> bool {
    inside(node.visible_bounds.or(node.bounds), 0.0, None, x, y)
        && (!own_radius || inside(node.bounds, node.radius, node.radii.as_ref(), x, y))
        && within_clips(node, x, y)
}

/// A handle's line: center x, line bottom, line middle.
pub type Endpoint = [f64; 3];

/// The anchor (0) or caret (1) handle position; `None` when that end is
/// scrolled out of the editor's content or clipped away.
pub fn endpoint(node: &ChromeNode, which: usize) -> Option<Endpoint> {
    let geometry = node.geometry()?;
    let [x, y, width, height] = if which == 0 {
        geometry.anchor?
    } else {
        geometry.caret?
    };
    let [cl, ct, cw, ch] = geometry.content_bounds?;
    let (center, bottom, middle) = (x + width / 2.0, y + height, y + height / 2.0);
    let visible = [center, bottom, middle].iter().all(|v| v.is_finite())
        && center >= cl
        && center <= cl + cw
        && bottom > ct
        && bottom <= ct + ch + 0.5
        && contains(node, center, bottom - 0.5, false);
    visible.then_some([center, bottom, middle])
}

/// The handle a touch at a logical point grabs: which end and its endpoint.
/// The anchor handle exists only for a range selection.
pub fn handle_hit(node: &ChromeNode, x: f64, y: f64, collapsed: bool) -> Option<(usize, Endpoint)> {
    if !within_clips(node, x, y) {
        return None;
    }
    let mut best = HANDLE_RADIUS * HANDLE_RADIUS;
    let mut hit = None;
    for which in usize::from(collapsed)..2 {
        let Some(end) = endpoint(node, which) else {
            continue;
        };
        if y < end[1] - HANDLE_REACH_ABOVE {
            continue;
        }
        let distance = (x - end[0]).powi(2) + (y - end[1] - HANDLE_KNOB).powi(2);
        if distance < best {
            best = distance;
            hit = Some((which, end));
        }
    }
    hit
}

/// Where a dragged handle queries the editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragPoint {
    /// Clamped into the visible part of the editor's content.
    pub point: [f64; 2],
    /// -1/0/1 per axis when the finger is beyond the content edge.
    pub edge: [i32; 2],
    /// False when the clamped point is clipped away; skip this query.
    pub visible: bool,
}

/// `None` when the editor has no visible content left; the drag ends.
pub fn drag_point(node: &ChromeNode, x: f64, y: f64) -> Option<DragPoint> {
    let [bl, bt, bw, bh] = node.visible_bounds?;
    let [cl, ct, cw, ch] = node.geometry()?.content_bounds?;
    let (left, top) = (bl.max(cl), bt.max(ct));
    let (right, bottom) = ((bl + bw).min(cl + cw), (bt + bh).min(ct + ch));
    if right - left < 1.0 || bottom - top < 1.0 {
        return None;
    }
    let point = [
        x.min(right - 0.5).max(left + 0.5),
        y.min(bottom - 0.5).max(top + 0.5),
    ];
    let axis = |v: f64, start: f64, size: f64| {
        if v < start {
            -1
        } else if v > start + size {
            1
        } else {
            0
        }
    };
    Some(DragPoint {
        point,
        edge: [axis(x, cl, cw), axis(y, ct, ch)],
        visible: contains(node, point[0], point[1], false),
    })
}

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod tests;
