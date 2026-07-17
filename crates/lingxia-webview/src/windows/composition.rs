//! WebView2 composition hosting.
//!
//! Each webview owns a `LingXiaWebViewSurface` child HWND created on its
//! dedicated UI thread, with a DirectComposition device/target/visual tree
//! bound to it; WebView2 renders into the tree through `RootVisualTarget`.
//! Reparenting moves the child HWND with Win32 `SetParent` — the DComp target
//! travels with the window, so surfaces keep their frames across host
//! switches — and the root visual's rectangle clip rounds whichever workspace
//! corners the shell assigns. Creation falls back to the windowed controller
//! when the runtime or the DComp setup cannot deliver composition.

use super::*;

mod dcomp;
mod dragdrop;
mod pointer;
mod surface_window;

use dcomp::DcompTree;

/// How a webview's controller is attached to the host window tree.
pub(crate) enum HostingMode {
    /// Classic windowed `ICoreWebView2Controller`: WebView2 owns a
    /// rectangular child HWND positioned in host client coordinates.
    Windowed,
    /// Composition-hosted controller rendering into [`CompositionSurface`].
    Composition(Box<CompositionSurface>),
}

pub(crate) struct CompositionSurface {
    /// The `LingXiaWebViewSurface` child window. Created on — and therefore
    /// destroyed with — this webview's UI thread. Holds the input-forwarding
    /// state (with the composition-interface clone of the controller) in its
    /// window user data.
    pub(crate) hwnd: HWND,
    dcomp: DcompTree,
    /// Last applied per-corner clip radii `[tl, tr, br, bl]`, physical px.
    radii: [i32; 4],
    /// Last applied wedge backdrop color (`0xAARGB`; alpha 0 = no wedges).
    corner_color: u32,
}

static COMPOSITION_HOSTING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Programmatic default for WebView2 composition hosting (on unless changed).
/// The `LINGXIA_WEBVIEW_COMPOSITION` env var (`0|false|off` / `1|true|on`)
/// overrides the default per process.
pub fn set_webview_composition_hosting(enabled: bool) {
    COMPOSITION_HOSTING.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

fn composition_hosting_enabled() -> bool {
    let configured = || COMPOSITION_HOSTING.load(std::sync::atomic::Ordering::Relaxed);
    match std::env::var("LINGXIA_WEBVIEW_COMPOSITION") {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "0" | "false" | "off" => false,
            "1" | "true" | "on" => true,
            _ => configured(),
        },
        Err(_) => configured(),
    }
}

/// Creates the controller for the configured hosting mode. A composition
/// failure (old WebView2 runtime, DComp setup error) downgrades to the
/// windowed controller so webview creation itself never regresses.
pub(crate) fn create_hosting_controller(
    env: &ICoreWebView2Environment,
    parent: HWND,
) -> StdResult<(ICoreWebView2Controller, HostingMode)> {
    if composition_hosting_enabled() {
        match create_composition_surface(env, parent) {
            Ok((controller, surface)) => {
                return Ok((controller, HostingMode::Composition(surface)));
            }
            Err(err) => {
                log::warn!("composition hosting unavailable; using windowed WebView2: {err}");
            }
        }
    }
    Ok((create_controller(env, parent)?, HostingMode::Windowed))
}

fn create_composition_surface(
    env: &ICoreWebView2Environment,
    parent: HWND,
) -> StdResult<(ICoreWebView2Controller, Box<CompositionSurface>)> {
    let env3: ICoreWebView2Environment3 = env.cast().map_err(|err| {
        WebViewError::WebView(format!("WebView2 runtime lacks composition hosting: {err}"))
    })?;
    let mut bounds = RECT::default();
    unsafe {
        WindowsAndMessaging::GetClientRect(parent, &mut bounds)
            .map_err(|err| WebViewError::WebView(format!("GetClientRect failed: {err}")))?;
    }
    let hwnd = surface_window::create_surface_window(parent, bounds)?;
    let assembled = (|| {
        let dcomp = DcompTree::new(hwnd)?;
        let controller = create_composition_controller(&env3, hwnd)?;
        unsafe {
            controller
                .SetRootVisualTarget(dcomp.webview_visual())
                .map_err(|err| {
                    WebViewError::WebView(format!("SetRootVisualTarget failed: {err}"))
                })?;
        }
        let base: ICoreWebView2Controller = controller.cast().map_err(|err| {
            WebViewError::WebView(format!("composition controller cast failed: {err}"))
        })?;
        surface_window::attach_input(hwnd, &env3, &controller, &base);
        dragdrop::register_drop_target(hwnd, &controller);
        Ok((
            base,
            Box::new(CompositionSurface {
                hwnd,
                dcomp,
                radii: [0; 4],
                corner_color: 0,
            }),
        ))
    })();
    if assembled.is_err() {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd);
        }
    }
    assembled
}

