//! DirectComposition tree for one composition-hosted WebView2 surface.
//!
//! Corner rounding is supplied by four "wedge" visuals above the webview
//! visual, not by the rectangle clip alone: DComp clips are not anti-aliased
//! over WebView2's swapchain content, so each owned corner is covered by a
//! premultiplied-alpha SDF wedge (opaque backdrop color outside the arc,
//! anti-aliasing only on the arc itself — the device-frame corner-mask
//! technique, re-hosted inside the compositor). The clip stays as a coarse
//! backstop.

use super::*;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice3, IDCompositionDesktopDevice, IDCompositionRectangleClip,
    IDCompositionTarget, IDCompositionVisual, IDCompositionVisual2,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;

/// Device → surface-HWND target → root visual → webview visual (WebView2's
/// `RootVisualTarget`, bounds-clipped) + four corner-wedge visuals above it.
pub(crate) struct DcompTree {
    device: IDCompositionDesktopDevice,
    d3d_context: ID3D11DeviceContext,
    /// Owns the HWND binding; dropping it detaches the tree.
    _target: IDCompositionTarget,
    root: IDCompositionVisual2,
    webview_visual: IDCompositionVisual2,
    clip: IDCompositionRectangleClip,
    /// Wedge/ring visuals for corners with a nonzero radius, `[tl, tr, br,
    /// bl]`.
    wedges: [Option<IDCompositionVisual2>; 4],
    /// Outline-mode edge hairlines, `[top, right, bottom, left]`.
    edges: [Option<IDCompositionVisual2>; 4],
    /// The `(radii, color, width, height)` the current visuals were built
    /// for (edges depend on the surface dimensions).
    wedge_style: ([i32; 4], u32, i32, i32),
}

impl DcompTree {
    pub(crate) fn new(surface: HWND) -> StdResult<Self> {
        unsafe {
            // BGRA D3D device backing the wedge surfaces (WebView2 supplies
            // its own content). Hardware first, WARP as fallback.
            let (d3d_device, d3d_context) = create_d3d_device()?;
            let dxgi: IDXGIDevice = d3d_device
                .cast()
                .map_err(|err| dcomp_error("IDXGIDevice cast", err))?;
            let device: IDCompositionDesktopDevice = DCompositionCreateDevice3(&dxgi)
                .map_err(|err| dcomp_error("DCompositionCreateDevice3", err))?;
            let target = device
                .CreateTargetForHwnd(surface, true)
                .map_err(|err| dcomp_error("CreateTargetForHwnd", err))?;
            let root = device
                .CreateVisual()
                .map_err(|err| dcomp_error("CreateVisual", err))?;
            let webview_visual = device
                .CreateVisual()
                .map_err(|err| dcomp_error("CreateVisual", err))?;
            let clip = device
                .CreateRectangleClip()
                .map_err(|err| dcomp_error("CreateRectangleClip", err))?;
            root.AddVisual(&webview_visual, false, None::<&IDCompositionVisual>)
                .map_err(|err| dcomp_error("AddVisual", err))?;
            target
                .SetRoot(&root)
                .map_err(|err| dcomp_error("SetRoot", err))?;
            device.Commit().map_err(|err| dcomp_error("Commit", err))?;
            Ok(Self {
                device,
                d3d_context,
                _target: target,
                root,
                webview_visual,
                clip,
                wedges: [const { None }; 4],
                edges: [const { None }; 4],
                wedge_style: ([0; 4], 0, 0, 0),
            })
        }
    }

    pub(crate) fn webview_visual(&self) -> &IDCompositionVisual2 {
        &self.webview_visual
    }

