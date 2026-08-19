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
use std::collections::HashMap;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
    ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice3, IDCompositionDesktopDevice, IDCompositionDevice,
    IDCompositionRectangleClip, IDCompositionTarget, IDCompositionVisual, IDCompositionVisual2,
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
    /// The `(radii, color)` the current visuals were built for.
    wedge_style: ([i32; 4], u32),
    /// Island nodes attached above [`Self::webview_visual`], keyed by author
    /// / node id. Sibling order is [`Self::sync_island_visuals`].
    island: HashMap<String, IslandVisual>,
}

struct IslandVisual {
    visual: IDCompositionVisual2,
    width: i32,
    height: i32,
    color: u32,
    text: Option<String>,
    hwnd: Option<isize>,
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
                island: HashMap::new(),
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
    ///   `radii` (aliased) and hairline corner arc rings in the color at
    ///   that alpha cover the aliased cut. The corner exterior stays fully
    ///   transparent, for frameless surfaces over arbitrary backdrops (the
    ///   frameless runner screen — the device frame paints the straight
    ///   perimeter hairline, surfaces only patch their own arcs).
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

    /// Rebuilds the corner visuals when the style changed, then repositions
    /// them for the current dimensions.
    fn update_corner_visuals(
        &mut self,
        width: i32,
        height: i32,
        radii: [i32; 4],
        corner_color: u32,
        outline: bool,
    ) -> StdResult<()> {
        let disabled = corner_color >> 24 == 0;
        let style = (radii, corner_color);
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

    /// A visual above the webview visual showing the given premultiplied
    /// BGRA pixels.
    fn create_pixel_visual(
        &self,
        width: i32,
        height: i32,
        pixels: &[u32],
    ) -> StdResult<IDCompositionVisual2> {
        let visual = self.create_color_visual(width, height, pixels)?;
        unsafe {
            self.root
                .AddVisual(&visual, true, &self.webview_visual)
                .map_err(|err| dcomp_error("AddVisual", err))?;
        }
        Ok(visual)
    }

    fn create_color_visual(
        &self,
        width: i32,
        height: i32,
        pixels: &[u32],
    ) -> StdResult<IDCompositionVisual2> {
        let width = width.max(1);
        let height = height.max(1);
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
            Ok(visual)
        }
    }

    fn create_hwnd_visual(&self, hwnd: HWND) -> StdResult<IDCompositionVisual2> {
        unsafe {
            let device: IDCompositionDevice = self
                .device
                .cast()
                .map_err(|err| dcomp_error("IDCompositionDevice cast", err))?;
            let surface = device
                .CreateSurfaceFromHwnd(hwnd)
                .map_err(|err| dcomp_error("CreateSurfaceFromHwnd", err))?;
            let visual = self
                .device
                .CreateVisual()
                .map_err(|err| dcomp_error("CreateVisual", err))?;
            visual
                .SetContent(&surface)
                .map_err(|err| dcomp_error("SetContent hwnd", err))?;
            Ok(visual)
        }
    }

    /// Inserts or updates island visuals immediately above the WebView2
    /// visual, in `specs` order (first = lowest). Wedge visuals stay on top.
    pub(crate) fn sync_island_visuals(&mut self, specs: &[IslandVisualSpec]) -> StdResult<()> {
        let live: std::collections::HashSet<&str> =
            specs.iter().map(|spec| spec.id.as_str()).collect();
        let stale: Vec<String> = self
            .island
            .keys()
            .filter(|id| !live.contains(id.as_str()))
            .cloned()
            .collect();
        for id in stale {
            if let Some(slot) = self.island.remove(&id) {
                unsafe {
                    let _ = self.root.RemoveVisual(&slot.visual);
                }
            }
        }
        for spec in specs {
            log::debug!(
                "island visual {} kind={} {}x{} hwnd={:?}",
                spec.id,
                spec.kind,
                spec.width,
                spec.height,
                spec.hwnd
            );
            self.upsert_island_visual(spec)?;
        }
        for slot in self.island.values() {
            unsafe {
                let _ = self.root.RemoveVisual(&slot.visual);
            }
        }
        let mut previous: Option<IDCompositionVisual2> = None;
        for spec in specs {
            let Some(slot) = self.island.get(&spec.id) else {
                continue;
            };
            unsafe {
                if let Some(ref previous) = previous {
                    self.root
                        .AddVisual(&slot.visual, true, previous)
                        .map_err(|err| dcomp_error("AddVisual island", err))?;
                } else {
                    self.root
                        .AddVisual(&slot.visual, true, &self.webview_visual)
                        .map_err(|err| dcomp_error("AddVisual island", err))?;
                }
            }
            previous = Some(slot.visual.clone());
        }
        // This device is shared with WebView2. Committing from a UI-thread
        // command callback after AddVisual has stalled the page (eval and
        // the dev websocket stop answering). apply_geometry commits later.
        Ok(())
    }

