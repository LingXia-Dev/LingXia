//! The terminal's renderer: a composited GPU surface per panel.
//!
//! The grid cannot be drawn on the GPU into the shell's `WM_PAINT` HDC, so the
//! panel body gets its own child window carrying a DirectComposition target
//! with a flip-model swapchain. That is the hosting WebView2 surfaces in this
//! repo already use, which is what makes it agree with the rounded card the
//! chrome painter draws around it.
//!
//! Everything the grid draws — cell backgrounds, glyphs, rules, cursor,
//! selection, scrollbar — is one rectangle with a color and an atlas region,
//! so a whole frame is a single instanced draw.

mod pipeline;
mod sprites;
mod text;

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use lingxia_terminal::{
    ATTR_BOLD, ATTR_DIM, ATTR_HIDDEN, ATTR_INVERSE, ATTR_ITALIC, ATTR_STRIKE, FrameCell,
    TerminalFrame, TerminalScrollbar,
};

use super::terminal_grid::PaneView;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG, D3D11_RENDER_TARGET_VIEW_DESC,
    D3D11_RENDER_TARGET_VIEW_DESC_0, D3D11_RTV_DIMENSION_TEXTURE2D, D3D11_SDK_VERSION,
    D3D11_TEX2D_RTV, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice3, IDCompositionDesktopDevice, IDCompositionRectangleClip,
    IDCompositionTarget, IDCompositionVisual2,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
    DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HTTRANSPARENT, HWND_TOP, RegisterClassW,
    SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SetParent, SetWindowPos, ShowWindow,
    WM_ERASEBKGND, WM_NCHITTEST, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_NOREDIRECTIONBITMAP,
    WS_VISIBLE,
};
use windows::core::{Interface, PCWSTR, Result, w};

use super::terminal_grid::{
    GRID_DEFAULT_BACKGROUND, GRID_DIM_FOREGROUND_PERCENT, GRID_PADDING, GridPoint,
    PANE_DROP_TARGET_COLOR, SCROLLBAR_MARGIN, SCROLLBAR_MAX_THUMB, SCROLLBAR_MIN_THUMB,
    SCROLLBAR_WIDTH, SELECTION_ACCENT, SELECTION_ACCENT_PERCENT,
};
use pipeline::{Pipeline, Quad};
use text::{BOLD, BOLD_ITALIC, Fonts, ITALIC, Metrics, REGULAR};

/// Atlas key namespace for drawn sprites, which have no font style. Chosen
/// past the four face indices so it can never collide with a glyph.
const SPRITE_STYLE: usize = 4;

/// The codepoint of a run that is exactly one drawn character.
///
/// Runs break on style, not on content, so a border is usually a run of the
/// same sprite; each cell still draws its own, because a sprite is defined by
/// the cell it fills.
fn sole_sprite(run: &str) -> Option<u32> {
    let mut chars = run.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let scalar = u32::from(first);
    sprites::handles(scalar).then_some(scalar)
}

/// Render a panel's terminal body.
pub(super) fn present(parent: HWND, panel_id: &str, body: RECT, radii: [i32; 4]) {
    register_captures();
    let mut surfaces = surfaces();
    let surface = match surfaces.entry(panel_id.to_string()) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => match Surface::new(parent) {
            Ok(surface) => entry.insert(surface),
            Err(error) => {
                log::error!("terminal GPU surface unavailable: {error}");
                return;
            }
        },
    };
    if let Err(error) = surface.present(parent, panel_id, body, radii) {
        log::error!("terminal GPU present failed: {error}");
        surfaces.remove(panel_id);
    }
}

/// Offer the composited grid to screenshots.
///
/// `BitBlt` cannot see a DirectComposition surface, so a capture that does not
/// come through here shows the card with an empty body — which is what every
/// Windows screenshot of the terminal did before this existed.
fn captures_for_window(window_id: usize) -> Vec<lingxia_windows_contract::WindowsSurfaceCapture> {
    let mut surfaces = surfaces();
    surfaces
        .values_mut()
        .filter(|surface| surface.parent.0 as usize == window_id)
        .filter_map(Surface::capture)
        .collect()
}

/// Register the screenshot provider once.
fn register_captures() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        lingxia_windows_contract::register_surface_capture_provider(captures_for_window);
    });
}

/// Tear down a panel's surface when the panel closes.
pub(super) fn drop_panel(panel_id: &str) {
    surfaces().remove(panel_id);
}

