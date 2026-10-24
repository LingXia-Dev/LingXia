use crate::render::ClipSnapshot;
use vello_cpu::{
    filter_effects::{Filter, FilterFunction},
    kurbo::{Affine, Rect},
};

/// Pixel-aligned capture rectangle; the original fractional/rounded clip is
/// applied only after filtering. Mirrors are taken around this raster boundary.
pub(crate) struct Region {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub filter: Filter,
    pub sampling_bounds: Rect,
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
        let filter = Filter::from_function(FilterFunction::Blur { radius: blur });
        let data = vello_common::filter::FilterData::new(filter.clone(), Affine::IDENTITY);
        let padding = data.source_padding;
        let tile_width = vello_common::tile::Tile::WIDTH;
        let tile_height = vello_common::tile::Tile::HEIGHT;
        let right = width.div_ceil(tile_width) * tile_width + padding.right;
        let bottom = height.div_ceil(tile_height) * tile_height + padding.bottom;
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
            filter,
            sampling_bounds: Rect::new(
                -f64::from(padding.left),
                -f64::from(padding.top),
                f64::from(right),
                f64::from(bottom),
            ),
        }))
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
    fn capture_snaps_outward_and_bounds_filter_padding_before_allocation() {
        let clip = ClipSnapshot {
            bounds: [16.25, 8.5, 15.5, 20.25],
            radius: 4.0,
            radii: None,
        };
        let region = Region::new(clip, 96, 96, 20.0).unwrap().unwrap();
        assert_eq!(
            (region.x, region.y, region.width, region.height),
            (16, 8, 16, 21)
        );
        assert!(region.sampling_bounds.x0 < 0.0 && region.sampling_bounds.x1 > 16.0);
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
            .is_err()
        );
    }
}