    /// Applies the bounds clip and the corner visuals for
    /// `(0, 0, width, height)`, then commits once. `corner_color` is
    /// `0xAARGB` and its alpha selects the corner treatment:
    ///
    /// - `0xFF` — **backdrop wedges**: the webview keeps a square bounds
    ///   clip and each owned corner is covered by an opaque wedge blending
    ///   the anti-aliased arc over live content (a rounded clip would also
    ///   clip the wedges — their useful pixels live exactly outside the arc
    ///   — and its cut edge is aliased anyway). Used where the corner sits
    ///   on a known solid backdrop (shell gutter, device bezel).
    /// - `0x01..=0xFE` — **outline**: the webview clip itself rounds at
    ///   `radii` (aliased) and a hairline outline — corner rings plus edge
    ///   lines in the color at that alpha — covers the cut edge. The corner
    ///   exterior stays fully transparent, for frameless surfaces over
    ///   arbitrary backdrops (the frameless runner screen).
    /// - `0x00` — square clip, no visuals.
    pub(crate) fn apply_geometry(
        &mut self,
        width: i32,
        height: i32,
        radii: [i32; 4],
        corner_color: u32,
    ) -> StdResult<()> {
        let alpha = corner_color >> 24;
        let outline = alpha > 0 && alpha < 0xff;
        let clip_radii = if outline {
            radii.map(|radius| radius.max(0) as f32)
        } else {
            [0.0; 4]
        };
        unsafe {
            self.clip
                .SetLeft2(0.0)
                .and_then(|_| self.clip.SetTop2(0.0))
                .and_then(|_| self.clip.SetRight2(width.max(0) as f32))
                .and_then(|_| self.clip.SetBottom2(height.max(0) as f32))
                .and_then(|_| self.clip.SetTopLeftRadiusX2(clip_radii[0]))
                .and_then(|_| self.clip.SetTopLeftRadiusY2(clip_radii[0]))
                .and_then(|_| self.clip.SetTopRightRadiusX2(clip_radii[1]))
                .and_then(|_| self.clip.SetTopRightRadiusY2(clip_radii[1]))
                .and_then(|_| self.clip.SetBottomRightRadiusX2(clip_radii[2]))
                .and_then(|_| self.clip.SetBottomRightRadiusY2(clip_radii[2]))
                .and_then(|_| self.clip.SetBottomLeftRadiusX2(clip_radii[3]))
                .and_then(|_| self.clip.SetBottomLeftRadiusY2(clip_radii[3]))
                .and_then(|_| self.webview_visual.SetClip(&self.clip))
                .map_err(|err| dcomp_error("clip update", err))?;
        }
        self.update_corner_visuals(width, height, radii, corner_color, outline)?;
        unsafe {
            self.device
                .Commit()
                .map_err(|err| dcomp_error("Commit", err))
        }
    }

    /// Rebuilds the corner (and, in outline mode, edge) visuals when the
    /// style or dimensions changed, then repositions them.
    fn update_corner_visuals(
        &mut self,
        width: i32,
        height: i32,
        radii: [i32; 4],
        corner_color: u32,
        outline: bool,
    ) -> StdResult<()> {
        let disabled = corner_color >> 24 == 0;
        // Edge hairlines span between the corner arcs, so outline visuals
        // depend on the dimensions too; wedge mode only depends on style.
        let style = if outline {
            (radii, corner_color, width, height)
        } else {
            (radii, corner_color, 0, 0)
        };
        if self.wedge_style != style {
            self.wedge_style = style;
            for (corner, radius) in radii.into_iter().enumerate() {
                if let Some(visual) = self.wedges[corner].take() {
                    unsafe {
                        let _ = self.root.RemoveVisual(&visual);
                    }
                }
                if disabled || radius <= 0 {
                    continue;
                }
                let size = radius;
                let pixels = if outline {
                    ring_pixels(corner, radius, corner_color)
                } else {
                    wedge_pixels(corner, radius, corner_color)
                };
                self.wedges[corner] = Some(self.create_pixel_visual(size, size, &pixels)?);
            }
            for slot in &mut self.edges {
                if let Some(visual) = slot.take() {
                    unsafe {
                        let _ = self.root.RemoveVisual(&visual);
                    }
                }
            }
            if outline && !disabled {
                let [tl, tr, br, bl] = radii.map(|radius| radius.max(0));
                // [top, right, bottom, left], each between its corner arcs.
                let spans = [
                    (width - tl - tr, 1),
                    (1, height - tr - br),
                    (width - bl - br, 1),
                    (1, height - tl - bl),
                ];
                for (edge, (edge_width, edge_height)) in spans.into_iter().enumerate() {
                    if edge_width <= 0 || edge_height <= 0 {
                        continue;
                    }
                    let pixels =
                        edge_pixels(edge_width as usize * edge_height as usize, corner_color);
                    self.edges[edge] =
                        Some(self.create_pixel_visual(edge_width, edge_height, &pixels)?);
                }
            }
        }
        for (corner, slot) in self.wedges.iter().enumerate() {
            let Some(visual) = slot else { continue };
            let radius = radii[corner];
            let (x, y) = match corner {
                0 => (0, 0),
                1 => (width - radius, 0),
                2 => (width - radius, height - radius),
                _ => (0, height - radius),
            };
            unsafe {
                visual
                    .SetOffsetX2(x as f32)
                    .and_then(|_| visual.SetOffsetY2(y as f32))
                    .map_err(|err| dcomp_error("wedge offset", err))?;
            }
        }
        let [tl, tr, _br, bl] = radii.map(|radius| radius.max(0));
        let edge_offsets = [(tl, 0), (width - 1, tr), (bl, height - 1), (0, tl)];
        for (edge, slot) in self.edges.iter().enumerate() {
            let Some(visual) = slot else { continue };
            let (x, y) = edge_offsets[edge];
            unsafe {
                visual
                    .SetOffsetX2(x as f32)
                    .and_then(|_| visual.SetOffsetY2(y as f32))
                    .map_err(|err| dcomp_error("edge offset", err))?;
            }
        }
        Ok(())
    }

