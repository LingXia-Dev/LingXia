use crate::util::{png_response, run_async};
use lingxia_control_protocol::methods;
use lxapp::automation as auto;
use lxapp::automation::gpu::{GpuAction, GpuPageAutomation};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_EVAL_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_QUERY_TEXT_LIMIT: usize = 4096;

pub(crate) fn handle_lxapp_page_command(
    handler: &str,
    args: Option<Value>,
) -> Option<Result<Option<Value>, String>> {
    if !handler.starts_with("lxapp.page.") {
        return None;
    }

    Some(handle_lxapp_page_command_impl(handler, args))
}

fn handle_lxapp_page_command_impl(
    handler: &str,
    args: Option<Value>,
) -> Result<Option<Value>, String> {
    match handler {
        methods::lxapp_page::CURRENT => {
            let args: PageTargetArgs = parse_args(handler, args)?;
            let info = crate::devtool::lxapp_dev_page_current(args.appid.as_deref())?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        methods::lxapp_page::LIST => {
            let args: AppArgs = parse_args(handler, args)?;
            let pages = crate::devtool::lxapp_dev_page_list(args.appid.as_deref())?;
            Ok(Some(page_list_response(pages)))
        }
        methods::lxapp_page::INFO => {
            let args: PageTargetArgs = parse_args(handler, args)?;
            let info =
                crate::devtool::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            serde_json::to_value(info)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        methods::lxapp_page::WAIT => {
            let args: WaitArgs = parse_args(handler, args)?;
            let state = args.state.unwrap_or(if args.selector.is_some() {
                crate::devtool::LxAppDevPageWaitState::Attached
            } else {
                crate::devtool::LxAppDevPageWaitState::Ready
            });
            let timeout = Duration::from_millis(args.timeout_ms.unwrap_or(5000));
            let result = run_async(crate::devtool::lxapp_dev_page_wait(
                args.appid.as_deref(),
                args.page.as_deref(),
                args.selector.as_deref(),
                args.index,
                state,
                timeout,
            ))?;
            serde_json::to_value(result)
                .map(Some)
                .map_err(|err| err.to_string())
        }
        methods::lxapp_page::EVAL => {
            let args: EvalArgs = parse_args(handler, args)?;
            let timeout = Duration::from_millis(args.timeout_ms.unwrap_or_else(|| {
                u64::try_from(DEFAULT_EVAL_TIMEOUT.as_millis()).unwrap_or(5000)
            }));
            let value = run_async(async move {
                tokio::time::timeout(
                    timeout,
                    crate::devtool::lxapp_dev_page_eval(
                        args.appid.as_deref(),
                        args.page.as_deref(),
                        &args.js,
                    ),
                )
                .await
                .map_err(|_| format!("lxapp page eval timed out after {}ms", timeout.as_millis()))?
            })?;
            Ok(Some(json!({ "value": value })))
        }
        methods::lxapp_page::TREE => {
            let args: PageTargetArgs = parse_args(handler, args)?;
            run_async(crate::devtool::lxapp_dev_page_tree(
                args.appid.as_deref(),
                args.page.as_deref(),
            ))
            .map(Some)
        }
        methods::lxapp_page::FRAME => {
            let args: FrameArgs = parse_args(handler, args)?;
            run_async(crate::devtool::lxapp_dev_page_frame(
                args.appid.as_deref(),
                args.page.as_deref(),
                args.window_ms,
            ))
            .map(Some)
        }
        methods::lxapp_page::HUD => {
            let args: HudArgs = parse_args(handler, args)?;
            run_async(crate::devtool::lxapp_dev_page_hud(
                args.appid.as_deref(),
                args.page.as_deref(),
                args.on,
            ))
            .map(Some)
        }
        methods::lxapp_page::QUERY => {
            let args: QueryArgs = parse_args(handler, args)?;
            let max_text = args
                .max_text
                .or_else(|| (!args.full).then_some(DEFAULT_QUERY_TEXT_LIMIT));
            if args.feedback {
                let app = feedback_app(args.appid.as_deref(), args.page.as_deref())?;
                return run_async(async move {
                    auto::feedback_query(&app, &args.selector, args.index, args.all, max_text).await
                })
                .map(Some);
            }
            run_async(crate::devtool::lxapp_dev_page_query(
                args.appid.as_deref(),
                args.page.as_deref(),
                &args.selector,
                args.index,
                args.all,
                max_text,
            ))
            .map(Some)
        }
        methods::lxapp_page::CLICK => {
            let args: SelectorActionArgs = parse_args(handler, args)?;
            if args.feedback {
                let (appid, layer) = feedback_layer(args.appid.as_deref(), args.page.as_deref())?;
                let click = GpuAction::Click { force: false };
                run_async(layer.act(&args.selector, args.index, click))?;
                return Ok(Some(feedback_action_response("click", appid)));
            }
            let info =
                crate::devtool::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(crate::devtool::lxapp_dev_page_click(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
                args.index,
            ))?;
            Ok(Some(page_action_response("click", info)))
        }
        methods::lxapp_page::TYPE => {
            let args: TextActionArgs = parse_args(handler, args)?;
            if args.feedback {
                let (appid, layer) = feedback_layer(args.appid.as_deref(), args.page.as_deref())?;
                let typing = GpuAction::Type { text: args.text };
                run_async(layer.act(&args.selector, args.index, typing))?;
                return Ok(Some(feedback_action_response("type", appid)));
            }
            let info =
                crate::devtool::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(crate::devtool::lxapp_dev_page_type(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
                args.index,
                &args.text,
            ))?;
            Ok(Some(page_action_response("type", info)))
        }
        methods::lxapp_page::FILL => {
            let args: TextActionArgs = parse_args(handler, args)?;
            let info =
                crate::devtool::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(crate::devtool::lxapp_dev_page_fill(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
                args.index,
                &args.text,
            ))?;
            Ok(Some(page_action_response("fill", info)))
        }
        methods::lxapp_page::PRESS => {
            let args: PressArgs = parse_args(handler, args)?;
            if args.feedback {
                let (appid, layer) = feedback_layer(args.appid.as_deref(), args.page.as_deref())?;
                let press = GpuAction::Press { key: args.key };
                let selector = args.selector.as_deref().unwrap_or("");
                run_async(layer.act(selector, args.index, press))?;
                return Ok(Some(feedback_action_response("press", appid)));
            }
            let info =
                crate::devtool::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(crate::devtool::lxapp_dev_page_press(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.key,
                args.selector.as_deref(),
                args.index,
            ))?;
            Ok(Some(page_action_response("press", info)))
        }
        methods::lxapp_page::SCROLL => {
            let args: ScrollArgs = parse_args(handler, args)?;
            let info =
                crate::devtool::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(crate::devtool::lxapp_dev_page_scroll(
                Some(&info.appid),
                Some(&info.instance_id),
                args.dx,
                args.dy,
            ))?;
            Ok(Some(page_action_response("scroll", info)))
        }
        methods::lxapp_page::SCROLL_TO => {
            let args: SelectorActionArgs = parse_args(handler, args)?;
            let info =
                crate::devtool::lxapp_dev_page_info(args.appid.as_deref(), args.page.as_deref())?;
            run_async(crate::devtool::lxapp_dev_page_scroll_to(
                Some(&info.appid),
                Some(&info.instance_id),
                &args.selector,
            ))?;
            Ok(Some(page_action_response("scroll_to", info)))
        }
        methods::lxapp_page::BACK => {
            let args: BackArgs = parse_args(handler, args)?;
            let info = run_async(crate::devtool::lxapp_dev_nav_back(
                args.appid.as_deref(),
                args.delta.unwrap_or(1),
            ))?;
            Ok(Some(json!({ "ok": true, "action": "back", "page": info })))
        }
        methods::lxapp_page::SCREENSHOT => {
            let parsed: PageTargetArgs = parse_args(handler, args)?;
            if parsed.feedback {
                let (appid, layer) =
                    feedback_layer(parsed.appid.as_deref(), parsed.page.as_deref())?;
                let bytes = run_async(layer.screenshot(true))?;
                return Ok(Some(png_response(
                    "feedback",
                    "css_pixels",
                    &bytes,
                    [("appid", json!(appid))],
                )));
            }
            let (info, bytes) = run_async(crate::devtool::lxapp_dev_page_screenshot_with_info(
                parsed.appid.as_deref(),
                parsed.page.as_deref(),
            ))?;
            Ok(Some(png_response(
                "page",
                "css_pixels",
                &bytes,
                [
                    ("appid", json!(info.appid)),
                    ("page", json!(info.name)),
                    ("path", json!(info.path)),
                    ("instance_id", json!(info.instance_id)),
                ],
            )))
        }
        // Routed here by its `lxapp.page.` prefix.
        methods::lxapp::HOT_SWAP => {
            let args: HotSwapArgs = parse_args(handler, args)?;
            Ok(Some(hot_swap_response(&args)?))
        }
        methods::lxapp::THEME => {
            let args: ThemeArgs = parse_args(handler, args)?;
            let pack = serde_json::to_string(&args.pack).map_err(|error| error.to_string())?;
            let pages = lingxia::dev::apply_dev_theme(args.appid.as_deref(), &pack)?;
            Ok(Some(
                json!({ "ok": true, "action": "theme", "pages": pages }),
            ))
        }
        methods::lxapp::PROBLEMS => {
            let args: ProblemsArgs = parse_args(handler, args)?;
            let pages = lingxia::dev::show_dev_problems(&args.appid, args.report.as_deref());
            Ok(Some(
                json!({ "ok": true, "action": "problems", "pages": pages }),
            ))
        }
        _ => Err(format!("unknown lxapp page handler: {}", handler)),
    }
}

fn page_list_response(pages: Vec<crate::devtool::LxAppDevPageInfo>) -> Value {
    let appid = pages
        .first()
        .map(|page| page.appid.clone())
        .unwrap_or_default();
    json!({
        "appid": appid,
        "pages_count": pages.len(),
        "opened_pages_count": pages.iter().filter(|page| page.opened).count(),
        "pages": pages,
    })
}

/// `--feedback`: the app whose window feedback layer is meant; no page name reaches it.
fn feedback_app(appid: Option<&str>, page: Option<&str>) -> Result<Arc<lxapp::LxApp>, String> {
    if page.is_some() {
        return Err("--feedback targets the feedback layer, not a page: drop --page".into());
    }
    auto::resolve_lxapp(appid.unwrap_or("current"))
}

/// The open feedback layer over the app's window, or `E_PAGE_NOT_READY`.
fn feedback_layer(
    appid: Option<&str>,
    page: Option<&str>,
) -> Result<(String, Arc<dyn GpuPageAutomation>), String> {
    let app = feedback_app(appid, page)?;
    let layer = auto::feedback(&app)?;
    Ok((app.appid.clone(), layer))
}

fn feedback_action_response(action: &'static str, appid: String) -> Value {
    json!({ "ok": true, "action": action, "feedback": { "appid": appid } })
}

fn page_action_response(action: &'static str, page: crate::devtool::LxAppDevPageInfo) -> Value {
    json!({ "ok": true, "action": action, "page": page })
}

fn parse_args<T>(handler: &str, args: Option<Value>) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args.unwrap_or_else(|| json!({})))
        .map_err(|err| format!("invalid args for {}: {}", handler, err))
}

