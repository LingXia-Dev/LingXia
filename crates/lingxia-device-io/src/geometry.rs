//! Map canonical capture geometry + normalized content coordinates into
//! device input. Owned here so remote input can stay generation-safe without
//! taking the media pipeline.

use lingxia_platform::capture::{CaptureError, CaptureSessionId, Rect, VisualGeometry};

/// Translate a normalized content point (0..1, origin top-left) into platform
/// global device coordinates for the frame the caller acted on.
pub fn map_normalized_pointer(
    geometry: &VisualGeometry,
    nx: f64,
    ny: f64,
    session_id: CaptureSessionId,
    generation: u64,
) -> Result<(i32, i32), CaptureError> {
    if geometry.session_id != session_id {
        return Err(CaptureError::UnknownSession);
    }
    if geometry.generation != generation {
        return Err(CaptureError::StaleGeneration);
    }
    if !(0.0..=1.0).contains(&nx) || !(0.0..=1.0).contains(&ny) {
        return Err(CaptureError::InvalidRequest(
            "normalized coordinates must be in 0..=1".into(),
        ));
    }
    if geometry.content_in_output.width <= 0 || geometry.content_in_output.height <= 0 {
        return Err(CaptureError::InvalidRequest(
            "content rectangle is empty".into(),
        ));
    }
    if geometry.source.width <= 0 || geometry.source.height <= 0 {
        return Err(CaptureError::InvalidRequest(
            "source rectangle is empty".into(),
        ));
    }

    let content_x =
        f64::from(geometry.content_in_output.x) + nx * f64::from(geometry.content_in_output.width);
    let content_y =
        f64::from(geometry.content_in_output.y) + ny * f64::from(geometry.content_in_output.height);

    let (unrotated_x, unrotated_y) = inverse_output_transform(
        content_x,
        content_y,
        geometry.output.width,
        geometry.output.height,
        geometry.rotation_degrees,
        geometry.mirrored,
    );
    let (content_origin_x, content_origin_y) = inverse_output_transform(
        f64::from(geometry.content_in_output.x),
        f64::from(geometry.content_in_output.y),
        geometry.output.width,
        geometry.output.height,
        geometry.rotation_degrees,
        geometry.mirrored,
    );
    let (content_far_x, content_far_y) = inverse_output_transform(
        f64::from(geometry.content_in_output.x + geometry.content_in_output.width),
        f64::from(geometry.content_in_output.y + geometry.content_in_output.height),
        geometry.output.width,
        geometry.output.height,
        geometry.rotation_degrees,
        geometry.mirrored,
    );
    let unrotated_w = (content_far_x - content_origin_x).abs().max(1.0);
    let unrotated_h = (content_far_y - content_origin_y).abs().max(1.0);
    let rel_x = (unrotated_x - content_origin_x.min(content_far_x)) / unrotated_w;
    let rel_y = (unrotated_y - content_origin_y.min(content_far_y)) / unrotated_h;

    let global_x = f64::from(geometry.source.x) + rel_x * f64::from(geometry.source.width);
    let global_y = f64::from(geometry.source.y) + rel_y * f64::from(geometry.source.height);
    Ok((global_x.round() as i32, global_y.round() as i32))
}

fn inverse_output_transform(
    x: f64,
    y: f64,
    output_w: u32,
    output_h: u32,
    rotation_degrees: i32,
    mirrored: bool,
) -> (f64, f64) {
    let cx = f64::from(output_w) / 2.0;
    let cy = f64::from(output_h) / 2.0;
    let dx = x - cx;
    let dy = y - cy;
    let rot = ((rotation_degrees % 360) + 360) % 360;
    let (dx, dy) = match rot {
        90 => (dy, -dx),
        180 => (-dx, -dy),
        270 => (-dy, dx),
        _ => (dx, dy),
    };
    let dx = if mirrored { -dx } else { dx };
    (cx + dx, cy + dy)
}