/// Cell size of the font in effect, so the facade can size PTYs and hit-test
/// without waiting for a frame.
pub(super) fn cell_size() -> Option<(i32, i32)> {
    let mut slot = shared_fonts()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fonts = ensure_fonts(&mut slot).ok()?;
    let metrics = fonts.0.metrics;
    Some((metrics.cell_width as i32, metrics.line_height as i32))
}

fn surfaces() -> MutexGuard<'static, HashMap<String, Surface>> {
    static SURFACES: OnceLock<Mutex<HashMap<String, Surface>>> = OnceLock::new();
    SURFACES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The fonts are process-wide: every panel draws the same configured family,
/// and the shaping cache is worth far more shared than split. The `u64` is the
/// configuration generation the faces were built for.
struct SharedFonts(Fonts, u64);

fn shared_fonts() -> &'static Mutex<Option<SharedFonts>> {
    static FONTS: OnceLock<Mutex<Option<SharedFonts>>> = OnceLock::new();
    FONTS.get_or_init(|| Mutex::new(None))
}

/// One panel's composited surface: child window, device, swapchain, and the
/// DirectComposition tree binding them together.
struct Surface {
    hwnd: HWND,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swapchain: IDXGISwapChain1,
    composition: IDCompositionDesktopDevice,
    /// Owns the HWND binding; dropping it detaches the tree.
    _target: IDCompositionTarget,
    /// Held to keep the tree alive; the target owns it as its root.
    _visual: IDCompositionVisual2,
    clip: IDCompositionRectangleClip,
    pipeline: Pipeline,
    view: Option<ID3D11RenderTargetView>,
    /// Font generation the atlas was filled for.
    fonts_generation: u64,
    parent: HWND,
    bounds: RECT,
    radii: [i32; 4],
    quads: Vec<Quad>,
    /// Clear color of the last frame, so a capture reproduces it exactly.
    background: u32,
    /// Offscreen copy of the frame, for screenshots. Built on demand.
    readback: Option<Readback>,
}

/// An offscreen target the frame can be drawn into and read back from.
struct Readback {
    target: ID3D11Texture2D,
    view: ID3D11RenderTargetView,
    staging: ID3D11Texture2D,
    width: u32,
    height: u32,
}

// Created and used only on the shell's UI thread; the map that holds it is
// shared, so say so rather than making every accessor thread-local.
unsafe impl Send for Surface {}

impl Surface {
    fn new(parent: HWND) -> Result<Self> {
        unsafe {
            let hwnd = create_surface_window(parent)?;
            let (device, context) = create_device()?;
            let dxgi: IDXGIDevice = device.cast()?;
            let factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
            // 1x1 until the first present sizes it: a zero-sized swapchain is
            // invalid and the real size is only known once the card is laid out.
            let swapchain =
                factory.CreateSwapChainForComposition(&device, &swapchain_desc(1, 1), None)?;
            let composition: IDCompositionDesktopDevice = DCompositionCreateDevice3(&dxgi)?;
            let target = composition.CreateTargetForHwnd(hwnd, true)?;
            let visual = composition.CreateVisual()?;
            let clip = composition.CreateRectangleClip()?;
            visual.SetContent(&swapchain)?;
            visual.SetClip(&clip)?;
            target.SetRoot(&visual)?;
            composition.Commit()?;
            let pipeline = Pipeline::new(&device)?;
            Ok(Self {
                hwnd,
                device,
                context,
                swapchain,
                composition,
                _target: target,
                _visual: visual,
                clip,
                pipeline,
                view: None,
                fonts_generation: u64::MAX,
                parent,
                bounds: RECT::default(),
                radii: [0; 4],
                quads: Vec::new(),
                background: GRID_DEFAULT_BACKGROUND,
                readback: None,
            })
        }
    }

