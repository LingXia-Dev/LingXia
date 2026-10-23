//! `browser.*` commands built on the page agent: snapshot, ref-or-selector
//! actions with auto-wait, form input, text, screenshots and console. Their
//! failures are typed: the error `code` is the failure kind and `data`
//! carries what the caller needs to recover.

use crate::util::png_response;
use lingxia_browser::BrowserAutomationError;
use lingxia_browser::agent::{
    self, ActionOptions, AfterAction, ClickOptions, ElementTarget, OptionChoice, ScreenshotScope,
    SnapshotOptions, TextOptions,
};
use lingxia_control_protocol::{ControlError, methods};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn is_agent_handler(handler: &str) -> bool {
    matches!(
        handler,
        methods::browser::SNAPSHOT
            | methods::browser::CLICK
            | methods::browser::HOVER
            | methods::browser::DRAG
            | methods::browser::TYPE
            | methods::browser::FILL
            | methods::browser::PRESS
            | methods::browser::SCROLL_TO
            | methods::browser::SELECT_OPTION
            | methods::browser::CHECK
            | methods::browser::UNCHECK
            | methods::browser::SET_INPUT_FILES
            | methods::browser::TEXT
            | methods::browser::SCREENSHOT
            | methods::browser::CONSOLE_LIST
            | methods::browser::CONSOLE_CLEAR
    )
}

fn usage(message: impl Into<String>) -> ControlError {
    ControlError {
        code: "usage".to_string(),
        message: message.into(),
        data: Some(json!({ "kind": "usage" })),
    }
}

pub(crate) fn control_error(error: BrowserAutomationError) -> ControlError {
    match error {
        BrowserAutomationError::Action(error) => {
            let mut data = error.details.clone();
            data.insert("kind".to_string(), json!(error.kind));
            let message = match data.get("covered_by") {
                Some(cover) => format!(
                    "{} ({}{})",
                    error.message,
                    cover
                        .get("selector")
                        .and_then(Value::as_str)
                        .unwrap_or("element"),
                    cover
                        .get("ref")
                        .and_then(Value::as_str)
                        .map(|r| format!(", ref {r}"))
                        .unwrap_or_default(),
                ),
                None => error.message,
            };
            ControlError {
                code: error.kind,
                message,
                data: Some(Value::Object(data)),
            }
        }
        BrowserAutomationError::TabNotFound(_) | BrowserAutomationError::WebViewNotFound(_) => {
            ControlError {
                code: "not_found".to_string(),
                message: error.to_string(),
                data: Some(json!({ "kind": "tab_not_found" })),
            }
        }
        BrowserAutomationError::WaitTimeout { .. } => ControlError {
            code: "timeout".to_string(),
            message: error.to_string(),
            data: Some(json!({ "kind": "timeout" })),
        },
        BrowserAutomationError::Input(lingxia_browser::WebViewInputError::Unsupported(what)) => {
            ControlError {
                code: "unsupported".to_string(),
                message: format!("{what} is not supported in this WebView"),
                data: Some(json!({ "kind": "unsupported" })),
            }
        }
        BrowserAutomationError::WebView(
            unsupported @ lingxia_browser::WebViewError::Unsupported(_),
        ) => ControlError {
            code: "unsupported".to_string(),
            message: unsupported.to_string(),
            data: Some(json!({ "kind": "unsupported" })),
        },
        other => ControlError {
            code: "request_failed".to_string(),
            message: other.to_string(),
            data: None,
        },
    }
}

/// Like `util::run_async`, but keeps the typed error.
fn block<T>(
    future: impl std::future::Future<Output = Result<T, ControlError>>,
) -> Result<T, ControlError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| ControlError {
            code: "request_failed".to_string(),
            message: err.to_string(),
            data: None,
        })?
        .block_on(future)
}

fn parse<T: for<'de> Deserialize<'de>>(
    handler: &str,
    args: Option<Value>,
) -> Result<T, ControlError> {
    serde_json::from_value(args.unwrap_or_else(|| json!({})))
        .map_err(|err| usage(format!("invalid args for {handler}: {err}")))
}

