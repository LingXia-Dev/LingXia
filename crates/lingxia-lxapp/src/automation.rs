//! Shared lower half for lxapp automation.
//!
//! Both automation front-ends — the devtool (`lxdev`) handlers and the
//! in-process `lx.automation()` JS API — delegate here, so target resolution,
//! navigation semantics (tab-bar guard, query append), and the DOM query
//! script can never drift between them. Errors are plain strings; each
//! front-end maps them into its own error type.

use crate::lxapp::LxApp;
use crate::startup::split_path_query;
use crate::{NavigationType, PageInstance};
use lingxia_webview::WebView;
use serde_json::Value;
use std::sync::Arc;

/// Snapshot of one page's runtime state, shared by both front-ends.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageStatus {
    /// App id that owns the page.
    pub appid: String,
    /// Declarative page name from the manifest, when available.
    pub name: Option<String>,
    /// Runtime page path.
    pub path: String,
    /// Whether this page is the current foreground page.
    pub current: bool,
    /// Whether this page is in the navigation stack.
    pub in_stack: bool,
    /// Whether the page currently has an attached WebView.
    pub ready: bool,
}

/// Resolve a running lxapp by id; empty or "current" means the active app.
pub fn resolve_lxapp(raw: &str) -> Result<Arc<LxApp>, String> {
    let trimmed = raw.trim();
    let appid = if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("current") {
        let (appid, _, _) = crate::lxapp::get_current_lxapp();
        if appid.is_empty() {
            return Err("no current lxapp".to_string());
        }
        appid
    } else {
        trimmed.to_string()
    };
    crate::lxapp::try_get(&appid).ok_or_else(|| format!("lxapp is not active: {appid}"))
}

/// Resolve a page by configured name; `None`/"current" means the current page.
/// Returns the page and its configured name (when it has one).
pub fn resolve_page(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
) -> Result<(PageInstance, Option<String>), String> {
    let name = page_name.map(str::trim).filter(|value| !value.is_empty());
    match name {
        None => {
            let page = app.current_page().map_err(|err| err.to_string())?;
            let name = page_name_for_path(app, &page.path());
            Ok((page, name))
        }
        Some(n) if n.eq_ignore_ascii_case("current") => {
            let page = app.current_page().map_err(|err| err.to_string())?;
            let name = page_name_for_path(app, &page.path());
            Ok((page, name))
        }
        Some(n) => {
            if let Some(page) = app.get_page_by_instance_id_str(n) {
                let name = page_name_for_path(app, &page.path());
                return Ok((page, name));
            }
            let path = app
                .find_page_path_by_name(n)
                .ok_or_else(|| format!("unknown page name: {n}"))?;
            let page = resolve_active_page_by_path(app, &path)
                .ok_or_else(|| format!("page is not active: {n}"))?;
            Ok((page, Some(n.to_string())))
        }
    }
}

/// Resolve a page's attached WebView, erroring while it is not ready.
pub fn resolve_webview(app: &Arc<LxApp>, page_name: Option<&str>) -> Result<Arc<WebView>, String> {
    let (page, _) = resolve_page(app, page_name)?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())
}

/// Whether the configured page name exists in the app's manifest at all
/// (regardless of runtime state). `None`/"current" counts as known.
pub fn page_name_known(app: &Arc<LxApp>, page_name: Option<&str>) -> bool {
    match page_name.map(str::trim).filter(|value| !value.is_empty()) {
        None => true,
        Some(n) if n.eq_ignore_ascii_case("current") => true,
        Some(n) => {
            app.get_page_by_instance_id_str(n).is_some() || app.find_page_path_by_name(n).is_some()
        }
    }
}

fn resolve_active_page_by_path(app: &Arc<LxApp>, path: &str) -> Option<PageInstance> {
    if let Ok(page) = app.require_page(path) {
        return Some(page);
    }
    let info = app.runtime_info();
    info.current_page
        .iter()
        .chain(info.page_stack.iter().rev())
        .find(|candidate| page_paths_match(candidate, path))
        .and_then(|candidate| app.get_page(candidate))
}

