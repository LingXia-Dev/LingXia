//! DirectComposition tree for one composition-hosted WebView2 surface.

use super::*;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice3, IDCompositionDesktopDevice, IDCompositionRectangleClip,
    IDCompositionTarget, IDCompositionVisual, IDCompositionVisual2,
};

/// Device → surface-HWND target → root visual (carries the workspace clip) →
/// webview visual (handed to WebView2 as `RootVisualTarget`).
pub(crate) struct DcompTree {
    device: IDCompositionDesktopDevice,
    /// Owns the HWND binding; dropping it detaches the tree.
    _target: IDCompositionTarget,
    root: IDCompositionVisual2,
    webview_visual: IDCompositionVisual2,
    clip: IDCompositionRectangleClip,
}

impl DcompTree {
    pub(crate) fn new(surface: HWND) -> StdResult<Self> {
        unsafe {
            // No rendering device: WebView2 supplies all content; the tree
            // never creates DComp surfaces of its own.
            let device: IDCompositionDesktopDevice =
                DCompositionCreateDevice3(None::<&windows::core::IUnknown>)
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
                _target: target,
                root,
                webview_visual,
                clip,
            })
        }
    }

    pub(crate) fn webview_visual(&self) -> &IDCompositionVisual2 {
        &self.webview_visual
    }

    /// Clips the tree to `(0, 0, width, height)` with per-corner radii
    /// `[tl, tr, br, bl]` and commits.
    pub(crate) fn apply_clip(&self, width: i32, height: i32, radii: [i32; 4]) -> StdResult<()> {
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
                .and_then(|_| self.device.Commit())
                .map_err(|err| dcomp_error("clip update", err))
        }
    }
}

fn dcomp_error(what: &str, err: windows::core::Error) -> WebViewError {
    WebViewError::WebView(format!("{what} failed: {err}"))
}