    fn upsert_island_visual(&mut self, spec: &IslandVisualSpec) -> StdResult<()> {
        let width = spec.width.max(1);
        let height = spec.height.max(1);
        let reusable = self.island.get(&spec.id).is_some_and(|slot| {
            slot.width == width
                && slot.height == height
                && slot.color == spec.color
                && slot.text == spec.text
                && slot.hwnd == spec.hwnd
        });
        if reusable {
            if let Some(slot) = self.island.get(&spec.id) {
                unsafe {
                    slot.visual
                        .SetOffsetX2(spec.offset_x)
                        .and_then(|_| slot.visual.SetOffsetY2(spec.offset_y))
                        .map_err(|err| dcomp_error("island offset", err))?;
                }
            }
            return Ok(());
        }
        if let Some(previous) = self.island.remove(&spec.id) {
            unsafe {
                let _ = self.root.RemoveVisual(&previous.visual);
            }
        }
        let visual = if let Some(handle) = spec.hwnd.filter(|handle| *handle != 0) {
            self.create_hwnd_visual(HWND(handle as *mut _))?
        } else {
            let pixels = rasterize_island_pixels(width, height, spec.color, spec.text.as_deref());
            self.create_color_visual(width, height, &pixels)?
        };
        unsafe {
            visual
                .SetOffsetX2(spec.offset_x)
                .and_then(|_| visual.SetOffsetY2(spec.offset_y))
                .map_err(|err| dcomp_error("island offset", err))?;
        }
        self.island.insert(
            spec.id.clone(),
            IslandVisual {
                visual,
                width,
                height,
                color: spec.color,
                text: spec.text.clone(),
                hwnd: spec.hwnd,
            },
        );
        Ok(())
    }
}

/// One island node to attach above the WebView visual.
#[derive(Debug, Clone)]
pub struct IslandVisualSpec {
    pub id: String,
    pub kind: String,
    pub offset_x: f32,
    pub offset_y: f32,
    pub width: i32,
    pub height: i32,
    /// Premultiplied source color as `0xAARRGGBB`. Alpha 0 is transparent.
    pub color: u32,
    pub text: Option<String>,
    /// Cloaked (non-child) HWND whose redirection surface is the visual
    /// content. Used for video; `CreateSurfaceFromHwnd` rejects `WS_CHILD`.
    pub hwnd: Option<isize>,
}