#[derive(Deserialize, Default)]
struct AppArgs {
    #[serde(default)]
    appid: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct FrameArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    window_ms: Option<u64>,
}

#[derive(Deserialize)]
struct HudArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    on: bool,
}

#[derive(Deserialize, Default)]
struct PageTargetArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    feedback: bool,
}

#[derive(Deserialize)]
struct WaitArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default, rename = "selector")]
    selector: Option<String>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    state: Option<crate::devtool::LxAppDevPageWaitState>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct EvalArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    js: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
struct QueryArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    selector: String,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    all: bool,
    #[serde(default)]
    full: bool,
    #[serde(default)]
    max_text: Option<usize>,
    #[serde(default)]
    feedback: bool,
}

#[derive(Deserialize)]
struct SelectorActionArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    selector: String,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    feedback: bool,
}

#[derive(Deserialize)]
struct TextActionArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    selector: String,
    text: String,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    feedback: bool,
}

#[derive(Deserialize)]
struct ScrollArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    #[serde(default)]
    dx: f64,
    #[serde(default)]
    dy: f64,
}

#[derive(Deserialize)]
struct PressArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    page: Option<String>,
    key: String,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    feedback: bool,
}

#[derive(Deserialize)]
struct BackArgs {
    #[serde(default)]
    appid: Option<String>,
    #[serde(default)]
    delta: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::page_list_response;