    fn present(&mut self, parent: HWND, panel_id: &str, body: RECT, radii: [i32; 4]) -> Result<()> {
        let width = (body.right - body.left).max(1);
        let height = (body.bottom - body.top).max(1);
        self.place(parent, body, radii, width, height)?;

        let mut slot = shared_fonts()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let fonts = ensure_fonts(&mut slot)?;
        if fonts.1 != self.fonts_generation {
            self.pipeline.reset_glyphs();
            self.fonts_generation = fonts.1;
        }
        let metrics = fonts.0.metrics;

        super::terminal_grid::set_panel_body(panel_id, body);
        self.quads.clear();
        let panes = super::terminal_panel::active_pane_frames(panel_id, body);
        let multi = panes.len() > 1;
        let (drag_source, drop_target) = super::terminal_panel::pane_drag_visuals(panel_id, body);
        let mut background = GRID_DEFAULT_BACKGROUND;
        for pane in &panes {
            // Pane rects are in the host's client space; the surface's origin
            // is the body's, so everything shifts by it once, here.
            let rect = RECT {
                left: pane.rect.left - body.left,
                top: pane.rect.top - body.top,
                right: pane.rect.right - body.left,
                bottom: pane.rect.bottom - body.top,
            };
            // A pane being dragged reads as lifted by fading like an
            // unfocused one, which is the same treatment for the same reason.
            let dim = (multi && !pane.focused) || drag_source == Some(pane.session_id);
            let quads = &mut self.quads;
            let pipeline = &mut self.pipeline;
            let shaper = &mut fonts.0;
            super::terminal_grid::with_pane(
                pane.session_id,
                rect,
                (metrics.cell_width as i32, metrics.line_height as i32),
                |frame, view, selection| {
                    if pane.focused || !multi {
                        background = frame.default_bg >> 8;
                    }
                    Builder {
                        quads,
                        fonts: shaper,
                        pipeline,
                        metrics,
                    }
                    .pane(frame, view, selection, rect, dim);
                },
            );
        }

        if let Some(target) = drop_target {
            let rect = RECT {
                left: target.left - body.left,
                top: target.top - body.top,
                right: target.right - body.left,
                bottom: target.bottom - body.top,
            };
            let mut builder = Builder {
                quads: &mut self.quads,
                fonts: &mut fonts.0,
                pipeline: &mut self.pipeline,
                metrics,
            };
            builder.outline(rect, 2.0, PANE_DROP_TARGET_COLOR);
        }

        self.background = background;
        let view = self.target_view()?;
        unsafe {
            self.context
                .ClearRenderTargetView(&view, &linear(background));
        }
        self.pipeline.draw(
            &self.device,
            &self.context,
            &view,
            width as f32,
            height as f32,
            &self.quads,
        )?;
        if debug_layer() {
            drain_debug_messages(&self.device);
        }
        unsafe { self.swapchain.Present(0, Default::default()).ok()? };
        Ok(())
    }

    fn place(
        &mut self,
        parent: HWND,
        body: RECT,
        radii: [i32; 4],
        width: i32,
        height: i32,
    ) -> Result<()> {
        unsafe {
            if parent != self.parent {
                // The card moved to another host window; rebind rather than
                // leave the surface under a window that may be gone.
                SetParent(self.hwnd, Some(parent))?;
                self.parent = parent;
                self.bounds = RECT::default();
            }
            if self.bounds != body {
                let resized = (self.bounds.right - self.bounds.left) != width
                    || (self.bounds.bottom - self.bounds.top) != height;
                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    body.left,
                    body.top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )?;
                self.bounds = body;
                if resized {
                    // Every view on the old backbuffers must go before
                    // ResizeBuffers, or DXGI refuses the call.
                    self.view = None;
                    self.context.OMSetRenderTargets(None, None);
                    self.context.Flush();
                    self.swapchain.ResizeBuffers(
                        0,
                        width as u32,
                        height as u32,
                        DXGI_FORMAT_B8G8R8A8_UNORM,
                        Default::default(),
                    )?;
                    self.radii = [i32::MIN; 4];
                }
                let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
            if radii != self.radii {
                self.clip.SetLeft2(0.0)?;
                self.clip.SetTop2(0.0)?;
                self.clip.SetRight2(width as f32)?;
                self.clip.SetBottom2(height as f32)?;
                self.clip.SetTopLeftRadiusX2(radii[0] as f32)?;
                self.clip.SetTopLeftRadiusY2(radii[0] as f32)?;
                self.clip.SetTopRightRadiusX2(radii[1] as f32)?;
                self.clip.SetTopRightRadiusY2(radii[1] as f32)?;
                self.clip.SetBottomRightRadiusX2(radii[2] as f32)?;
                self.clip.SetBottomRightRadiusY2(radii[2] as f32)?;
                self.clip.SetBottomLeftRadiusX2(radii[3] as f32)?;
                self.clip.SetBottomLeftRadiusY2(radii[3] as f32)?;
                self.radii = radii;
                self.composition.Commit()?;
            }
        }
        Ok(())
    }