/// BGRA pixels captured from the composition surface HWND (DComp target).
#[derive(Debug, Clone)]
pub struct CompositionSurfacePixels {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

fn rasterize_island_pixels(width: i32, height: i32, color: u32, text: Option<&str>) -> Vec<u32> {
    let width = width.max(1);
    let height = height.max(1);
    let alpha = (color >> 24) & 0xff;
    let red = (color >> 16) & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = color & 0xff;
    let premultiply = |channel: u32| (channel * alpha + 127) / 255;
    let fill =
        (alpha << 24) | (premultiply(red) << 16) | (premultiply(green) << 8) | premultiply(blue);
    let mut pixels = vec![fill; (width * height) as usize];
    if let Some(text) = text.filter(|text| !text.is_empty()) {
        blit_text_5x7(&mut pixels, width, height, text);
    }
    // 8×8 magenta probe in the corner so PrintWindow of the DComp target
    // can distinguish island visuals from CSS (which never paints #FF00FF).
    let marker = 0xffff_00ffu32;
    for y in 0..8.min(height) {
        for x in 0..8.min(width) {
            pixels[(y * width + x) as usize] = marker;
        }
    }
    pixels
}

fn blit_text_5x7(pixels: &mut [u32], width: i32, height: i32, text: &str) {
    const GLYPH_W: i32 = 5;
    const GAP: i32 = 1;
    let mut cursor_x = 2;
    let cursor_y = 2;
    let white = 0xffff_ffffu32;
    for ch in text.chars() {
        if cursor_x + GLYPH_W > width {
            break;
        }
        if let Some(rows) = glyph_5x7(ch) {
            for (row, bits) in rows.iter().enumerate() {
                let y = cursor_y + row as i32;
                if y < 0 || y >= height {
                    continue;
                }
                for col in 0..GLYPH_W {
                    if bits & (1 << (4 - col)) == 0 {
                        continue;
                    }
                    let x = cursor_x + col;
                    if x < 0 || x >= width {
                        continue;
                    }
                    pixels[(y * width + x) as usize] = white;
                }
            }
        }
        cursor_x += GLYPH_W + GAP;
    }
}

fn glyph_5x7(ch: char) -> Option<[u8; 7]> {
    Some(match ch {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'a' => [0x00, 0x00, 0x0e, 0x01, 0x0f, 0x11, 0x0f],
        'e' => [0x00, 0x00, 0x0e, 0x11, 0x1f, 0x10, 0x0e],
        'i' => [0x04, 0x00, 0x0c, 0x04, 0x04, 0x04, 0x0e],
        'l' => [0x0c, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'n' => [0x00, 0x00, 0x1e, 0x11, 0x11, 0x11, 0x11],
        't' => [0x00, 0x04, 0x1f, 0x04, 0x04, 0x04, 0x03],
        'v' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x0a, 0x04],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::rasterize_island_pixels;

    #[test]
    fn rasterizes_inline_native_label_as_opaque_glyphs() {
        let pixels = rasterize_island_pixels(96, 16, 0, Some("Inline native"));
        let painted = pixels.iter().filter(|pixel| **pixel == 0xffff_ffff).count();
        assert!(
            painted > 20,
            "expected white glyph pixels for 'Inline native', got {painted}"
        );
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
            // Ring alphas must match the shell's CARD_SHADOW_RING_ALPHA.
            let ring_alphas = [5.0f32, 4.0, 3.0, 3.0, 2.0, 2.0, 1.0, 1.0];
            let mut keep = 1.0f32;
            for spread in 1usize..=8 {
                if shadow_distance <= (radius as usize + spread) as f32 {
                    keep *= 1.0 - ring_alphas[spread - 1] / 255.0;
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
/// anti-aliased arc band hugging the inside of the clip radius,
/// transparent on both sides. The band sits fully inside the radius so a
/// host window region cutting at the same radius cannot clip it away; the
/// dark hairline masks the cut's aliased content edge on any backdrop.
fn ring_pixels(corner: usize, radius: i32, color: u32) -> Vec<u32> {
    let (center_x, center_y) = match corner {
        0 => (radius, radius),
        1 => (0, radius),
        2 => (0, 0),
        _ => (radius, 0),
    };
    let alpha = (color >> 24) & 0xff;
    let (red, green, blue) = ((color >> 16) & 0xff, (color >> 8) & 0xff, color & 0xff);
    let (inner, outer) = (radius as f32 - 2.0, radius as f32);
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

fn create_d3d_device() -> StdResult<(ID3D11Device, ID3D11DeviceContext)> {
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        let mut context = None;
        let created = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                Default::default(),
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