fn tab(raw: &str) -> Result<String, ControlError> {
    crate::browser::resolve_tab_id(raw).map_err(|message| ControlError {
        code: "not_found".to_string(),
        message,
        data: Some(json!({ "kind": "tab_not_found" })),
    })
}

/// `ref` or `selector` (+ `index`), flattened into each command's args.
#[derive(Deserialize, Default)]
struct TargetFields {
    #[serde(rename = "ref", default)]
    reference: Option<String>,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    index: Option<usize>,
}

impl TargetFields {
    fn optional(self) -> Result<Option<ElementTarget>, ControlError> {
        match (self.reference, self.selector) {
            (None, None) => {
                if self.index.is_some() {
                    return Err(usage("index needs a selector"));
                }
                Ok(None)
            }
            (Some(_), Some(_)) => Err(usage("pass exactly one of ref or selector")),
            (Some(reference), None) => {
                if self.index.is_some() {
                    return Err(usage("index applies to a selector, not a ref"));
                }
                Ok(Some(ElementTarget {
                    reference: Some(reference),
                    selector: None,
                    index: None,
                }))
            }
            (None, Some(selector)) => Ok(Some(ElementTarget {
                reference: None,
                selector: Some(selector),
                index: self.index,
            })),
        }
    }

    fn required(self) -> Result<ElementTarget, ControlError> {
        self.optional()?
            .ok_or_else(|| usage("pass ref (from browser.snapshot) or selector"))
    }
}

#[derive(Deserialize)]
struct ActionArgs {
    tab_id: String,
    #[serde(flatten)]
    target: TargetFields,
    #[serde(default)]
    timeout_ms: Option<u64>,
    /// `diff` or `full`: include a snapshot of the page after the action.
    #[serde(default)]
    snapshot: AfterAction,
    /// Wait for the action to navigate the tab.
    #[serde(default)]
    wait_navigation: bool,
    #[serde(default)]
    complete: bool,
    // click
    #[serde(default)]
    button: lingxia_browser::PointerButton,
    #[serde(default)]
    click_count: Option<u32>,
    // type / fill
    #[serde(default, alias = "value")]
    text: Option<String>,
    #[serde(default)]
    clear: bool,
    // press
    #[serde(default)]
    key: Option<String>,
    // drag
    #[serde(default)]
    to: Option<ElementTarget>,
    // select_option
    #[serde(default)]
    options: Vec<OptionChoice>,
    // set_input_files
    #[serde(default)]
    files: Vec<String>,
}

impl ActionArgs {
    fn options(&self) -> ActionOptions {
        ActionOptions {
            timeout: self
                .timeout_ms
                .map(Duration::from_millis)
                .unwrap_or(DEFAULT_TIMEOUT),
            after: self.snapshot,
        }
    }
}

fn action_response(action: &str, tab_id: &str, result: Value, navigation: Option<Value>) -> Value {
    let mut object = match result {
        Value::Object(object) => object,
        _ => serde_json::Map::new(),
    };
    object.insert("ok".to_string(), json!(true));
    object.insert("action".to_string(), json!(action));
    object.insert("tab_id".to_string(), json!(tab_id));
    if let Some(Value::Object(navigation)) = navigation {
        for (key, value) in navigation {
            object.entry(key).or_insert(value);
        }
    }
    Value::Object(object)
}

async fn current_url(tab_id: &str, wanted: bool) -> Result<Option<Option<String>>, ControlError> {
    if !wanted {
        return Ok(None);
    }
    lingxia_browser::current_url(tab_id)
        .await
        .map(Some)
        .map_err(control_error)
}

async fn wait_navigation(
    tab_id: &str,
    initial: Option<Option<String>>,
    complete: bool,
    timeout: Duration,
) -> Result<Option<Value>, ControlError> {
    let Some(initial_url) = initial else {
        return Ok(None);
    };
    let result = lingxia_browser::wait(
        tab_id,
        lingxia_browser::BrowserWaitCondition::Navigation {
            initial_url,
            wait_until_complete: complete,
        },
        timeout,
    )
    .await
    .map_err(control_error)?;
    Ok(serde_json::to_value(result).ok())
}