fn create_composition_controller(
    env3: &ICoreWebView2Environment3,
    hwnd: HWND,
) -> StdResult<ICoreWebView2CompositionController> {
    let env3 = env3.clone();
    let (tx, rx) = mpsc::channel();

    CreateCoreWebView2CompositionControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            env3.CreateCoreWebView2CompositionController(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |result, controller| {
            result?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .map_err(|_| windows::core::Error::from(E_POINTER))?;
            Ok(())
        }),
    )
    .map_err(map_webview2_error)?;

    rx.recv()
        .map_err(|_| {
            WebViewError::WebView("Composition controller callback channel failed".to_string())
        })?
        .map_err(|err| {
            WebViewError::WebView(format!("Composition controller creation failed: {err}"))
        })
}

impl CompositionSurface {
    /// Positions the surface window at `bounds` (host client coordinates),
    /// sizes the controller to match, and re-applies the corner clip and
    /// wedges — one commit, so bounds and corners never present out of sync.
    /// `corners` of `None` keeps the last applied style.
    pub(crate) fn set_geometry(
        &mut self,
        base: &ICoreWebView2Controller,
        bounds: RECT,
        corners: Option<([i32; 4], u32)>,
    ) -> StdResult<()> {
        let (radii, corner_color) = corners.unwrap_or((self.radii, self.corner_color));
        let width = (bounds.right - bounds.left).max(0);
        let height = (bounds.bottom - bounds.top).max(0);
        unsafe {
            WindowsAndMessaging::SetWindowPos(
                self.hwnd,
                None,
                bounds.left,
                bounds.top,
                width,
                height,
                WindowsAndMessaging::SWP_NOZORDER | WindowsAndMessaging::SWP_NOACTIVATE,
            )
            .map_err(|err| WebViewError::WebView(format!("SetWindowPos failed: {err}")))?;
            base.SetBounds(RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            })
            .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        }
        self.radii = radii;
        self.corner_color = corner_color;
        self.dcomp
            .apply_geometry(width, height, radii, corner_color)
    }

    /// Shows the surface window before the controller (so composition starts
    /// against a visible window) and hides the controller before the window
    /// (so no live frame flashes through the hide).
    pub(crate) fn set_visible(
        &self,
        base: &ICoreWebView2Controller,
        visible: bool,
    ) -> StdResult<()> {
        unsafe {
            if visible {
                let _ = WindowsAndMessaging::ShowWindow(self.hwnd, WindowsAndMessaging::SW_SHOWNA);
            }
            let result = base
                .SetIsVisible(visible)
                .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")));
            if !visible {
                let _ = WindowsAndMessaging::ShowWindow(self.hwnd, WindowsAndMessaging::SW_HIDE);
            }
            result
        }
    }

    /// Moves the surface window under a new host. WebView2's own parent stays
    /// the surface window, so its composition target survives the move — no
    /// blank frame, unlike the windowed controller's `SetParentWindow`.
    pub(crate) fn set_parent(&self, parent: HWND) -> StdResult<()> {
        unsafe {
            WindowsAndMessaging::SetParent(self.hwnd, Some(parent))
                .map_err(|err| WebViewError::WebView(format!("SetParent failed: {err}")))?;
            // Sit beneath native-component siblings, matching the windowed
            // controller's placement; the shell paints chrome on the host
            // window itself, below all children.
            let _ = WindowsAndMessaging::SetWindowPos(
                self.hwnd,
                Some(WindowsAndMessaging::HWND_BOTTOM),
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOACTIVATE,
            );
        }
        Ok(())
    }

    /// Destroys the surface window. Must run on the webview's UI thread (the
    /// window's owner); called from `cleanup_state` after `Controller.Close`.
    pub(crate) fn destroy(&self) {
        dragdrop::revoke_drop_target(self.hwnd);
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(self.hwnd);
        }
    }
}
