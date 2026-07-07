//! Window management via the Accessibility API. A window id is a `CGWindowID`;
//! we map it to its owning app's AX window by matching position and size, then
//! drive `AXPosition`/`AXSize`/`AXMinimized`/`AXRaise`/close-button. Requires the
//! Accessibility permission (mutating a foreign app's windows).

use super::axui::{AxEl, require_trusted};
use super::{display_for_rect, displays, parse_window_id};
use crate::error::{Error, Result};
use crate::model::{Window, WindowTarget};

/// Resolve a target to its current `CGWindow` record.
fn resolve(target: &WindowTarget) -> Result<Window> {
    match target {
        WindowTarget::Id(id) => {
            let wid = parse_window_id(id)?;
            super::window_record(wid)
                .ok_or_else(|| Error::Stale(format!("window {id} no longer exists")))
        }
        WindowTarget::Match(query) => {
            let mut wins = super::windows(query)?;
            match wins.len() {
                0 => Err(Error::NotFound("no window matched the query".into())),
                1 => Ok(wins.remove(0)),
                n => Err(Error::Ambiguous(format!(
                    "{n} windows matched; refine --match or use --window <id>"
                ))),
            }
        }
    }
}

/// Map a `CGWindowID` string to its AX window element by matching geometry
/// within the owning application.
pub(super) fn ax_window_for_id(window_id: &str) -> Result<AxEl> {
    let wid = parse_window_id(window_id)?;
    let target = super::window_record(wid)
        .ok_or_else(|| Error::Stale(format!("window {window_id} no longer exists")))?;
    require_trusted()?;
    let app = AxEl::for_app(target.pid as i32)?;
    let ax_windows = app.windows();
    // Exact bridge first: the AX window whose CGWindowID equals the target.
    for w in &ax_windows {
        if w.window_id() == Some(wid) {
            return Ok(w.clone_ref());
        }
    }
    // Fall back to geometry when the private id bridge is unavailable.
    let mut best: Option<(f64, AxEl)> = None;
    for w in ax_windows {
        let (Some(pos), Some(size)) = (w.attr_point("AXPosition"), w.attr_size("AXSize")) else {
            continue;
        };
        let dx = pos.x - target.bounds.x as f64;
        let dy = pos.y - target.bounds.y as f64;
        let dw = size.width - target.bounds.w as f64;
        let dh = size.height - target.bounds.h as f64;
        let score = dx * dx + dy * dy + dw * dw + dh * dh;
        if best.as_ref().is_none_or(|(s, _)| score < *s) {
            best = Some((score, w));
        }
    }
    best.map(|(_, w)| w)
        .ok_or_else(|| Error::NotFound(format!("no AX window matched {window_id}")))
}

/// Update a snapshot's geometry from the AX element, which reflects a
/// position/size change immediately — unlike CGWindowList, whose compositor
/// snapshot lags a move/resize by a frame or two.
fn with_ax_geometry(ax: &AxEl, mut w: Window) -> Window {
    if let Some(p) = ax.attr_point("AXPosition") {
        w.bounds.x = p.x.round() as i32;
        w.bounds.y = p.y.round() as i32;
    }
    if let Some(s) = ax.attr_size("AXSize") {
        w.bounds.w = s.width.round() as i32;
        w.bounds.h = s.height.round() as i32;
    }
    w
}

pub fn status(target: &WindowTarget) -> Result<Window> {
    resolve(target)
}

pub fn focus(target: &WindowTarget) -> Result<Window> {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    let w = resolve(target)?;
    require_trusted()?;
    // Activate the owning app: setting AXFrontmost alone does not steal the
    // foreground from another process, so keyboard input would go elsewhere.
    // NSRunningApplication.activate is the reliable cross-process path.
    if let Some(app) =
        NSRunningApplication::runningApplicationWithProcessIdentifier(w.pid as libc::pid_t)
    {
        app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
    }
    // Also set app-level frontmost via AX for apps that honor it, then raise
    // this specific window and make it the main one.
    let _ = AxEl::for_app(w.pid as i32)?.set_bool("AXFrontmost", true);
    let ax = ax_window_for_id(&w.id)?;
    let _ = ax.perform("AXRaise");
    let _ = ax.set_bool("AXMain", true);
    Ok(with_ax_geometry(&ax, w))
}