fn page_path_key(path: &str) -> String {
    let (path, _) = split_path_query(path);
    path.trim_start_matches('/').to_string()
}

/// Whether two page paths refer to the same page, ignoring query and leading `/`.
pub fn page_paths_match(left: &str, right: &str) -> bool {
    page_path_key(left) == page_path_key(right)
}

/// The configured page name for a runtime path, when one exists.
pub fn page_name_for_path(app: &Arc<LxApp>, path: &str) -> Option<String> {
    app.runtime_info()
        .page_entries
        .into_iter()
        .find(|entry| page_paths_match(&entry.path, path))
        .map(|entry| entry.name)
        .filter(|name| !name.is_empty())
}

/// Build a [`PageStatus`] for one resolved page.
pub fn page_status(app: &Arc<LxApp>, page: &PageInstance, name: Option<&str>) -> PageStatus {
    let info = app.runtime_info();
    let path = page.path();
    PageStatus {
        appid: info.appid,
        name: name.map(str::to_string),
        current: info
            .current_page
            .as_deref()
            .is_some_and(|current| page_paths_match(current, &path)),
        in_stack: info
            .page_stack
            .iter()
            .any(|stack_page| page_paths_match(stack_page, &path)),
        ready: page.webview().is_some(),
        path,
    }
}

/// [`PageStatus`] for every configured page of the app.
pub fn list_page_statuses(app: &Arc<LxApp>) -> Vec<PageStatus> {
    let info = app.runtime_info();
    info.page_entries
        .iter()
        .map(|entry| PageStatus {
            appid: info.appid.clone(),
            name: (!entry.name.is_empty()).then(|| entry.name.clone()),
            path: entry.path.clone(),
            current: info
                .current_page
                .as_deref()
                .is_some_and(|current| page_paths_match(current, &entry.path)),
            in_stack: info
                .page_stack
                .iter()
                .any(|stack_page| page_paths_match(stack_page, &entry.path)),
            ready: app
                .require_page(&entry.path)
                .ok()
                .is_some_and(|page| page.webview().is_some()),
        })
        .collect()
}

// ===================== navigation =====================

fn normalize_tabbar_path(url: &str) -> String {
    let (path, _) = split_path_query(url);
    let mut trimmed = path.trim_start_matches('/').to_string();
    if let Some(dot_pos) = trimmed.rfind('.')
        && trimmed.rfind('/').is_none_or(|slash| dot_pos > slash)
    {
        trimmed.truncate(dot_pos);
    }
    trimmed
}

fn is_tabbar_page_url(app: &LxApp, url: &str) -> bool {
    let Some(tabbar) = app.get_tabbar() else {
        return false;
    };
    let target = normalize_tabbar_path(url);
    tabbar
        .list
        .iter()
        .any(|item| normalize_tabbar_path(&item.pagePath) == target)
}

/// Navigate the app's page stack to a configured page by name and return the
/// landed page (+ configured name).
///
/// Enforces the same rules as the runtime navigation APIs: `Replace`
/// (redirect) may not target a tab-bar page, and `query` is appended to the
/// resolved page path.
///
/// `wait_ready` awaits the destination WebView before returning — correct for
/// an off-thread caller (the devtool) that wants a settled page. An in-process
/// caller (`lx.automation()` from an lxapp's own Logic) **must** pass `false`:
/// awaiting readiness there deadlocks, since the single logic thread that would
/// signal readiness is the one blocked on this call. That matches the
/// fire-and-forget semantics of `lx.navigateTo`.
pub async fn navigate(
    app: &Arc<LxApp>,
    page_name: &str,
    query: Option<&Value>,
    kind: NavigationType,
    wait_ready: bool,
) -> Result<(PageInstance, Option<String>), String> {
    let page_name = page_name.trim();
    if page_name.is_empty() {
        return Err("page name is required".to_string());
    }
    let path = app
        .find_page_path_by_name(page_name)
        .ok_or_else(|| format!("unknown page name: {page_name}"))?;
    let target_url = match query {
        Some(query) => crate::append_page_query(path, query)?,
        None => path,
    };

    if kind == NavigationType::Replace && is_tabbar_page_url(app, &target_url) {
        return Err("redirectTo cannot navigate to a tabBar page".to_string());
    }

    app.ensure_page_exists(&target_url)
        .map_err(|err| err.to_string())?;
    let current_path = app
        .peek_current_page()
        .ok_or_else(|| "no current page".to_string())?;
    let current_page = app
        .get_page(&current_path)
        .ok_or_else(|| "current page not found".to_string())?;
    let target_page = app.get_or_create_page(&target_url);
    let target_page = current_page
        .navigate_to(target_page, kind)
        .map_err(|err| err.to_string())?;
    if wait_ready {
        target_page
            .wait_webview_ready()
            .await
            .map_err(|err| err.to_string())?;
    }

    resolve_page(app, None)
}