    /// A visual above the webview visual showing the given premultiplied
    /// BGRA pixels.
    fn create_pixel_visual(
        &self,
        width: i32,
        height: i32,
        pixels: &[u32],
    ) -> StdResult<IDCompositionVisual2> {
        unsafe {
            let surface = self
                .device
                .CreateSurface(
                    width as u32,
                    height as u32,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_ALPHA_MODE_PREMULTIPLIED,
                )
                .map_err(|err| dcomp_error("CreateSurface", err))?;
            let mut offset = windows::Win32::Foundation::POINT::default();
            let texture: ID3D11Texture2D = surface
                .BeginDraw(None, &mut offset)
                .map_err(|err| dcomp_error("BeginDraw", err))?;
            self.d3d_context.UpdateSubresource(
                &texture,
                0,
                Some(&D3D11_BOX {
                    left: offset.x as u32,
                    top: offset.y as u32,
                    front: 0,
                    right: (offset.x + width) as u32,
                    bottom: (offset.y + height) as u32,
                    back: 1,
                }),
                pixels.as_ptr() as *const _,
                (width * 4) as u32,
                0,
            );
            surface
                .EndDraw()
                .map_err(|err| dcomp_error("EndDraw", err))?;
            let visual = self
                .device
                .CreateVisual()
                .map_err(|err| dcomp_error("CreateVisual", err))?;
            visual
                .SetContent(&surface)
                .map_err(|err| dcomp_error("SetContent", err))?;
            self.root
                .AddVisual(&visual, true, &self.webview_visual)
                .map_err(|err| dcomp_error("AddVisual", err))?;
            Ok(visual)
        }
    }
}

