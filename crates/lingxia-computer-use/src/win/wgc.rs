//! Windows.Graphics.Capture window capture. Captures the real composited
//! output of a window (works for GPU/hardware-composited windows where
//! `PrintWindow` returns black frames). Returns raw RGBA + size; the caller
//! encodes PNG. Falls back to PrintWindow at the call site on any error.

use crate::error::{Error, Result};
use std::sync::mpsc;
use std::time::Duration;
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ,
    D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::{IInspectable, Interface};

/// Captured RGBA pixels for a window.
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

fn fail(msg: impl std::fmt::Display) -> Error {
    Error::Failed(format!("wgc: {msg}"))
}

/// WGC uses WinRT, which requires COM initialized on the calling thread. The
/// FrameArrived event is delivered on an MTA threadpool thread, so we init MTA.
/// Idempotent: an already-initialized thread returns S_FALSE / RPC_E_CHANGED_MODE,
/// both of which are fine for our use.
fn ensure_com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

pub fn capture_window(hwnd: HWND) -> Result<Rgba> {
    ensure_com();
    unsafe {
        // D3D11 device (BGRA support required for WGC).
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|e| fail(format!("D3D11CreateDevice: {e}")))?;
        let device = device.ok_or_else(|| fail("no d3d device"))?;
        let context = context.ok_or_else(|| fail("no d3d context"))?;

        // WinRT IDirect3DDevice from the DXGI device.
        let dxgi: IDXGIDevice = device.cast().map_err(|e| fail(format!("dxgi cast: {e}")))?;
        let inspectable: IInspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
            .map_err(|e| fail(format!("interop device: {e}")))?;
        let rt_device: IDirect3DDevice = inspectable
            .cast()
            .map_err(|e| fail(format!("rt device cast: {e}")))?;

        // Capture item for the window.
        let interop: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
                .map_err(|e| fail(format!("item interop factory: {e}")))?;
        let item: GraphicsCaptureItem = interop
            .CreateForWindow(hwnd)
            .map_err(|e| fail(format!("CreateForWindow: {e}")))?;
        let size = item.Size().map_err(|e| fail(format!("item size: {e}")))?;
        if size.Width <= 0 || size.Height <= 0 {
            return Err(fail("window has zero capture size"));
        }

        // Frame pool + session; grab the first frame via the FrameArrived event
        // (delivered on an MTA threadpool thread, so a channel works).
        // FreeThreaded so FrameArrived is delivered on a threadpool thread; the
        // CLI thread has no DispatcherQueue/message pump to service plain Create.
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &rt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(|e| fail(format!("frame pool: {e}")))?;
        let session = pool
            .CreateCaptureSession(&item)
            .map_err(|e| fail(format!("capture session: {e}")))?;

        let (tx, rx) = mpsc::channel();
        let token = pool
            .FrameArrived(
                &TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(
                    move |pool, _| {
                        if let Some(pool) = pool.as_ref()
                            && let Ok(frame) = pool.TryGetNextFrame()
                        {
                            let _ = tx.send(frame);
                        }
                        Ok(())
                    },
                ),
            )
            .map_err(|e| fail(format!("FrameArrived: {e}")))?;

        session
            .StartCapture()
            .map_err(|e| fail(format!("StartCapture: {e}")))?;

        let frame = rx
            .recv_timeout(Duration::from_millis(1500))
            .map_err(|_| Error::Timeout("wgc: no frame within timeout".into()))?;

        let _ = pool.RemoveFrameArrived(token);

        // Frame surface -> ID3D11Texture2D.
        let surface = frame.Surface().map_err(|e| fail(format!("surface: {e}")))?;
        let access: IDirect3DDxgiInterfaceAccess = surface
            .cast()
            .map_err(|e| fail(format!("dxgi access: {e}")))?;
        let texture: ID3D11Texture2D = access
            .GetInterface()
            .map_err(|e| fail(format!("get texture: {e}")))?;

        // Copy into a CPU-readable staging texture.
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut desc);
        let width = desc.Width;
        let height = desc.Height;
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;

        let mut staging: Option<ID3D11Texture2D> = None;
        device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .map_err(|e| fail(format!("staging texture: {e}")))?;
        let staging = staging.ok_or_else(|| fail("no staging texture"))?;
        context.CopyResource(&staging, &texture);

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|e| fail(format!("map: {e}")))?;

        // Copy row by row (RowPitch may exceed width*4), BGRA -> RGBA.
        let row_pitch = mapped.RowPitch as usize;
        let src = mapped.pData as *const u8;
        let w = width as usize;
        let h = height as usize;
        let mut pixels = vec![0u8; w * h * 4];
        for y in 0..h {
            let row = std::slice::from_raw_parts(src.add(y * row_pitch), w * 4);
            let dst = &mut pixels[y * w * 4..(y + 1) * w * 4];
            for x in 0..w {
                dst[x * 4] = row[x * 4 + 2]; // R <- B
                dst[x * 4 + 1] = row[x * 4 + 1]; // G
                dst[x * 4 + 2] = row[x * 4]; // B <- R
                dst[x * 4 + 3] = 255; // opaque
            }
        }
        context.Unmap(&staging, 0);
        let _ = session.Close();
        let _ = pool.Close();

        Ok(Rgba {
            width,
            height,
            pixels,
        })
    }
}