    /// An `_SRGB` view over the `_UNORM` backbuffer, so the GPU encodes on
    /// write and blending happens in the space it is correct in.
    fn target_view(&mut self) -> Result<ID3D11RenderTargetView> {
        if let Some(view) = &self.view {
            return Ok(view.clone());
        }
        let desc = D3D11_RENDER_TARGET_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
            ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 },
            },
        };
        unsafe {
            let backbuffer: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let mut view = None;
            self.device
                .CreateRenderTargetView(&backbuffer, Some(&desc), Some(&mut view))?;
            let view = view.ok_or_else(windows::core::Error::from_thread)?;
            self.view = Some(view.clone());
            Ok(view)
        }
    }
}

impl Surface {
    /// Re-draw the last frame into an offscreen target and read it back.
    ///
    /// Re-drawing rather than copying the swapchain: the flip model leaves the
    /// back buffer's contents undefined after `Present`, and the quads that
    /// built the frame are still here.
    fn capture(&mut self) -> Option<lingxia_windows_contract::WindowsSurfaceCapture> {
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_TEXTURE2D_DESC,
            D3D11_USAGE_STAGING,
        };
        use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_TYPELESS;

        let width = (self.bounds.right - self.bounds.left).max(0) as u32;
        let height = (self.bounds.bottom - self.bounds.top).max(0) as u32;
        if width == 0 || height == 0 {
            return None;
        }
        if self
            .readback
            .as_ref()
            .is_none_or(|readback| readback.width != width || readback.height != height)
        {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                // Typeless, because the `_SRGB` view below is only legal over
                // a typeless texture. A swapchain back buffer is the exception
                // DXGI makes for the flip model, not the rule.
                Format: DXGI_FORMAT_B8G8R8A8_TYPELESS,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: windows::Win32::Graphics::Direct3D11::D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
                ..Default::default()
            };
            let staging = D3D11_TEXTURE2D_DESC {
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..desc
            };
            unsafe {
                let mut target = None;
                report(
                    "capture target",
                    self.device.CreateTexture2D(&desc, None, Some(&mut target)),
                )?;
                let target = target?;
                let mut view = None;
                report(
                    "capture view",
                    self.device.CreateRenderTargetView(
                        &target,
                        Some(&D3D11_RENDER_TARGET_VIEW_DESC {
                            Format: DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
                            ViewDimension: D3D11_RTV_DIMENSION_TEXTURE2D,
                            Anonymous: D3D11_RENDER_TARGET_VIEW_DESC_0 {
                                Texture2D: D3D11_TEX2D_RTV { MipSlice: 0 },
                            },
                        }),
                        Some(&mut view),
                    ),
                )?;
                let mut readback_staging = None;
                report(
                    "capture staging",
                    self.device
                        .CreateTexture2D(&staging, None, Some(&mut readback_staging)),
                )?;
                self.readback = Some(Readback {
                    target,
                    view: view?,
                    staging: readback_staging?,
                    width,
                    height,
                });
            }
        }
        let readback = self.readback.as_ref()?;
        unsafe {
            self.context
                .ClearRenderTargetView(&readback.view, &linear(self.background));
        }
        report(
            "capture draw",
            self.pipeline.draw(
                &self.device,
                &self.context,
                &readback.view,
                width as f32,
                height as f32,
                &self.quads,
            ),
        )?;

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        unsafe {
            self.context
                .CopyResource(&readback.staging, &readback.target);
            let mut mapped = Default::default();
            report(
                "capture map",
                self.context
                    .Map(&readback.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)),
            )?;
            for row in 0..height {
                let source = (mapped.pData as *const u8).add((row * mapped.RowPitch) as usize);
                let target = (row * width * 4) as usize;
                std::ptr::copy_nonoverlapping(
                    source,
                    pixels[target..].as_mut_ptr(),
                    (width * 4) as usize,
                );
            }
            self.context.Unmap(&readback.staging, 0);
        }
        Some(lingxia_windows_contract::WindowsSurfaceCapture {
            x: self.bounds.left,
            y: self.bounds.top,
            width,
            height,
            pixels,
        })
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Build the fonts from the configuration in effect, rebuilding when it moved.
fn ensure_fonts(slot: &mut Option<SharedFonts>) -> Result<&mut SharedFonts> {
    let config = lingxia_terminal_config::runtime::current_config();
    let generation = lingxia_terminal_config::runtime::generation();
    let size = if (4.0..=96.0).contains(&config.font.size) {
        config.font.size
    } else {
        13.0
    };
    match slot {
        Some(fonts) if fonts.1 == generation => {}
        Some(fonts) => {
            if fonts
                .0
                .reload(&config.font.family, size, config.font.ligatures)?
            {
                log::info!("terminal font: {} {size}pt", fonts.0.family());
            }
            fonts.1 = generation;
        }
        None => {
            let fonts = Fonts::new(&config.font.family, size, config.font.ligatures)?;
            log::info!("terminal font: {} {size}pt", fonts.family());
            *slot = Some(SharedFonts(fonts, generation));
        }
    }
    Ok(slot.as_mut().expect("populated above"))
}