pub fn activate(target: &WindowTarget) -> Result<Window> {
    focus(target)
}

pub fn raise(target: &WindowTarget) -> Result<Window> {
    let w = resolve(target)?;
    let ax = ax_window_for_id(&w.id)?;
    // Best-effort: some windows (e.g. Calculator's fixed panel) don't expose the
    // AXRaise action. Treat "unsupported" as a no-op rather than a hard failure.
    let _ = ax.perform("AXRaise");
    Ok(with_ax_geometry(&ax, w))
}

pub fn move_to(target: &WindowTarget, x: i32, y: i32) -> Result<Window> {
    let w = resolve(target)?;
    let ax = ax_window_for_id(&w.id)?;
    ax.set_point("AXPosition", x as f64, y as f64)?;
    Ok(with_ax_geometry(&ax, w))
}

pub fn move_to_display(target: &WindowTarget, display_id: &str) -> Result<Window> {
    let w = resolve(target)?;
    let ds = displays()?;
    let d = ds
        .iter()
        .find(|d| d.id == display_id)
        .ok_or_else(|| Error::NotFound(format!("no display {display_id}")))?;
    let ax = ax_window_for_id(&w.id)?;
    ax.set_point("AXPosition", d.work_area.x as f64, d.work_area.y as f64)?;
    Ok(with_ax_geometry(&ax, w))
}

pub fn resize(target: &WindowTarget, width: i32, height: i32) -> Result<Window> {
    if width <= 0 || height <= 0 {
        return Err(Error::Usage("width/height must be positive".into()));
    }
    let w = resolve(target)?;
    let ax = ax_window_for_id(&w.id)?;
    ax.set_size("AXSize", width as f64, height as f64)?;
    Ok(with_ax_geometry(&ax, w))
}

pub fn minimize(target: &WindowTarget) -> Result<Window> {
    let w = resolve(target)?;
    let ax = ax_window_for_id(&w.id)?;
    ax.set_bool("AXMinimized", true)?;
    // A minimized window leaves the on-screen list, so re-querying would either
    // miss it or (mid-animation) still report it as non-minimized. Report the
    // state we just set directly.
    Ok(Window {
        minimized: true,
        visible: false,
        ..w
    })
}

pub fn restore(target: &WindowTarget) -> Result<Window> {
    let w = resolve(target)?;
    let ax = ax_window_for_id(&w.id)?;
    ax.set_bool("AXMinimized", false)?;
    Ok(with_ax_geometry(&ax, w))
}

/// macOS has no true "maximize"; approximate it by filling the window's display
/// work area (best-effort; the app may clamp to its own min/max size).
pub fn maximize(target: &WindowTarget) -> Result<Window> {
    let w = resolve(target)?;
    let ds = displays()?;
    let (id, ..) = display_for_rect(&ds, &w.bounds);
    let d = ds
        .iter()
        .find(|d| d.id == id)
        .or_else(|| ds.first())
        .ok_or_else(|| Error::Unavailable("no display for window".into()))?;
    let ax = ax_window_for_id(&w.id)?;
    ax.set_bool("AXMinimized", false)?;
    ax.set_point("AXPosition", d.work_area.x as f64, d.work_area.y as f64)?;
    ax.set_size("AXSize", d.work_area.w as f64, d.work_area.h as f64)?;
    Ok(with_ax_geometry(&ax, w))
}

/// AX cannot set another app's window level, so always-on-top is unsupported.
pub fn set_always_on_top(_target: &WindowTarget, _on: bool) -> Result<Window> {
    Err(Error::Unsupported(
        "always-on-top is not settable through the macOS Accessibility API".into(),
    ))
}

/// Ask a window to close by pressing its AX close button. Destructive.
pub fn close(target: &WindowTarget) -> Result<Window> {
    let w = resolve(target)?;
    let ax = ax_window_for_id(&w.id)?;
    let button = ax
        .attr_element("AXCloseButton")
        .ok_or_else(|| Error::Unsupported("window has no close button".into()))?;
    button.perform("AXPress")?;
    // Report the pre-close snapshot; the window is likely gone by now.
    Ok(w)
}