/// Premultiplied BGRA wedge bitmap: alpha = 1 − arc coverage (4×4
/// supersampled, matching the GDI+ card arcs), colored `0xAARGB` shaded by
/// the same translucent shadow rings `draw_content_card` paints around the
/// workspace card — a flat backdrop would read as a bright patch against
/// the shadowed gutter. Arc centers per corner index `[tl, tr, br, bl]` sit
/// at the wedge-local corner farthest into the content.
fn wedge_pixels(corner: usize, radius: i32, color: u32) -> Vec<u32> {
    let (center_x, center_y) = match corner {
        0 => (radius, radius),
        1 => (0, radius),
        2 => (0, 0),
        _ => (radius, 0),
    };
    let alpha = (color >> 24) & 0xff;
    let (red, green, blue) = ((color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff);
    let mut pixels = Vec::with_capacity((radius * radius) as usize);
    for y in 0..radius {
        for x in 0..radius {
            let mut hits = 0u32;
            for sub_y in 0..4 {
                for sub_x in 0..4 {
                    let dx = x as f32 + (sub_x as f32 + 0.5) / 4.0 - center_x as f32;
                    let dy = y as f32 + (sub_y as f32 + 0.5) / 4.0 - center_y as f32;
                    if dx * dx + dy * dy <= (radius * radius) as f32 {
                        hits += 1;
                    }
                }
            }
            let inside = hits as f32 / 16.0;
            // The card shadow: rings of radius+spread with a +2px vertical
            // offset (draw_content_card's layered expansions).
            let dx = x as f32 + 0.5 - center_x as f32;
            let dy = y as f32 + 0.5 - (center_y as f32 + 2.0);
            let shadow_distance = (dx * dx + dy * dy).sqrt();
            let mut keep = 1.0f32;
            for spread in 1..=8 {
                if shadow_distance <= (radius + spread) as f32 {
                    let ring_alpha = if spread <= 2 { 10.0 } else { 5.0 };
                    keep *= 1.0 - ring_alpha / 255.0;
                }
            }
            let coverage = ((1.0 - inside) * alpha as f32) as u32;
            let shaded = |channel: u32| (channel as f32 * keep) as u32;
            let premultiply = |channel: u32| (channel * coverage + 127) / 255;
            pixels.push(
                (coverage << 24)
                    | (premultiply(shaded(red)) << 16)
                    | (premultiply(shaded(green)) << 8)
                    | premultiply(shaded(blue)),
            );
        }
    }
    pixels
}

/// Premultiplied BGRA corner-ring bitmap for outline mode: a ~2px
/// anti-aliased arc band centered on the clip radius, transparent on both
/// sides. It covers the rounded clip's aliased cut edge; the exterior stays
/// see-through for frameless surfaces over arbitrary backdrops.
fn ring_pixels(corner: usize, radius: i32, color: u32) -> Vec<u32> {
    let (center_x, center_y) = match corner {
        0 => (radius, radius),
        1 => (0, radius),
        2 => (0, 0),
        _ => (radius, 0),
    };
    let alpha = (color >> 24) & 0xff;
    let (red, green, blue) = ((color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff);
    let (inner, outer) = (radius as f32 - 1.0, radius as f32 + 1.0);
    let mut pixels = Vec::with_capacity((radius * radius) as usize);
    for y in 0..radius {
        for x in 0..radius {
            let mut hits = 0u32;
            for sub_y in 0..4 {
                for sub_x in 0..4 {
                    let dx = x as f32 + (sub_x as f32 + 0.5) / 4.0 - center_x as f32;
                    let dy = y as f32 + (sub_y as f32 + 0.5) / 4.0 - center_y as f32;
                    let distance = (dx * dx + dy * dy).sqrt();
                    if distance >= inner && distance <= outer {
                        hits += 1;
                    }
                }
            }
            let coverage = hits * alpha / 16;
            let premultiply = |channel: u32| (channel * coverage + 127) / 255;
            pixels.push(
                (coverage << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue),
            );
        }
    }
    pixels
}

/// Uniform premultiplied hairline pixels for the outline's straight edges.
fn edge_pixels(count: usize, color: u32) -> Vec<u32> {
    let alpha = (color >> 24) & 0xff;
    let premultiply = |channel: u32| (channel * alpha + 127) / 255;
    let pixel = (alpha << 24)
        | (premultiply((color >> 16) & 0xff) << 16)
        | (premultiply((color >> 8) & 0xff) << 8)
        | premultiply(color & 0xff);
    vec![pixel; count]
}

fn create_d3d_device() -> StdResult<(ID3D11Device, ID3D11DeviceContext)> {
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        let mut context = None;
        let created = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        if created.is_ok()
            && let (Some(device), Some(context)) = (device, context)
        {
            return Ok((device, context));
        }
        log::warn!("D3D11CreateDevice({driver:?}) failed; trying next driver");
    }
    Err(WebViewError::WebView(
        "no D3D11 device available for composition wedges".to_string(),
    ))
}

fn dcomp_error(what: &str, err: windows::core::Error) -> WebViewError {
    WebViewError::WebView(format!("{what} failed: {err}"))
}