/// The style a run of cells shares. Runs break wherever any of it changes,
/// which is also where shaping has to restart.
#[derive(Clone, Copy, PartialEq)]
struct RunStyle {
    face: usize,
    color: u32,
    strike: bool,
    underline: &'static str,
    underline_color: u32,
}

/// Turns one pane's frame into quads.
struct Builder<'a> {
    quads: &'a mut Vec<Quad>,
    fonts: &'a mut Fonts,
    pipeline: &'a mut Pipeline,
    metrics: Metrics,
}

impl Builder<'_> {
    fn pane(
        &mut self,
        frame: &TerminalFrame,
        view: PaneView,
        selection: Option<(GridPoint, GridPoint)>,
        rect: RECT,
        dim: bool,
    ) {
        let origin = (
            (rect.left + GRID_PADDING) as f32,
            (rect.top + GRID_PADDING) as f32,
        );
        // Frame colors are packed 0xRRGGBBAA; every quad wants 0xRRGGBB.
        let background = frame.default_bg >> 8;
        let foreground = frame.default_fg >> 8;

        // The pane's own background, so panes with different schemes and the
        // divider gaps between them stay right.
        self.solid(
            rect.left as f32,
            rect.top as f32,
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
            background,
        );

        // Backgrounds first: a later cell's background must not cover the
        // right half of a wide glyph the previous cell drew.
        for (index, cell) in frame.cells.iter().enumerate() {
            // Alpha 0 means "inherit the default", so such a cell needs no fill
            // of its own unless inverse video swaps it in.
            if cell.attrs & ATTR_INVERSE == 0 && cell.bg & 0xff == 0 {
                continue;
            }
            let (_, mut fill) = resolve(cell, background, foreground);
            if dim {
                fill = blend(fill, background, GRID_DIM_FOREGROUND_PERCENT);
            }
            let (row, col) = frame.position(index);
            self.solid(
                origin.0 + f32::from(col) * self.metrics.cell_width,
                origin.1 + f32::from(row) * self.metrics.line_height,
                self.metrics.cell_width * f32::from(cell.columns.max(1)),
                self.metrics.line_height,
                fill,
            );
        }

        if let Some((start, end)) = selection {
            let highlight = blend(SELECTION_ACCENT, background, SELECTION_ACCENT_PERCENT);
            for row in start.row..=end.row {
                let first = if row == start.row { start.col } else { 0 };
                let last = if row == end.row { end.col } else { frame.cols };
                if last <= first {
                    continue;
                }
                self.solid(
                    origin.0 + f32::from(first) * self.metrics.cell_width,
                    origin.1 + f32::from(row) * self.metrics.line_height,
                    f32::from(last - first) * self.metrics.cell_width,
                    self.metrics.line_height,
                    highlight,
                );
            }
        }

        self.text(frame, origin, background, foreground, dim);

        // Only the focused pane paints a cursor: hollow cursors in every split
        // make cursor-heavy TUIs look like they flicker in several places.
        if !dim && !view.exited && frame.cursor.visible {
            let style = frame.cursor.style.as_str();
            let (width, height) = match style {
                "bar" => (2.0, self.metrics.line_height),
                "underline" => (self.metrics.cell_width, 2.0),
                _ => (self.metrics.cell_width, self.metrics.line_height),
            };
            let top = origin.1
                + f32::from(frame.cursor.row) * self.metrics.line_height
                + if style == "underline" {
                    self.metrics.line_height - height
                } else {
                    0.0
                };
            self.solid(
                origin.0 + f32::from(frame.cursor.col) * self.metrics.cell_width,
                top,
                width,
                height,
                foreground,
            );
        }

        if let Some(scrollbar) = view.scrollbar {
            self.scrollbar(scrollbar, rect, background, foreground);
        }
    }

    /// Text, run by run: adjacent cells sharing a style shape together, which
    /// is what lets the font's `calt` turn `!=` into one glyph.
    fn text(
        &mut self,
        frame: &TerminalFrame,
        origin: (f32, f32),
        background: u32,
        foreground: u32,
        dim: bool,
    ) {
        let mut run = String::new();
        let mut columns = 0u16;
        let mut at = (0u16, 0u16);
        let mut style = RunStyle {
            face: REGULAR,
            color: foreground,
            strike: false,
            underline: "none",
            underline_color: foreground,
        };

        for (index, cell) in frame.cells.iter().enumerate() {
            // A continuation column belongs to the wide glyph before it.
            if cell.columns == 0 {
                continue;
            }
            let (row, col) = frame.position(index);
            let cell_style = self.style_of(cell, background, foreground, dim);
            let contiguous =
                !run.is_empty() && row == at.0 && col == at.1 + columns && cell_style == style;
            if !contiguous {
                self.flush(&run, at, columns, style, origin);
                run.clear();
                columns = 0;
                at = (row, col);
                style = cell_style;
            }
            let cluster = frame.cell_text(cell);
            if cell.attrs & ATTR_HIDDEN != 0 || cluster.is_empty() {
                // Concealed text still occupies its columns, so keep the run
                // aligned rather than closing it.
                run.push(' ');
            } else {
                run.push_str(cluster);
            }
            columns += u16::from(cell.columns);
        }
        self.flush(&run, at, columns, style, origin);
    }

    fn style_of(&self, cell: &FrameCell, background: u32, foreground: u32, dim: bool) -> RunStyle {
        let (mut color, cell_background) = resolve(cell, background, foreground);
        if cell.attrs & ATTR_DIM != 0 {
            color = blend(color, cell_background, GRID_DIM_FOREGROUND_PERCENT);
        }
        if dim {
            color = blend(color, background, GRID_DIM_FOREGROUND_PERCENT);
        }
        RunStyle {
            face: match (cell.attrs & ATTR_BOLD != 0, cell.attrs & ATTR_ITALIC != 0) {
                (true, true) => BOLD_ITALIC,
                (true, false) => BOLD,
                (false, true) => ITALIC,
                (false, false) => REGULAR,
            },
            color,
            strike: cell.attrs & ATTR_STRIKE != 0,
            underline: underline_name(cell.underline),
            // Alpha 0 is SGR 58's "no explicit color" — follow the text.
            underline_color: if cell.underline_color & 0xff == 0 {
                color
            } else {
                cell.underline_color >> 8
            },
        }
    }

    fn flush(
        &mut self,
        run: &str,
        at: (u16, u16),
        columns: u16,
        style: RunStyle,
        origin: (f32, f32),
    ) {
        if run.is_empty() {
            return;
        }
        let x = origin.0 + f32::from(at.1) * self.metrics.cell_width;
        let y = origin.1 + f32::from(at.0) * self.metrics.line_height;
        let width = f32::from(columns) * self.metrics.cell_width;

        if let Some(scalar) = sole_sprite(run) {
            self.sprite(scalar, x, y, style.color);
        } else if !run.trim().is_empty() {
            let glyphs: Vec<_> = self.fonts.shape(run, style.face).to_vec();
            for glyph in glyphs {
                let sprite = match self.pipeline.sprite(glyph.index, style.face) {
                    Some(sprite) => sprite,
                    None => {
                        let raster = self.fonts.rasterize(glyph.index, style.face).ok().flatten();
                        self.pipeline
                            .insert_sprite(glyph.index, style.face, raster.as_ref())
                    }
                };
                let Some(sprite) = sprite else { continue };
                self.quads.push(Quad {
                    // Sprites are placed at whole pixels: a fractional origin
                    // resamples a bitmap that is already the right size.
                    rect: [
                        (x + f32::from(glyph.cell) * self.metrics.cell_width + sprite.left).round(),
                        (y + self.metrics.baseline + sprite.top).round(),
                        sprite.width,
                        sprite.height,
                    ],
                    color: linear(style.color),
                    uv: sprite.uv,
                    params: [f32::from(u8::from(sprite.colored)), 0.0, 0.0, 0.0],
                });
            }
        }

        if style.underline != "none" {
            let thickness = self.metrics.underline_thickness;
            let top = y + self.metrics.baseline + self.metrics.underline_offset;
            self.solid(x, top, width, thickness, style.underline_color);
            if style.underline == "double" {
                self.solid(
                    x,
                    top + thickness * 2.0,
                    width,
                    thickness,
                    style.underline_color,
                );
            }
        }
        if style.strike {
            self.solid(
                x,
                y + self.metrics.baseline + self.metrics.strike_offset,
                width,
                self.metrics.underline_thickness,
                style.color,
            );
        }
    }

    /// Box art drawn to the cell rather than taken from the font, so borders
    /// meet exactly at any size and in any face.
    fn sprite(&mut self, scalar: u32, x: f32, y: f32, color: u32) {
        let sprite = match self.pipeline.sprite(scalar as u16, SPRITE_STYLE) {
            Some(sprite) => sprite,
            None => {
                let drawn = sprites::draw(
                    scalar,
                    self.metrics.cell_width,
                    self.metrics.line_height,
                    self.metrics.baseline,
                );
                self.pipeline
                    .insert_sprite(scalar as u16, SPRITE_STYLE, drawn.as_ref())
            }
        };
        let Some(sprite) = sprite else { return };
        self.quads.push(Quad {
            rect: [
                (x + sprite.left).round(),
                (y + self.metrics.baseline + sprite.top).round(),
                sprite.width,
                sprite.height,
            ],
            color: linear(color),
            uv: sprite.uv,
            params: [0.0; 4],
        });
    }

    fn scrollbar(
        &mut self,
        scrollbar: TerminalScrollbar,
        rect: RECT,
        background: u32,
        foreground: u32,
    ) {
        let track_top = (rect.top + SCROLLBAR_MARGIN) as f32;
        let track = ((rect.bottom - SCROLLBAR_MARGIN) - (rect.top + SCROLLBAR_MARGIN)) as f32;
        if track <= 0.0 || scrollbar.total == 0 || scrollbar.len >= scrollbar.total {
            return;
        }
        let visible = scrollbar.len.min(scrollbar.total) as f32;
        let thumb = (track * visible / scrollbar.total as f32).clamp(
            (SCROLLBAR_MIN_THUMB as f32).min(track),
            (SCROLLBAR_MAX_THUMB as f32).min(track),
        );
        let max_offset = (scrollbar.total - scrollbar.len) as f32;
        let progress = (scrollbar.offset as f32 / max_offset).clamp(0.0, 1.0);
        self.solid(
            (rect.right - SCROLLBAR_MARGIN - SCROLLBAR_WIDTH) as f32,
            track_top + (track - thumb) * progress,
            SCROLLBAR_WIDTH as f32,
            thumb,
            blend(foreground, background, 38),
        );
    }

    /// A rectangle drawn as its four edges, for the pane drop target.
    fn outline(&mut self, rect: RECT, thickness: f32, color: u32) {
        let (x, y) = (rect.left as f32, rect.top as f32);
        let (width, height) = (
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        );
        self.solid(x, y, width, thickness, color);
        self.solid(x, y + height - thickness, width, thickness, color);
        self.solid(x, y, thickness, height, color);
        self.solid(x + width - thickness, y, thickness, height, color);
    }

    fn solid(&mut self, x: f32, y: f32, width: f32, height: f32, color: u32) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        self.quads.push(Quad {
            rect: [
                x.round(),
                y.round(),
                width.round().max(1.0),
                height.round().max(1.0),
            ],
            color: linear(color),
            uv: self.pipeline.solid_uv,
            params: [0.0; 4],
        });
    }
}

