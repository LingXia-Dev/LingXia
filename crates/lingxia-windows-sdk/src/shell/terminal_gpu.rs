//! GPU-composited surface for a terminal panel.
//!
//! The grid cannot be drawn on the GPU into the shell's `WM_PAINT` HDC, so it
//! gets its own child window carrying a DirectComposition target with a
//! flip-model swapchain. That is the same hosting WebView2 surfaces already
//! use here, which is what makes it composite correctly with the rounded card
//! the chrome painter draws around it.
//!
//! Opt-in while the GPU path cannot draw text yet: a composited surface covers
//! the GDI grid underneath it, so switching unconditionally would blank the
//! terminal. Set `LINGXIA_TERMINAL_GPU=1` to use it.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice3, IDCompositionDesktopDevice, IDCompositionRectangleClip,
    IDCompositionTarget, IDCompositionVisual2,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_TOP, RegisterClassW, SW_SHOWNOACTIVATE,
    SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, ShowWindow, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_EX_NOREDIRECTIONBITMAP, WS_VISIBLE,
};
use windows::core::{Interface, PCWSTR, w};

/// Whether the GPU path is switched on for this process.
pub(super) fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        matches!(
            std::env::var("LINGXIA_TERMINAL_GPU").as_deref(),
            Ok("1" | "true" | "on")
        )
    })
}

/// Draw a panel's terminal body on the GPU.
///
/// `body` is in `parent`'s client coordinates and `radii` are the card's
/// corner radii, `[tl, tr, br, bl]`. Returns `false` when the surface could
/// not be brought up, so the caller falls back to drawing with GDI.
pub(super) fn present(
    parent: HWND,
    panel_id: &str,
    body: RECT,
    radii: [i32; 4],
    background: u32,
) -> bool {
    if !enabled() {
        return false;
    }
    let mut surfaces = surfaces();
    let surface = match surfaces.entry(panel_id.to_string()) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => match Surface::new(parent) {
            Ok(surface) => entry.insert(surface),
            Err(error) => {
                log::warn!("terminal GPU surface unavailable: {error}");
                return false;
            }
        },
    };
    match surface.present(parent, body, radii, background) {
        Ok(()) => true,
        Err(error) => {
            log::warn!("terminal GPU present failed: {error}");
            surfaces.remove(panel_id);
            false
        }
    }
}

/// Tear down a panel's surface when the panel closes.
pub(super) fn drop_panel(panel_id: &str) {
    surfaces().remove(panel_id);
}

fn surfaces() -> MutexGuard<'static, HashMap<String, Surface>> {
    static SURFACES: OnceLock<Mutex<HashMap<String, Surface>>> = OnceLock::new();
    SURFACES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One panel's composited surface: child window, D3D device, swapchain, and
/// the DirectComposition tree binding them together.
struct Surface {
    hwnd: HWND,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swapchain: IDXGISwapChain1,
    composition: IDCompositionDesktopDevice,
    /// Owns the HWND binding; dropping it detaches the tree.
    _target: IDCompositionTarget,
    /// Held only to keep the tree alive; the target owns it as its root.
    _visual: IDCompositionVisual2,
    clip: IDCompositionRectangleClip,
    /// Backbuffer view, dropped and rebuilt across a resize.
    view: Option<ID3D11RenderTargetView>,
    parent: HWND,
    bounds: RECT,
    radii: [i32; 4],
}

// The surface is created and used only on the shell's UI thread; the map that
// holds it is shared, so say so rather than making every accessor thread-local.
unsafe impl Send for Surface {}

impl Surface {
    fn new(parent: HWND) -> windows::core::Result<Self> {
        unsafe {
            let hwnd = create_surface_window(parent)?;
            let (device, context) = create_device()?;
            let dxgi: IDXGIDevice = device.cast()?;
            let factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
            // 1x1 until the first present sizes it; a zero-sized swapchain is
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
            Ok(Self {
                hwnd,
                device,
                context,
                swapchain,
                composition,
                _target: target,
                _visual: visual,
                clip,
                view: None,
                parent,
                bounds: RECT::default(),
                radii: [0; 4],
            })
        }
    }

    fn present(
        &mut self,
        parent: HWND,
        body: RECT,
        radii: [i32; 4],
        background: u32,
    ) -> windows::core::Result<()> {
        let width = (body.right - body.left).max(1);
        let height = (body.bottom - body.top).max(1);
        unsafe {
            if parent != self.parent {
                // The card moved to another host window; rebind rather than
                // leave the surface parented to a window that may be gone.
                windows::Win32::UI::WindowsAndMessaging::SetParent(self.hwnd, Some(parent))?;
                self.parent = parent;
                self.bounds = RECT::default();
            }
            if self.bounds != body {
                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    body.left,
                    body.top,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )?;
                let resized = (self.bounds.right - self.bounds.left) != width
                    || (self.bounds.bottom - self.bounds.top) != height;
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

            if self.view.is_none() {
                let backbuffer: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
                let mut view = None;
                self.device
                    .CreateRenderTargetView(&backbuffer, None, Some(&mut view))?;
                self.view = view;
            }
            let Some(view) = self.view.clone() else {
                return Ok(());
            };
            // Premultiplied, and the grid is opaque, so the clear color is the
            // terminal background as-is.
            let [r, g, b] = [
                ((background >> 16) & 0xff) as f32 / 255.0,
                ((background >> 8) & 0xff) as f32 / 255.0,
                (background & 0xff) as f32 / 255.0,
            ];
            self.context
                .ClearRenderTargetView(&view, &[srgb(r), srgb(g), srgb(b), 1.0]);
            self.swapchain.Present(0, Default::default()).ok()?;
        }
        Ok(())
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// `ClearRenderTargetView` takes linear values while the swapchain is UNORM,
/// so an sRGB color has to be linearized or every fill comes out too light.
fn srgb(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
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
        AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
        ..Default::default()
    }
}

fn create_device() -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        let mut context = None;
        let result = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                None,
                // BGRA support is required by DirectComposition.
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
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

/// A child window that owns nothing but the composition target.
///
/// `WS_EX_NOREDIRECTIONBITMAP` keeps GDI from allocating a redirection surface
/// for it — the window never paints, the compositor does.
fn create_surface_window(parent: HWND) -> windows::core::Result<HWND> {
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

/// Input belongs to the shell's own hit-testing, which works in the parent's
/// coordinates — so this window declines to be hit at all.
unsafe extern "system" fn surface_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::{HTTRANSPARENT, WM_ERASEBKGND, WM_NCHITTEST};
    match message {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        // Nothing to erase: the compositor owns every pixel.
        WM_ERASEBKGND => LRESULT(1),
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}