pub(crate) fn handle(handler: &str, args: Option<Value>) -> Result<Option<Value>, ControlError> {
    match handler {
        methods::browser::SNAPSHOT => {
            #[derive(Deserialize)]
            struct Args {
                tab_id: String,
                #[serde(flatten)]
                root: TargetFields,
                #[serde(default)]
                format: Option<String>,
                #[serde(default)]
                max_nodes: Option<u32>,
                #[serde(default)]
                max_chars: Option<u32>,
                #[serde(default)]
                since: Option<String>,
                #[serde(default)]
                full: bool,
            }
            let args: Args = parse(handler, args)?;
            let tab_id = tab(&args.tab_id)?;
            let options = SnapshotOptions {
                root: args.root.optional()?,
                format: args.format,
                max_nodes: args.max_nodes,
                max_chars: args.max_chars,
                since: args.since,
                full: args.full,
            };
            let mut value = block(async {
                agent::snapshot(&tab_id, &options)
                    .await
                    .map_err(control_error)
            })?;
            if let Some(object) = value.as_object_mut() {
                object.insert("tab_id".to_string(), json!(tab_id));
            }
            Ok(Some(value))
        }
        methods::browser::TEXT => {
            #[derive(Deserialize)]
            struct Args {
                tab_id: String,
                #[serde(flatten)]
                target: TargetFields,
                #[serde(default)]
                max_chars: Option<u32>,
                #[serde(default)]
                all: bool,
            }
            let args: Args = parse(handler, args)?;
            let tab_id = tab(&args.tab_id)?;
            let options = TextOptions {
                target: args.target.optional()?,
                max_chars: args.max_chars,
                all: args.all,
            };
            let mut value =
                block(async { agent::text(&tab_id, &options).await.map_err(control_error) })?;
            if let Some(object) = value.as_object_mut() {
                object.insert("tab_id".to_string(), json!(tab_id));
            }
            Ok(Some(value))
        }
        methods::browser::SCREENSHOT => {
            #[derive(Deserialize)]
            struct Args {
                tab_id: String,
                #[serde(flatten)]
                target: TargetFields,
                #[serde(default)]
                full_page: bool,
                #[serde(default)]
                annotate: bool,
                #[serde(default)]
                timeout_ms: Option<u64>,
            }
            let args: Args = parse(handler, args)?;
            let tab_id = tab(&args.tab_id)?;
            let scope = match (args.target.optional()?, args.full_page) {
                (Some(_), true) => return Err(usage("pass either an element or full_page")),
                (Some(target), false) => ScreenshotScope::Element(target),
                (None, true) => ScreenshotScope::FullPage,
                (None, false) => ScreenshotScope::Viewport,
            };
            let timeout = args
                .timeout_ms
                .map(Duration::from_millis)
                .unwrap_or(DEFAULT_TIMEOUT);
            let shot = block(async {
                agent::screenshot(&tab_id, &scope, args.annotate, timeout)
                    .await
                    .map_err(control_error)
            })?;
            let mut extra: Vec<(&'static str, Value)> = vec![("tab_id", json!(tab_id))];
            for (key, value) in shot.meta {
                let key: &'static str = match key.as_str() {
                    "scope" => "scope",
                    "marks" => "marks",
                    "truncated" => "marks_truncated",
                    "viewport" => "viewport",
                    "page" => "page",
                    "ref" => "ref",
                    "rect" => "rect",
                    "clipped" => "clipped",
                    _ => continue,
                };
                extra.push((key, value));
            }
            Ok(Some(png_response(
                "browser",
                "css_pixels",
                &shot.png,
                extra,
            )))
        }
        methods::browser::CONSOLE_LIST => {
            #[derive(Deserialize)]
            struct Args {
                tab_id: String,
                #[serde(default)]
                since: Option<u64>,
                #[serde(default)]
                levels: Vec<String>,
            }
            let args: Args = parse(handler, args)?;
            let tab_id = tab(&args.tab_id)?;
            let list =
                block(async { Ok(agent::console_list(&tab_id, args.since, &args.levels).await) })?;
            let mut value = serde_json::to_value(list).unwrap_or_default();
            if let Some(object) = value.as_object_mut() {
                object.insert("tab_id".to_string(), json!(tab_id));
            }
            Ok(Some(value))
        }
        methods::browser::CONSOLE_CLEAR => {
            #[derive(Deserialize)]
            struct Args {
                tab_id: String,
            }
            let args: Args = parse(handler, args)?;
            let tab_id = tab(&args.tab_id)?;
            agent::console_clear(&tab_id);
            Ok(Some(action_response(
                "console_clear",
                &tab_id,
                json!({}),
                None,
            )))
        }
        _ => handle_action(handler, args),
    }
}