/// Identity geometry used by tests and as a starting template for adapters.
pub fn identity_geometry(
    session_id: CaptureSessionId,
    generation: u64,
    source: Rect,
) -> VisualGeometry {
    VisualGeometry {
        session_id,
        generation,
        source,
        target_local: Rect {
            x: 0,
            y: 0,
            width: source.width,
            height: source.height,
        },
        output: lingxia_platform::capture::Size {
            width: source.width.max(0) as u32,
            height: source.height.max(0) as u32,
        },
        content_in_output: Rect {
            x: 0,
            y: 0,
            width: source.width,
            height: source.height,
        },
        scale: 1.0,
        rotation_degrees: 0,
        mirrored: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_platform::capture::Size;

    fn session() -> CaptureSessionId {
        CaptureSessionId(7)
    }

    fn source(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn identity_maps_corners_and_center() {
        let geometry = identity_geometry(session(), 1, source(10, 20, 200, 100));
        assert_eq!(
            map_normalized_pointer(&geometry, 0.0, 0.0, session(), 1).unwrap(),
            (10, 20)
        );
        assert_eq!(
            map_normalized_pointer(&geometry, 1.0, 1.0, session(), 1).unwrap(),
            (210, 120)
        );
        assert_eq!(
            map_normalized_pointer(&geometry, 0.5, 0.5, session(), 1).unwrap(),
            (110, 70)
        );
    }

    #[test]
    fn negative_origin_multi_monitor_is_preserved() {
        let geometry = identity_geometry(session(), 3, source(-1920, 0, 1920, 1080));
        assert_eq!(
            map_normalized_pointer(&geometry, 0.0, 0.0, session(), 3).unwrap(),
            (-1920, 0)
        );
        assert_eq!(
            map_normalized_pointer(&geometry, 1.0, 0.0, session(), 3).unwrap(),
            (0, 0)
        );
    }

    #[test]
    fn mixed_dpi_scale_does_not_change_normalized_mapping() {
        let mut geometry = identity_geometry(session(), 1, source(0, 0, 800, 400));
        geometry.scale = 2.0;
        assert_eq!(
            map_normalized_pointer(&geometry, 0.25, 0.5, session(), 1).unwrap(),
            (200, 200)
        );
    }

    #[test]
    fn letterbox_uses_the_content_rect_not_the_output() {
        let mut geometry = identity_geometry(session(), 1, source(0, 0, 200, 100));
        geometry.output = Size {
            width: 200,
            height: 200,
        };
        geometry.content_in_output = Rect {
            x: 0,
            y: 50,
            width: 200,
            height: 100,
        };
        assert_eq!(
            map_normalized_pointer(&geometry, 0.0, 0.0, session(), 1).unwrap(),
            (0, 0)
        );
        assert_eq!(
            map_normalized_pointer(&geometry, 1.0, 1.0, session(), 1).unwrap(),
            (200, 100)
        );
    }

    #[test]
    fn crop_maps_through_the_visible_content() {
        let mut geometry = identity_geometry(session(), 1, source(100, 50, 400, 300));
        geometry.output = Size {
            width: 200,
            height: 150,
        };
        geometry.content_in_output = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 150,
        };
        assert_eq!(
            map_normalized_pointer(&geometry, 0.0, 0.0, session(), 1).unwrap(),
            (100, 50)
        );
        assert_eq!(
            map_normalized_pointer(&geometry, 1.0, 1.0, session(), 1).unwrap(),
            (500, 350)
        );
    }

    #[test]
    fn rotation_180_maps_the_opposite_corner() {
        let mut geometry = identity_geometry(session(), 1, source(0, 0, 100, 80));
        geometry.rotation_degrees = 180;
        assert_eq!(
            map_normalized_pointer(&geometry, 0.0, 0.0, session(), 1).unwrap(),
            (100, 80)
        );
        assert_eq!(
            map_normalized_pointer(&geometry, 1.0, 1.0, session(), 1).unwrap(),
            (0, 0)
        );
    }

    #[test]
    fn stale_generation_and_unknown_session_are_rejected() {
        let geometry = identity_geometry(session(), 4, source(0, 0, 10, 10));
        assert!(matches!(
            map_normalized_pointer(&geometry, 0.5, 0.5, session(), 3),
            Err(CaptureError::StaleGeneration)
        ));
        assert!(matches!(
            map_normalized_pointer(&geometry, 0.5, 0.5, CaptureSessionId(99), 4),
            Err(CaptureError::UnknownSession)
        ));
    }

    #[test]
    fn a_moved_window_is_a_new_generation() {
        let before = identity_geometry(session(), 1, source(0, 0, 200, 100));
        let after = identity_geometry(session(), 2, source(80, 40, 200, 100));
        assert!(matches!(
            map_normalized_pointer(&after, 0.0, 0.0, session(), 1),
            Err(CaptureError::StaleGeneration)
        ));
        assert_eq!(
            map_normalized_pointer(&after, 0.0, 0.0, session(), 2).unwrap(),
            (80, 40)
        );
        assert_eq!(
            map_normalized_pointer(&before, 0.0, 0.0, session(), 1).unwrap(),
            (0, 0)
        );
    }
}
