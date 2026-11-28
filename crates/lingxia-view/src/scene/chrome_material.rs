//! The material under a tab bar that floats over the page: the page beneath, blurred and
//! tinted with the page theme's `surface`. The core draws it so the bar takes the page's own
//! theme on every platform; the host draws only the bar's items over it.
use crate::platform::ChromeShape;
use crate::scene::node::ClipSnapshot;
use crate::scene::paint::Paint;
use vello_cpu::kurbo::Affine;

/// Blur radius in logical pixels.
const BLUR: f32 = 24.0;
/// Colour kept vivid through the blur, so the content under the bar still reads as its own.
const SATURATION: f32 = 1.4;
/// How much `surface` covers the blurred page: the least that keeps the bar's items at 4.5:1
/// over a mid-grey page and over the appearance's own white or black, so content reads through.
const TINT_ALPHA_LIGHT: u8 = 0xCC;
const TINT_ALPHA_DARK: u8 = 0xC0;

/// Paints for the tab bar over a `width` x `height` page (logical pixels), or none. A host that
/// reports the bar's `shape` gets the material in it (a floating pill gets a pill); otherwise
/// the band the bottom inset covers past the system area, full width.
pub(crate) fn paints(
    surface: [u8; 4],
    dark: bool,
    [width, height]: [f32; 2],
    shape: Option<ChromeShape>,
    insets_bottom: f32,
    safe_area_bottom: f32,
) -> Vec<Paint> {
    let (bounds, radius) = match shape {
        Some(ChromeShape {
            rect: [x, y, w, h],
            radius,
        }) => {
            let (left, top) = (x.max(0.0), y.max(0.0));
            let (right, bottom) = ((x + w).min(width), (y + h).min(height));
            if !(right > left && bottom > top) {
                return Vec::new();
            }
            ([left, top, right - left, bottom - top], radius)
        }
        None => {
            let band = insets_bottom.min(height);
            if !(band > safe_area_bottom && band > 0.0 && width > 0.0) {
                return Vec::new();
            }
            ([0.0, height - band, width, band], 0.0)
        }
    };
    let [r, g, b, _] = surface;
    let alpha = if dark {
        TINT_ALPHA_DARK
    } else {
        TINT_ALPHA_LIGHT
    };
    vec![
        Paint::Transform(Affine::IDENTITY),
        Paint::Backdrop {
            node: 0,
            clip: ClipSnapshot {
                bounds,
                radius,
                radii: None,
                transform: None,
            },
            blur: BLUR,
            saturation: SATURATION,
            opacity: 1.0,
        },
        Paint::Rect {
            color: [r, g, b, alpha],
            bounds,
            radius,
            corners: None,
        },
        Paint::PopBackdrop,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: [u8; 4] = [250, 250, 250, 255];
    const PAGE: [f32; 2] = [400.0, 800.0];

    /// The material's clip and tint: bounds and radius of each.
    fn drawn(paints: &[Paint]) -> Option<([f32; 4], f32, [f32; 4], f32)> {
        match paints {
            [
                Paint::Transform(_),
                Paint::Backdrop { clip, .. },
                Paint::Rect {
                    color: [250, 250, 250, TINT_ALPHA_LIGHT],
                    bounds,
                    radius,
                    ..
                },
                Paint::PopBackdrop,
            ] => Some((clip.bounds, clip.radius, *bounds, *radius)),
            _ => None,
        }
    }

    #[test]
    fn without_a_shape_the_bottom_chrome_band_gets_a_full_width_material() {
        assert!(
            paints(SURFACE, false, PAGE, None, 34.0, 34.0).is_empty(),
            "the home indicator alone"
        );
        assert!(paints(SURFACE, false, PAGE, None, 0.0, 0.0).is_empty());
        let band = [0.0, 717.0, 400.0, 83.0];
        assert_eq!(
            drawn(&paints(SURFACE, false, PAGE, None, 83.0, 34.0)),
            Some((band, 0.0, band, 0.0)),
            "the bar and the system area under it"
        );
    }

    #[test]
    fn a_floating_pill_gets_a_pill_and_no_band() {
        let pill = ChromeShape {
            rect: [60.0, 720.0, 280.0, 56.0],
            radius: 28.0,
        };
        let rect = pill.rect;
        assert_eq!(
            drawn(&paints(SURFACE, false, PAGE, Some(pill), 100.0, 20.0)),
            Some((rect, 28.0, rect, 28.0)),
            "the reported shape wins over the inset band"
        );
        let past_the_edge = ChromeShape {
            rect: [-10.0, 780.0, 420.0, 40.0],
            radius: 0.0,
        };
        assert_eq!(
            drawn(&paints(
                SURFACE,
                false,
                PAGE,
                Some(past_the_edge),
                20.0,
                0.0
            )),
            Some((
                [0.0, 780.0, 400.0, 20.0],
                0.0,
                [0.0, 780.0, 400.0, 20.0],
                0.0
            )),
            "clipped to the page"
        );
        let off_page = ChromeShape {
            rect: [0.0, 900.0, 400.0, 40.0],
            radius: 0.0,
        };
        assert!(paints(SURFACE, false, PAGE, Some(off_page), 40.0, 0.0).is_empty());
    }

    #[test]
    fn the_bar_items_keep_their_contrast_over_mid_grey_and_their_own_ground() {
        use lingxia_theme::{Rgba, Role, contrast, over};
        let looks = lingxia_theme::looks(None);
        for (dark, alpha) in [(false, TINT_ALPHA_LIGHT), (true, TINT_ALPHA_DARK)] {
            let [r, g, b, _] = looks.role(Role::Surface, dark, false).0;
            let own = if dark { [0, 0, 0, 255] } else { [255; 4] };
            for page in [own, [128, 128, 128, 255]] {
                let ground = over(Rgba([r, g, b, alpha]), Rgba(page));
                for item in [Role::TextMuted, Role::Accent, Role::Text] {
                    let ratio = contrast(looks.role(item, dark, false), ground);
                    assert!(
                        ratio >= 4.5,
                        "{item:?} at {ratio:.2}:1 over {page:?}, dark {dark}"
                    );
                }
            }
        }
    }
}