/// Navigate back `delta` pages and return the landed page (+ configured name).
/// See [`navigate`] for the `wait_ready` contract.
pub async fn navigate_back(
    app: &Arc<LxApp>,
    delta: u32,
    wait_ready: bool,
) -> Result<(PageInstance, Option<String>), String> {
    app.current_page()
        .map_err(|err| err.to_string())?
        .navigate_back(delta)
        .map_err(|err| err.to_string())?;
    let (page, name) = resolve_page(app, None)?;
    if wait_ready {
        page.wait_webview_ready()
            .await
            .map_err(|err| err.to_string())?;
    }
    Ok((page, name))
}

// ===================== page WebView actions =====================

/// Evaluate JavaScript in the target page WebView.
pub async fn page_eval(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    js: &str,
) -> Result<Value, String> {
    resolve_webview(app, page_name)?
        .evaluate_javascript(js)
        .await
        .map_err(|err| err.to_string())
}

/// Query DOM nodes in the target page; returns the query-script payload.
pub async fn page_query(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    all: bool,
    max_text: Option<usize>,
) -> Result<Value, String> {
    let script = build_query_script(selector, index, all, max_text)?;
    page_eval(app, page_name, &script).await
}

/// Click the matching DOM node.
pub async fn page_click(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
) -> Result<(), String> {
    resolve_webview(app, page_name)?
        .click(selector, lingxia_webview::ClickOptions { index })
        .await
        .map_err(|err| err.to_string())
}

/// Type text into the matching editable node without clearing it.
pub async fn page_type(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    text: &str,
) -> Result<(), String> {
    resolve_webview(app, page_name)?
        .type_text(
            selector,
            text,
            lingxia_webview::TypeOptions {
                index,
                replace: false,
            },
        )
        .await
        .map_err(|err| err.to_string())
}

/// Replace the matching editable node's content with `text`.
pub async fn page_fill(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    text: &str,
) -> Result<(), String> {
    resolve_webview(app, page_name)?
        .fill(selector, text, lingxia_webview::FillOptions { index })
        .await
        .map_err(|err| err.to_string())
}

/// Send a key press to the target page WebView.
pub async fn page_press(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    key: &str,
    selector: Option<&str>,
    index: Option<usize>,
) -> Result<(), String> {
    resolve_webview(app, page_name)?
        .press(
            key,
            lingxia_webview::PressOptions {
                selector: selector.map(ToOwned::to_owned),
                index,
            },
        )
        .await
        .map_err(|err| err.to_string())
}

/// Capture a PNG screenshot of the target page's WebView.
pub async fn page_screenshot(app: &Arc<LxApp>, page_name: Option<&str>) -> Result<Vec<u8>, String> {
    resolve_webview(app, page_name)?
        .take_screenshot()
        .await
        .map_err(|err| err.to_string())
}

/// Scroll the target page WebView by a pixel delta.
pub async fn page_scroll(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    dx: f64,
    dy: f64,
) -> Result<(), String> {
    resolve_webview(app, page_name)?
        .scroll(dx, dy, lingxia_webview::ScrollOptions)
        .await
        .map_err(|err| err.to_string())
}

/// Scroll the first element matching `selector` into view.
pub async fn page_scroll_to(
    app: &Arc<LxApp>,
    page_name: Option<&str>,
    selector: &str,
) -> Result<(), String> {
    resolve_webview(app, page_name)?
        .scroll_to(selector, lingxia_webview::ScrollOptions)
        .await
        .map_err(|err| err.to_string())
}