    #[test]
    fn page_list_contract_contains_only_lxapp_pages() {
        let response = page_list_response(Vec::new());

        assert!(response.get("pages").is_some());
        assert!(response.get("surfaces").is_none());
    }

    #[test]
    fn a_theme_push_names_its_lxapp_or_the_host() {
        let app: super::ThemeArgs =
            serde_json::from_value(serde_json::json!({"appid": "a", "pack": {"name": "x"}}))
                .unwrap();
        assert_eq!(app.appid.as_deref(), Some("a"));
        let host: super::ThemeArgs =
            serde_json::from_value(serde_json::json!({"pack": {"name": "x"}})).unwrap();
        assert!(host.appid.is_none() && host.pack["name"] == "x");
        assert!(serde_json::from_value::<super::ThemeArgs>(serde_json::json!({})).is_err());
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProblemsArgs {
    appid: String,
    /// The type-check's report; `null` once the project is clean.
    report: Option<String>,
}

#[derive(serde::Deserialize)]
struct ThemeArgs {
    /// The lxapp whose `theme.json` was saved; without it, the host's `themes/` file.
    #[serde(default)]
    appid: Option<String>,
    /// The completed, checked pack.
    pack: Value,
}

#[derive(serde::Deserialize)]
struct HotSwapArgs {
    appid: String,
    path: String,
    /// Every page of one edit, `path` first: one bundle sync swaps them all. A host that
    /// predates it swaps `path` alone and answers without `results`.
    #[serde(default)]
    paths: Vec<String>,
}

/// `pages` answers for `path`, as before batches; `results` for each of `paths`.
fn hot_swap_response(args: &HotSwapArgs) -> Result<Value, String> {
    if args.paths.is_empty() {
        let swapped =
            lingxia::dev::hot_swap_gpu_pages(&args.appid, std::slice::from_ref(&args.path))?
                .remove(0)?;
        return Ok(json!({ "ok": true, "action": "hot_swap", "pages": swapped }));
    }
    let outcomes = lingxia::dev::hot_swap_gpu_pages(&args.appid, &args.paths)?;
    let results: Vec<Value> = args
        .paths
        .iter()
        .zip(outcomes)
        .map(|(path, outcome)| match outcome {
            Ok(pages) => json!({ "path": path, "pages": pages }),
            Err(error) => json!({ "path": path, "error": error }),
        })
        .collect();
    let pages: u64 = results.iter().filter_map(|r| r["pages"].as_u64()).sum();
    Ok(json!({ "ok": true, "action": "hot_swap", "pages": pages, "results": results }))
}
