use crate::util::run_async;
use lingxia_devtool_protocol::handlers;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_EVAL_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn handle_lxapp_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("lxapp.") {
        return None;
    }

    Some(handle_lxapp_command_impl(handler, args))
}

/// Report the selected session's automation capabilities. Runtime-known facts
/// (platform, page-input support) are resolved here; the CLI adds session id.
fn build_doctor() -> Value {
    let page_input = lingxia::dev::lxapp_dev_page_input_supported();
    let is_desktop = cfg!(any(target_os = "windows", target_os = "macos"));
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "android") {
        "android"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(all(target_os = "linux", target_env = "ohos")) {
        "harmony"
    } else {
        "unknown"
    };
    let backend = if is_desktop {
        "runner"
    } else {
        "lingxia_mobile"
    };
    let page_input = if page_input {
        let tier = if is_desktop {
            "native"
        } else if cfg!(target_os = "android") {
            "hybrid"
        } else {
            "js"
        };
        json!({ "supported": true, "tier": tier, "coordinate_space": "css_pixels" })
    } else {
        json!({ "supported": false, "reason": "no first-class page input bridge" })
    };
    json!({
        "target": "lxapp",
        "platform": platform,
        "backend": backend,
        "capabilities": {
            "page_screenshot": { "supported": true },
            "page_input": page_input,
            "runner": { "supported": is_desktop },
        },
        "coordinate_spaces": {
            "page": "css_pixels"
        }
    })
}

fn handle_lxapp_command_impl(handler: &str, args: Option<Value>) -> Result<Option<Value>, String> {
    match handler {
        handlers::lxapp::LIST => {
            let args: ListArgs = parse_args(handler, args)?;
            let (current_appid, _, current_session_id) = lxapp::get_current_lxapp();
            let mut apps: Vec<Value> = lxapp::list_lxapps()
                .into_iter()
                .filter(|app| args.all || app.status == "opened" || app.status == "opening")
                .map(|app| {
                    let current =
                        app.appid == current_appid && app.session_id == current_session_id;
                    json!({
                        "appid": app.appid,
                        "name": app.app_name,
                        "status": app.status,
                        "current": current,
                        "page": app.current_page,
                        "pages_count": app.pages_count,
                    })
                })
                .collect();
            apps.sort_by(|a, b| {
                let a_current = a.get("current").and_then(Value::as_bool).unwrap_or(false);
                let b_current = b.get("current").and_then(Value::as_bool).unwrap_or(false);
                b_current.cmp(&a_current).then_with(|| {
                    a.get("appid")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .cmp(b.get("appid").and_then(Value::as_str).unwrap_or(""))
                })
            });
            Ok(Some(Value::Array(apps)))
        }
        handlers::lxapp::DOCTOR => Ok(Some(build_doctor())),
        handlers::lxapp::CURRENT => {
            let (appid, path, _) = lxapp::get_current_lxapp();
            Ok(Some(json!({
                "appid": appid,
                "path": path,
            })))
        }
        handlers::lxapp::INFO => {
            let args: AppArgs = parse_args(handler, args)?;
            let app = resolve_app(&args.appid)?;
            lxapp_runtime_info_value(&app).map(Some)
        }
        handlers::lxapp::PAGES => {
            let args: AppArgs = parse_args(handler, args)?;
            let app = resolve_app(&args.appid)?;
            let info = app.runtime_info();
            let pages = info
                .page_entries
                .iter()
                .map(|page| {
                    json!({
                        "name": page.name,
                        "path": page.path,
                        "current": info.current_page.as_deref().is_some_and(|current| page_paths_match(current, &page.path)),
                        "in_stack": info.page_stack.iter().any(|stack_page| page_paths_match(stack_page, &page.path)),
                    })
                })
                .collect::<Vec<_>>();
            Ok(Some(json!({
                "appid": info.appid,
                "pages_count": info.pages_count,
                "pages": pages,
            })))
        }
        handlers::lxapp::EVAL => {
            let args: EvalArgs = parse_args(handler, args)?;
            let app = resolve_app(&args.appid)?;
            let timeout = Duration::from_millis(args.timeout_ms.unwrap_or_else(|| {
                u64::try_from(DEFAULT_EVAL_TIMEOUT.as_millis()).unwrap_or(5000)
            }));
            let value = run_async(async move {
                tokio::time::timeout(timeout, app.eval_logic(args.script))
                    .await
                    .map_err(|_| format!("lxapp eval timed out after {}ms", timeout.as_millis()))?
                    .map_err(|err| err.to_string())
            })?;
            Ok(Some(json!({ "value": value })))
        }
        handlers::lxapp::OPEN => {
            let args: OpenArgs = parse_args(handler, args)?;
            let release_type = release_type(args.release_type.as_deref())?;
            ensure_lxapp_available(&args.appid, release_type)?;
            let app = lxapp::open_lxapp(
                &args.appid,
                lxapp::LxAppStartupOptions::new(args.path.as_deref().unwrap_or(""))
                    .set_release_type(release_type),
            )
            .map_err(|err| err.to_string())?;
            let page = run_async(lingxia::dev::lxapp_dev_page_wait(
                Some(&app.appid),
                None,
                None,
                None,
                lingxia::dev::LxAppDevPageWaitState::Ready,
                Duration::from_secs(15),
            ))?
            .page;
            Ok(Some(json!({
                "appid": app.appid,
                "path": page.path,
                "page": page,
            })))
        }
        handlers::lxapp::CLOSE => {
            let args: AppArgs = parse_args(handler, args)?;
            let appid = resolve_appid(&args.appid)?;
            lxapp::close_lxapp(&appid).map_err(|err| err.to_string())?;
            Ok(Some(
                json!({ "ok": true, "action": "close", "appid": appid }),
            ))
        }
        handlers::lxapp::RESTART => {
            let args: AppArgs = parse_args(handler, args)?;
            let appid = resolve_appid(&args.appid)?;
            let page = run_async(lingxia::dev::lxapp_dev_restart(
                &appid,
                Duration::from_secs(15),
            ))?;
            Ok(Some(json!({
                "ok": true,
                "action": "restart",
                "appid": appid,
                "page": page,
            })))
        }
        handlers::lxapp::UNINSTALL => {
            let args: AppArgs = parse_args(handler, args)?;
            let appid = resolve_appid(&args.appid)?;
            lxapp::uninstall_lxapp(&appid).map_err(|err| err.to_string())?;
            Ok(Some(
                json!({ "ok": true, "action": "uninstall", "appid": appid }),
            ))
        }
        _ => Err(format!("unknown lxapp handler: {}", handler)),
    }
}