// ===================== DOM query script =====================

/// Build the DOM query IIFE shared by every automation front-end.
///
/// Single-node mode returns the `describe` payload below (`{ exists, index,
/// count, tag, visible, enabled, editable, text, value, rect, … }`); `all`
/// mode returns `{ count, items: [...] }`. `visible` is viewport-aware.
pub fn build_query_script(
    selector: &str,
    index: Option<usize>,
    all: bool,
    max_text_chars: Option<usize>,
) -> Result<String, String> {
    let selector_json =
        serde_json::to_string(selector).map_err(|err| format!("invalid selector: {err}"))?;
    let index_json =
        serde_json::to_string(&index).map_err(|err| format!("invalid index: {err}"))?;
    let max_text_json = serde_json::to_string(&max_text_chars)
        .map_err(|err| format!("invalid query limit: {err}"))?;
    Ok(format!(
        r#"
(() => {{
  const selector = {selector_json};
  const requestedIndex = {index_json};
  const all = {all};
  const maxText = {max_text_json};
  const truncate = (value) => {{
    const text = String(value ?? "");
    if (typeof maxText === "number" && maxText >= 0 && text.length > maxText) {{
      return {{ value: text.slice(0, maxText), truncated: true }};
    }}
    return {{ value: text, truncated: false }};
  }};
  if (typeof selector !== "string" || selector.trim() === "") {{
    throw new Error("selector must not be empty");
  }}
  let nodes;
  try {{
    nodes = Array.from(document.querySelectorAll(selector));
  }} catch (err) {{
    throw new Error("invalid selector: " + String(err && err.message ? err.message : err));
  }}
  const describe = (el, index, count) => {{
    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const disabled = !!el.disabled || el.getAttribute("aria-disabled") === "true";
    const tag = (el.tagName || "").toLowerCase();
    const inputType = tag === "input" ? String(el.type || "text").toLowerCase() : "";
    const blockedInputTypes = new Set([
      "button", "checkbox", "color", "file", "hidden", "image", "radio",
      "range", "reset", "submit"
    ]);
    const editable = !!el.isContentEditable ||
      (tag === "textarea" && !disabled && !el.readOnly) ||
      (tag === "input" && !disabled && !el.readOnly && !blockedInputTypes.has(inputType));
    const visible = rect.width > 0 &&
      rect.height > 0 &&
      rect.bottom > 0 &&
      rect.right > 0 &&
      rect.top < window.innerHeight &&
      rect.left < window.innerWidth &&
      style.visibility !== "hidden" &&
      style.display !== "none" &&
      Number(style.opacity || "1") !== 0;
    const hasValue = "value" in el;
    const text = truncate(el.innerText || el.textContent || "");
    const value = hasValue ? truncate(el.value ?? "") : null;
    return {{
      exists: true,
      index,
      count,
      tag,
      type: inputType || null,
      id: el.id || null,
      name: el.getAttribute("name"),
      role: el.getAttribute("role"),
      aria_label: el.getAttribute("aria-label"),
      placeholder: el.getAttribute("placeholder"),
      visible,
      enabled: !disabled,
      editable,
      text: text.value,
      text_truncated: text.truncated,
      value: value ? value.value : null,
      value_truncated: value ? value.truncated : false,
      rect: {{
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        right: rect.right,
        bottom: rect.bottom,
        center_x: rect.left + (rect.width / 2),
        center_y: rect.top + (rect.height / 2),
        viewport_width: window.innerWidth,
        viewport_height: window.innerHeight
      }}
    }};
  }};
  const count = nodes.length;
  if (all) {{
    return {{ count, items: nodes.map((el, index) => describe(el, index, count)) }};
  }}
  const index = typeof requestedIndex === "number" ? requestedIndex : 0;
  const el = nodes[index];
  if (!el) {{
    return {{
      exists: false,
      index,
      count,
      visible: false,
      enabled: false,
      editable: false
    }};
  }}
  return describe(el, index, count);
}})()
"#
    ))
}
