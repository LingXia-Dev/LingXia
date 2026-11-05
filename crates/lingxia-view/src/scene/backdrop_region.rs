use crate::scene::node::ClipSnapshot;
use vello_cpu::{
    filter_effects::{Filter, FilterFunction},
    kurbo::{Affine, Rect},
};

/// Pixel-aligned capture rectangle; the original fractional/rounded clip is
/// applied only after filtering. Mirrors are taken around this raster boundary.
///
/// A wide blur is low-frequency, so it is filtered at a fraction of the capture's resolution
/// and stretched back bilinearly: cost falls with the square of [`Region::scale`].
pub(crate) struct Region {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    /// Capture pixels per filtered pixel: 1 up to a 2 px blur, at most 4.
    pub scale: f32,
    /// The filtered image, `width` × `height` divided by `scale`.
    pub filtered_width: u16,
    pub filtered_height: u16,
    /// Blur at the filtered resolution.
    pub filter: Filter,
    /// Where the filter's padded input is painted, in filtered pixels.
    pub sampling_bounds: Rect,
}

/// The smallest blur, in filtered pixels, a reduced resolution leaves.
const MIN_FILTERED_BLUR: f32 = 2.0;
/// The coarsest reduction.
const MAX_SCALE: f32 = 4.0;

/// Capture pixels per filtered pixel for a blur of `blur` device pixels.
pub(crate) fn filter_scale(blur: f32) -> f32 {
    (blur / MIN_FILTERED_BLUR).clamp(1.0, MAX_SCALE)
}

impl Region {
    pub fn new(
        clip: ClipSnapshot,
        width: u16,
        height: u16,
        blur: f32,
    ) -> Result<Option<Self>, String> {
        let [x, y, w, h] = clip.bounds.map(f64::from);
        if w <= 0.0 || h <= 0.0 {
            return Ok(None);
        }
        let x0 = x.clamp(0.0, f64::from(width)).floor() as u16;
        let y0 = y.clamp(0.0, f64::from(height)).floor() as u16;
        let x1 = (x + w).clamp(0.0, f64::from(width)).ceil() as u16;
        let y1 = (y + h).clamp(0.0, f64::from(height)).ceil() as u16;
        if x1 <= x0 || y1 <= y0 {
            return Ok(None);
        }
        let (width, height) = (x1 - x0, y1 - y0);
        let scale = filter_scale(blur);
        let filtered_width = (f32::from(width) / scale).ceil().max(1.0) as u16;
        let filtered_height = (f32::from(height) / scale).ceil().max(1.0) as u16;
        let filter = Filter::from_function(FilterFunction::Blur {
            radius: blur / scale,
        });
        let data = vello_common::filter::FilterData::new(filter.clone(), Affine::IDENTITY);
        let padding = data.source_padding;
        let tile_width = vello_common::tile::Tile::WIDTH;
        let tile_height = vello_common::tile::Tile::HEIGHT;
        let right = filtered_width.div_ceil(tile_width) * tile_width + padding.right;
        let bottom = filtered_height.div_ceil(tile_height) * tile_height + padding.bottom;
        let padded_width = u64::from(right) + u64::from(padding.left);
        let padded_height = u64::from(bottom) + u64::from(padding.top);
        if padded_width > 4096
            || padded_height > 4096
            || padded_width * padded_height > 4 * 1024 * 1024
        {
            return Err("UI_COMPOSITE: padded filter region exceeds limit".into());
        }
        Ok(Some(Self {
            x: x0,
            y: y0,
            width,
            height,
            scale,
            filtered_width,
            filtered_height,
            filter,
            sampling_bounds: Rect::new(
                -f64::from(padding.left),
                -f64::from(padding.top),
                f64::from(right),
                f64::from(bottom),
            ),
        }))
    }

    /// Maps the capture onto the filtered image.
    pub fn downscale(&self) -> Affine {
        Affine::scale(1.0 / f64::from(self.scale))
    }

    /// Places the filtered image over the capture rectangle in the frame.
    pub fn placement(&self) -> Affine {
        Affine::translate((f64::from(self.x), f64::from(self.y)))
            * Affine::scale(f64::from(self.scale))
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(
            f64::from(self.x),
            f64::from(self.y),
            f64::from(self.x + self.width),
            f64::from(self.y + self.height),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_blur_filters_at_a_fraction_that_keeps_two_pixels_of_blur() {
        assert_eq!(filter_scale(0.0), 1.0);
        assert_eq!(filter_scale(2.0), 1.0);
        assert_eq!(filter_scale(6.0), 3.0);
        assert_eq!(filter_scale(72.0), 4.0);
        for blur in [3.0f32, 5.0, 8.0, 20.0] {
            let scale = filter_scale(blur);
            assert!(blur / scale >= MIN_FILTERED_BLUR - f32::EPSILON);
        }
        let clip = ClipSnapshot {
            bounds: [0.0, 0.0, 1000.0, 1500.0],
            radius: 0.0,
            radii: None,
            transform: None,
        };
        let region = Region::new(clip, 1084, 2412, 72.0).unwrap().unwrap();
        assert_eq!((region.filtered_width, region.filtered_height), (250, 375));
        let corner = region.placement() * vello_cpu::kurbo::Point::new(250.0, 375.0);
        assert_eq!((corner.x, corner.y), (1000.0, 1500.0));
    }

    #[test]
    fn capture_snaps_outward_and_bounds_filter_padding_before_allocation() {
        let clip = ClipSnapshot {
            bounds: [16.25, 8.5, 15.5, 20.25],
            radius: 4.0,
            radii: None,
            transform: None,
        };
        let region = Region::new(clip, 96, 96, 20.0).unwrap().unwrap();
        assert_eq!(
            (region.x, region.y, region.width, region.height),
            (16, 8, 16, 21)
        );
        assert!(region.sampling_bounds.x0 < 0.0 && region.sampling_bounds.x1 > 4.0);
        assert_eq!(region.scale, 4.0);
        assert_eq!((region.filtered_width, region.filtered_height), (4, 6));
        assert!(
            Region::new(
                ClipSnapshot {
                    bounds: [16.25, 0.0, 0.0, 20.0],
                    ..clip
                },
                96,
                96,
                20.0
            )
            .unwrap()
            .is_none()
        );
        assert!(
            Region::new(
                ClipSnapshot {
                    bounds: [100.0, 0.0, 20.0, 20.0],
                    ..clip
                },
                96,
                96,
                20.0
            )
            .unwrap()
            .is_none()
        );
        // Unblurred, the filtered image is the full capture and can exceed the budget.
        assert!(
            Region::new(
                ClipSnapshot {
                    bounds: [0.0, 0.0, 2100.0, 2100.0],
                    ..clip
                },
                2100,
                2100,
                0.0
            )
            .is_err()
        );
        // A wide blur is filtered small enough to fit where full resolution would not.
        assert!(
            Region::new(
                ClipSnapshot {
                    bounds: [0.0, 0.0, 2048.0, 2048.0],
                    ..clip
                },
                2048,
                2048,
                256.0
            )
            .unwrap()
            .is_some()
        );
    }
}