fn page_paths_match(left: &str, right: &str) -> bool {
    fn key(path: &str) -> &str {
        path.split(['?', '#'])
            .next()
            .unwrap_or(path)
            .trim_start_matches('/')
    }
    key(left) == key(right)
}

fn lxapp_runtime_info_value(app: &Arc<lxapp::LxApp>) -> Result<Value, String> {
    let mut value = serde_json::to_value(app.runtime_info()).map_err(|err| err.to_string())?;
    if let Value::Object(map) = &mut value {
        map.remove("session_id");
        map.remove("current_page");
        map.remove("initial_route");
        map.remove("pages_count");
        map.remove("page_entries");
        map.remove("page_stack");
    }
    Ok(value)
}

fn resolve_app(raw: &str) -> Result<Arc<lxapp::LxApp>, String> {
    let appid = resolve_appid(raw)?;
    if let Some(app) = lxapp::try_get(&appid) {
        return Ok(app);
    }
    ensure_lxapp_available(&appid, lxapp::ReleaseType::Release)
}

fn ensure_lxapp_available(
    appid: &str,
    release_type: lxapp::ReleaseType,
) -> Result<Arc<lxapp::LxApp>, String> {
    if let Some(app) = lxapp::try_get(appid) {
        return Ok(app);
    }
    if lxapp::installed_lxapp_path(appid, release_type).is_some() {
        return lxapp::ensure_lxapp(appid, release_type).map_err(|err| err.to_string());
    }
    lxapp::register_builtin_asset_bundle(appid.to_string());
    lxapp::ensure_builtin_lxapp(appid).map_err(|err| err.to_string())
}

fn resolve_appid(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("current") {
        let (appid, _, _) = lxapp::get_current_lxapp();
        if appid.is_empty() {
            Err("no current lxapp".to_string())
        } else {
            Ok(appid)
        }
    } else if trimmed.is_empty() {
        Err("appid is required".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn release_type(value: Option<&str>) -> Result<lxapp::ReleaseType, String> {
    match value
        .unwrap_or("release")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "release" => Ok(lxapp::ReleaseType::Release),
        "preview" | "trial" => Ok(lxapp::ReleaseType::Preview),
        "developer" | "develop" | "dev" => Ok(lxapp::ReleaseType::Developer),
        other => Err(format!(
            "unsupported release_type {other:?}; expected release, preview, or developer"
        )),
    }
}

fn parse_args<T>(handler: &str, args: Option<Value>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args.unwrap_or_else(|| json!({})))
        .map_err(|err| format!("invalid args for {}: {}", handler, err))
}

#[derive(Deserialize)]
struct ListArgs {
    #[serde(default)]
    all: bool,
}

#[derive(Deserialize)]
struct AppArgs {
    appid: String,
}

#[derive(Deserialize)]
struct EvalArgs {
    appid: String,
    script: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct OpenArgs {
    appid: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    release_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::page_paths_match;

    #[test]
    fn configured_page_status_ignores_query_and_leading_slash() {
        assert!(page_paths_match("/pages/home?tab=1", "pages/home"));
        assert!(page_paths_match("pages/home#section", "/pages/home"));
        assert!(!page_paths_match("pages/home", "pages/profile"));
    }
}