/// A cell's colors, with alpha 0 meaning "inherit the frame's default".
fn resolve(cell: &FrameCell, background: u32, foreground: u32) -> (u32, u32) {
    let fg = if cell.fg & 0xff == 0 {
        foreground
    } else {
        cell.fg >> 8
    };
    let bg = if cell.bg & 0xff == 0 {
        background
    } else {
        cell.bg >> 8
    };
    if cell.attrs & ATTR_INVERSE != 0 {
        (bg, fg)
    } else {
        (fg, bg)
    }
}

/// `FrameCell::underline` is [`UnderlineStyle`] as an index; the run style
/// names it, because that is what the glyph pass branches on.
fn underline_name(index: u8) -> &'static str {
    match index {
        1 => "single",
        2 => "double",
        3 => "curly",
        4 => "dotted",
        5 => "dashed",
        _ => "none",
    }
}

fn parse_hex(token: &str) -> Option<u32> {
    let hex = token.strip_prefix('#').unwrap_or(token);
    (hex.len() == 6)
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}

fn blend(color: u32, towards: u32, percent: u32) -> u32 {
    let channel = |shift: u32| {
        let from = (color >> shift) & 0xff;
        let to = (towards >> shift) & 0xff;
        ((from * percent + to * (100 - percent)) / 100) << shift
    };
    channel(16) | channel(8) | channel(0)
}

