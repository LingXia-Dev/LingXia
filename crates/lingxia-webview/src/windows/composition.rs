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
    /// The `LingXiaWebViewSurface` child window. Created on this webview's
    /// UI thread, but parented into foreign host windows — if such a host is
    /// destroyed, the surface dies with it and is recreated on the next
    /// reparent (see [`CompositionSurface::ensure_alive`]). Holds the
    /// input-forwarding state in its window user data.
    pub(crate) hwnd: HWND,
    env3: ICoreWebView2Environment3,
    controller: ICoreWebView2CompositionController,
    dcomp: DcompTree,
    /// Controller-level event subscriptions owned by the current surface
    /// window; removed before a recreation re-subscribes.
    input_tokens: surface_window::InputSubscriptions,
    /// The host the surface currently lives under, so a recreation from the
    /// geometry/visibility paths knows where to rebuild.
    parent: HWND,
    /// Last applied per-corner clip radii `[tl, tr, br, bl]`, physical px.
    radii: [i32; 4],
    /// Last applied wedge backdrop color (`0xAARGB`; alpha 0 = no wedges).
    corner_color: u32,
    /// Last applied bounds/visibility, replayed after a recreation.
    bounds: RECT,
    visible: bool,
}

static COMPOSITION_HOSTING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Programmatic default for WebView2 composition hosting (on unless changed).
/// The `LINGXIA_WEBVIEW_COMPOSITION` env var (`0|false|off` / `1|true|on`)
/// overrides the default per process.
pub fn set_webview_composition_hosting(enabled: bool) {
    COMPOSITION_HOSTING.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// True when new webviews will attempt composition hosting. Host chrome
/// keys workarounds off this — e.g. the device frame drops its corner-mask
/// overlay and region clip when the composition corner wedges replace them.
pub fn webview_composition_hosting_enabled() -> bool {
    composition_hosting_enabled()
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
        let input_tokens = attach_surface(hwnd, &dcomp, &env3, &controller)?;
        let base: ICoreWebView2Controller = controller.cast().map_err(|err| {
            WebViewError::WebView(format!("composition controller cast failed: {err}"))
        })?;
        Ok((
            base,
            Box::new(CompositionSurface {
                hwnd,
                env3,
                controller,
                dcomp,
                input_tokens,
                parent,
                radii: [0; 4],
                corner_color: 0,
                bounds,
                visible: false,
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

/// Binds a surface window to the controller: visual target, input
/// forwarding, drag-and-drop. Shared by creation and post-teardown
/// recreation.
fn attach_surface(
    hwnd: HWND,
    dcomp: &DcompTree,
    env3: &ICoreWebView2Environment3,
    controller: &ICoreWebView2CompositionController,
) -> StdResult<surface_window::InputSubscriptions> {
    unsafe {
        controller
            .SetRootVisualTarget(dcomp.webview_visual())
            .map_err(|err| WebViewError::WebView(format!("SetRootVisualTarget failed: {err}")))?;
    }
    let base: ICoreWebView2Controller = controller.cast().map_err(|err| {
        WebViewError::WebView(format!("composition controller cast failed: {err}"))
    })?;
    let tokens = surface_window::attach_input(hwnd, env3, controller, &base);
    dragdrop::register_drop_target(hwnd, controller);
    Ok(tokens)
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
    /// Recreates the surface window under `parent` when the previous one was
    /// destroyed (its former host window was torn down — DestroyWindow kills
    /// reparented children), replaying the last applied geometry and
    /// visibility. Returns `true` when a recreation happened.
    fn ensure_alive(&mut self, parent: HWND, base: &ICoreWebView2Controller) -> StdResult<bool> {
        if unsafe { WindowsAndMessaging::IsWindow(Some(self.hwnd)).as_bool() } {
            return Ok(false);
        }
        log::info!("composition surface window died with its former host; recreating");
        // The dead window's controller-level subscriptions outlive it; drop
        // them before attach_surface re-subscribes, or handler chains grow
        // with every recovery.
        surface_window::detach_input(&self.controller, base, self.input_tokens);
        self.input_tokens = surface_window::InputSubscriptions::default();
        let hwnd = surface_window::create_surface_window(parent, self.bounds)?;
        let rebuilt = (|| {
            let dcomp = DcompTree::new(hwnd)?;
            let tokens = attach_surface(hwnd, &dcomp, &self.env3, &self.controller)?;
            Ok((dcomp, tokens))
        })();
        let (dcomp, tokens) = match rebuilt {
            Ok(parts) => parts,
            Err(err) => {
                unsafe {
                    let _ = WindowsAndMessaging::DestroyWindow(hwnd);
                }
                return Err(err);
            }
        };
        self.hwnd = hwnd;
        self.dcomp = dcomp;
        self.input_tokens = tokens;
        self.parent = parent;
        let bounds = self.bounds;
        self.set_geometry(base, bounds, None)?;
        if self.visible {
            self.set_visible(base, true)?;
        }
        Ok(true)
    }

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
        // Self-heal here too: the main surface's own parent window never gets
        // a reparent command, so a surface killed elsewhere would otherwise
        // stay dead when re-shown standalone.
        if !unsafe { WindowsAndMessaging::IsWindow(Some(self.hwnd)).as_bool() } {
            self.bounds = bounds;
            if let Some((radii, corner_color)) = corners {
                self.radii = radii;
                self.corner_color = corner_color;
            }
            let parent = self.parent;
            return self.ensure_alive(parent, base).map(|_| ());
        }
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
        self.bounds = bounds;
        self.radii = radii;
        self.corner_color = corner_color;
        self.dcomp
            .apply_geometry(width, height, radii, corner_color)
    }

    /// Visibility is window-level first: hiding only hides the surface
    /// window and leaves the controller rendering through a grace timer, so
    /// a quick hide→show cycle (tab switches) re-reveals a live frame
    /// instead of flashing the card while WebView2 restarts presentation.
    /// The timer suspends long-hidden controllers to stop background
    /// rasterization.
    pub(crate) fn set_visible(
        &mut self,
        base: &ICoreWebView2Controller,
        visible: bool,
    ) -> StdResult<()> {
        self.visible = visible;
        unsafe {
            if visible {
                surface_window::cancel_hide_suspend(self.hwnd);
                let result = base
                    .SetIsVisible(true)
                    .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")));
                let _ = WindowsAndMessaging::ShowWindow(self.hwnd, WindowsAndMessaging::SW_SHOWNA);
                result
            } else {
                let _ = WindowsAndMessaging::ShowWindow(self.hwnd, WindowsAndMessaging::SW_HIDE);
                surface_window::schedule_hide_suspend(self.hwnd);
                Ok(())
            }
        }
    }

    /// Moves the surface window under a new host. WebView2's own parent stays
    /// the surface window, so its composition target survives the move — no
    /// blank frame, unlike the windowed controller's `SetParentWindow`. A
    /// surface killed by its former host's teardown is recreated here.
    pub(crate) fn set_parent(
        &mut self,
        base: &ICoreWebView2Controller,
        parent: HWND,
    ) -> StdResult<()> {
        unsafe {
            if !self.ensure_alive(parent, base)? {
                WindowsAndMessaging::SetParent(self.hwnd, Some(parent))
                    .map_err(|err| WebViewError::WebView(format!("SetParent failed: {err}")))?;
            }
            self.parent = parent;
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
    /// The wndproc's WM_DESTROY arm revokes the OLE drop target.
    pub(crate) fn destroy(&self) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(self.hwnd);
        }
    }
}