fn handle_action(handler: &str, args: Option<Value>) -> Result<Option<Value>, ControlError> {
    let mut args: ActionArgs = parse(handler, args)?;
    let tab_id = tab(&args.tab_id)?;
    // The page change is read after any navigation the action waits for.
    let after = args.snapshot;
    let options = ActionOptions {
        after: AfterAction::None,
        ..args.options()
    };
    let target_fields = std::mem::take(&mut args.target);
    let action = handler
        .strip_prefix("browser.")
        .unwrap_or(handler)
        .to_string();
    let wants_navigation = args.wait_navigation;
    let complete = args.complete;
    block(async move {
        let initial = current_url(&tab_id, wants_navigation).await?;
        let result = match handler {
            methods::browser::CLICK => {
                let target = target_fields.required()?;
                let click = ClickOptions {
                    button: args.button,
                    click_count: args.click_count.unwrap_or(1),
                };
                agent::click(&tab_id, &target, click, &options).await
            }
            methods::browser::HOVER => {
                agent::hover(&tab_id, &target_fields.required()?, &options).await
            }
            methods::browser::DRAG => {
                let source = target_fields.required()?;
                let to = args
                    .to
                    .take()
                    .ok_or_else(|| usage("drag needs to: { ref } or { selector }"))?;
                agent::drag(&tab_id, &source, &to, &options).await
            }
            methods::browser::FILL => {
                let text = args.text.take().ok_or_else(|| usage("fill needs text"))?;
                agent::fill(&tab_id, &target_fields.required()?, &text, &options).await
            }
            methods::browser::TYPE => {
                let text = args.text.take().ok_or_else(|| usage("type needs text"))?;
                agent::type_text(
                    &tab_id,
                    &target_fields.required()?,
                    &text,
                    args.clear,
                    &options,
                )
                .await
            }
            methods::browser::PRESS => {
                let key = args.key.take().ok_or_else(|| usage("press needs key"))?;
                let target = target_fields.optional()?;
                agent::press(&tab_id, target.as_ref(), &key, &options).await
            }
            methods::browser::SCROLL_TO => {
                agent::scroll_to(&tab_id, &target_fields.required()?, &options).await
            }
            methods::browser::SELECT_OPTION => {
                agent::select_option(&tab_id, &target_fields.required()?, &args.options, &options)
                    .await
            }
            methods::browser::CHECK | methods::browser::UNCHECK => {
                let checked = handler == methods::browser::CHECK;
                agent::set_checked(&tab_id, &target_fields.required()?, checked, &options).await
            }
            methods::browser::SET_INPUT_FILES => {
                agent::set_input_files(&tab_id, &target_fields.required()?, &args.files, &options)
                    .await
            }
            other => return Err(usage(format!("unknown browser handler: {other}"))),
        }
        .map_err(control_error)?;
        let navigation = wait_navigation(&tab_id, initial, complete, options.timeout).await?;
        let result = agent::with_page_change(&tab_id, result, after)
            .await
            .map_err(control_error)?;
        Ok(Some(action_response(&action, &tab_id, result, navigation)))
    })
}