/// sRGB to linear. The render target encodes on write, so the shader has to be
/// handed linear values or every fill comes out too light.
fn linear(color: u32) -> [f32; 4] {
    let channel = |shift: u32| {
        let value = ((color >> shift) & 0xff) as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    [channel(16), channel(8), channel(0), 1.0]
}

fn swapchain_desc(width: u32, height: u32) -> DXGI_SWAP_CHAIN_DESC1 {
    DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
        // The grid is opaque; the card behind it never shows through.
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        ..Default::default()
    }
}

fn create_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    // The debug layer names exactly why a draw produced nothing, which no
    // amount of reading the state back can. It is absent unless the Graphics
    // Tools feature is installed, so it is requested and then dropped.
    let debug = debug_layer();
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        let mut context = None;
        let result = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                None,
                // Required by DirectComposition.
                if debug {
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_DEBUG
                } else {
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT
                },
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        if result.is_ok()
            && let (Some(device), Some(context)) = (device, context)
        {
            return Ok((device, context));
        }
    }
    Err(windows::core::Error::from_thread())
}

const SURFACE_CLASS: PCWSTR = w!("LingXiaTerminalSurface");

/// A child window owning nothing but the composition target.
///
/// `WS_EX_NOREDIRECTIONBITMAP` keeps GDI from allocating a redirection surface
/// for it: the window never paints, the compositor does.
fn create_surface_window(parent: HWND) -> Result<HWND> {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    REGISTERED.get_or_init(|| unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(surface_proc),
            lpszClassName: SURFACE_CLASS,
            ..Default::default()
        };
        RegisterClassW(&class) != 0
    });
    unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            SURFACE_CLASS,
            PCWSTR::null(),
            WS_CHILD | WS_CLIPSIBLINGS | WS_VISIBLE,
            0,
            0,
            1,
            1,
            Some(parent),
            None,
            None,
            None,
        )
    }
}

