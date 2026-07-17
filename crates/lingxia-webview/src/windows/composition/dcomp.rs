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

/// Device → surface-HWND target → root visual (clip) → webview visual
/// (WebView2's `RootVisualTarget`) + four corner-wedge visuals above it.
pub(crate) struct DcompTree {
    device: IDCompositionDesktopDevice,
    d3d_context: ID3D11DeviceContext,
    /// Owns the HWND binding; dropping it detaches the tree.
    _target: IDCompositionTarget,
    root: IDCompositionVisual2,
    webview_visual: IDCompositionVisual2,
    clip: IDCompositionRectangleClip,
    /// Wedge visuals for corners with a nonzero radius, `[tl, tr, br, bl]`.
    wedges: [Option<IDCompositionVisual2>; 4],
    /// The `(radii, color)` the current wedge set was rendered for.
    wedge_style: ([i32; 4], u32),
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
                wedge_style: ([0; 4], 0),
            })
        }
    }

    pub(crate) fn webview_visual(&self) -> &IDCompositionVisual2 {
        &self.webview_visual
    }

    /// Applies the clip and the corner wedges for `(0, 0, width, height)`
    /// with per-corner radii `[tl, tr, br, bl]`, then commits once.
    /// `corner_color` is the 0xAARGB backdrop the wedges paint outside the
    /// arc; alpha 0 disables wedges (a separate mask owns the corners, e.g.
    /// the device frame's bezel ring).
    pub(crate) fn apply_geometry(
        &mut self,
        width: i32,
        height: i32,
        radii: [i32; 4],
        corner_color: u32,
    ) -> StdResult<()> {
        let [tl, tr, br, bl] = radii.map(|radius| radius.max(0) as f32);
        unsafe {
            self.clip
                .SetLeft2(0.0)
                .and_then(|_| self.clip.SetTop2(0.0))
                .and_then(|_| self.clip.SetRight2(width.max(0) as f32))
                .and_then(|_| self.clip.SetBottom2(height.max(0) as f32))
                .and_then(|_| self.clip.SetTopLeftRadiusX2(tl))
                .and_then(|_| self.clip.SetTopLeftRadiusY2(tl))
                .and_then(|_| self.clip.SetTopRightRadiusX2(tr))
                .and_then(|_| self.clip.SetTopRightRadiusY2(tr))
                .and_then(|_| self.clip.SetBottomRightRadiusX2(br))
                .and_then(|_| self.clip.SetBottomRightRadiusY2(br))
                .and_then(|_| self.clip.SetBottomLeftRadiusX2(bl))
                .and_then(|_| self.clip.SetBottomLeftRadiusY2(bl))
                .and_then(|_| self.root.SetClip(&self.clip))
                .map_err(|err| dcomp_error("clip update", err))?;
        }
        self.update_wedges(width, height, radii, corner_color)?;
        unsafe {
            self.device
                .Commit()
                .map_err(|err| dcomp_error("Commit", err))
        }
    }

    /// Rebuilds wedge visuals when the style changed, then repositions them
    /// at the current corners.
    fn update_wedges(
        &mut self,
        width: i32,
        height: i32,
        radii: [i32; 4],
        corner_color: u32,
    ) -> StdResult<()> {
        let disabled = corner_color >> 24 == 0;
        let style = (radii, corner_color);
        if self.wedge_style != style {
            self.wedge_style = style;
            for corner in 0..4 {
                if let Some(visual) = self.wedges[corner].take() {
                    unsafe {
                        let _ = self.root.RemoveVisual(&visual);
                    }
                }
                let radius = radii[corner];
                if disabled || radius <= 0 {
                    continue;
                }
                self.wedges[corner] = Some(self.create_wedge(corner, radius, corner_color)?);
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
        Ok(())
    }

    /// One radius×radius wedge visual above the webview visual: opaque
    /// `color` outside the corner arc, transparent inside, AA on the arc.
    fn create_wedge(
        &self,
        corner: usize,
        radius: i32,
        color: u32,
    ) -> StdResult<IDCompositionVisual2> {
        let pixels = wedge_pixels(corner, radius, color);
        unsafe {
            let surface = self
                .device
                .CreateSurface(
                    radius as u32,
                    radius as u32,
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
                    right: (offset.x + radius) as u32,
                    bottom: (offset.y + radius) as u32,
                    back: 1,
                }),
                pixels.as_ptr() as *const _,
                (radius * 4) as u32,
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

/// Premultiplied BGRA wedge bitmap: alpha = 1 − arc coverage, colored
/// `0xAARGB`. Arc centers per corner index `[tl, tr, br, bl]` sit at the
/// wedge-local corner farthest into the content.
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
            let dx = x as f32 + 0.5 - center_x as f32;
            let dy = y as f32 + 0.5 - center_y as f32;
            let inside = (radius as f32 - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
            let coverage = ((1.0 - inside) * alpha as f32) as u32;
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
