//! What must hold for any buffer the decoder admits.
#![allow(dead_code)]

use lingxia_ui::{Renderer, protocol::decode};
use serde_json::Value;

/// Decodes `wire` and, when admitted, lays it out and checks the frame. Rejections are
/// fine; a panic or a broken frame is the bug.
pub fn check_wire(wire: &[Value], width: u16, height: u16, scale: f32) -> Result<bool, String> {
    let Ok(records) = decode(wire) else {
        return Ok(false);
    };
    // Font discovery dominates a fresh renderer; one per thread keeps runs fast.
    thread_local!(static RENDERER: std::cell::RefCell<Renderer> = std::cell::RefCell::new(Renderer::default()));
    let Ok(frame) =
        RENDERER.with(|renderer| renderer.borrow_mut().layout(&records, width, height, scale))
    else {
        return Ok(true);
    };
    let (w, h) = (f32::from(width) / scale, f32::from(height) / scale);
    for (index, node) in frame.nodes.iter().enumerate() {
        let [x, y, nw, nh] = node.bounds;
        if !(x.is_finite() && y.is_finite() && nw.is_finite() && nh.is_finite())
            || nw < 0.0
            || nh < 0.0
        {
            return Err(format!(
                "node {index} <{}> bounds {:?}",
                node.tag, node.bounds
            ));
        }
        let [vx, vy, vw, vh] = node.visible_bounds;
        let slack = 0.5;
        if !(vx.is_finite() && vy.is_finite() && vw.is_finite() && vh.is_finite())
            || vw < 0.0
            || vh < 0.0
            || (vw > 0.0
                && vh > 0.0
                && (vx < -slack || vy < -slack || vx + vw > w + slack || vy + vh > h + slack))
        {
            return Err(format!(
                "node {index} <{}> visible bounds {:?} outside {w}x{h}",
                node.tag, node.visible_bounds
            ));
        }
    }
    let cost = frame.cost();
    if cost.nodes != frame.nodes.len() {
        return Err("cost disagrees with the frame".into());
    }
    if u32::from(width) * u32::from(height) <= 64 * 64 * 4 {
        frame
            .rasterize()
            .map_err(|error| format!("rasterize: {error}"))?;
    }
    Ok(true)
}