/// Input belongs to the shell's hit-testing, which works in the parent's
/// coordinates — so this window declines to be hit at all.
unsafe extern "system" fn surface_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        // Nothing to erase: the compositor owns every pixel.
        WM_ERASEBKGND => LRESULT(1),
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// Report a failed capture step instead of losing it to `?`. A screenshot that
/// silently drops the terminal looks exactly like a renderer that draws
/// nothing, which cost an afternoon once.
fn report<T>(what: &str, result: Result<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            log::warn!("terminal {what} failed: {error}");
            None
        }
    }
}

/// Whether the D3D11 debug layer was asked for. It names exactly why a draw
/// produced nothing, which reading the state back cannot, but it needs the
/// Graphics Tools feature installed — hence opt-in.
fn debug_layer() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("LINGXIA_TERMINAL_GPU_DEBUG").is_ok())
}

/// Log whatever the debug layer has to say.
fn drain_debug_messages(device: &ID3D11Device) {
    use windows::Win32::Graphics::Direct3D11::ID3D11InfoQueue;

    let Ok(queue) = device.cast::<ID3D11InfoQueue>() else {
        return;
    };
    unsafe {
        for index in 0..queue.GetNumStoredMessages() {
            let mut size = 0;
            if queue.GetMessage(index, None, &mut size).is_err() || size == 0 {
                continue;
            }
            let mut buffer = vec![0u8; size];
            let message = buffer.as_mut_ptr().cast();
            if queue.GetMessage(index, Some(message), &mut size).is_err() {
                continue;
            }
            let message = &*message;
            let text = std::slice::from_raw_parts(
                message.pDescription.cast::<u8>(),
                message.DescriptionByteLength.saturating_sub(1),
            );
            log::warn!("d3d11: {}", String::from_utf8_lossy(text));
        }
        queue.ClearStoredMessages();
    }
}
