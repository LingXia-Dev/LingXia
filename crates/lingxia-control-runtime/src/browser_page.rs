//! `browser.*` commands built on the page script: snapshot, ref-or-selector
//! actions with auto-wait, form input, text, screenshots and console. Their
//! failures are typed: the error `code` is the failure kind and `data`
//! carries what the caller needs to recover.

use crate::util::png_response;
use lingxia_browser::BrowserAutomationError;
use lingxia_browser::page::{
    self, ActionOptions, AfterAction, ClickOptions, ElementTarget, NavigationWait, OptionChoice,
    PageAction, ScreenshotScope, SnapshotOptions, TextOptions,
};
use lingxia_control_protocol::{ControlError, methods};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

pub(crate) fn is_page_handler(handler: &str) -> bool {
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
    control_error(page::usage(message))
}

pub(crate) fn control_error(error: BrowserAutomationError) -> ControlError {
    let error = page::typed_failure(error);
    let mut data = error.details;
    data.insert("kind".to_string(), json!(error.kind));
    ControlError {
        code: error.kind,
        message: error.message,
        data: Some(Value::Object(data)),
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
        data: Some(json!({ "kind": "not_found", "reason": "tab_not_found" })),
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
        ElementTarget::from_parts(self.reference, self.selector, self.index).map_err(control_error)
    }

    fn required(self) -> Result<ElementTarget, ControlError> {
        ElementTarget::required(self.reference, self.selector, self.index).map_err(control_error)
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

fn action_response(
    action: &str,
    tab_id: &str,
    result: Value,
    navigation: Option<lingxia_browser::BrowserWaitResult>,
) -> Value {
    let mut object = match result {
        Value::Object(object) => object,
        _ => serde_json::Map::new(),
    };
    object.insert("ok".to_string(), json!(true));
    object.insert("action".to_string(), json!(action));
    object.insert("tab_id".to_string(), json!(tab_id));
    if let Some(Value::Object(navigation)) = navigation.and_then(|n| serde_json::to_value(n).ok()) {
        for (key, value) in navigation {
            object.entry(key).or_insert(value);
        }
    }
    Value::Object(object)
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
                page::snapshot(&tab_id, &options)
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
                block(async { page::text(&tab_id, &options).await.map_err(control_error) })?;
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
                .unwrap_or(page::DEFAULT_ACTION_TIMEOUT);
            let shot = block(async {
                page::screenshot(&tab_id, &scope, args.annotate, timeout)
                    .await
                    .map_err(control_error)
            })?;
            let mut extra = vec![("tab_id", json!(tab_id))];
            let mut meta = shot.meta;
            for key in [
                "scope",
                "marks",
                "marks_truncated",
                "viewport",
                "page",
                "ref",
                "rect",
                "clipped",
            ] {
                if let Some(value) = meta.remove(key) {
                    extra.push((key, value));
                }
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
                block(async { Ok(page::console_list(&tab_id, args.since, &args.levels).await) })?;
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
            page::console_clear(&tab_id);
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
    let args: ActionArgs = parse(handler, args)?;
    let tab_id = tab(&args.tab_id)?;
    let options = ActionOptions {
        timeout: args
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(page::DEFAULT_ACTION_TIMEOUT),
        after: args.snapshot,
    };
    let navigation = args.wait_navigation.then_some(NavigationWait {
        complete: args.complete,
    });
    let target = args.target;
    let action = match handler {
        methods::browser::CLICK => PageAction::Click {
            target: target.required()?,
            click: ClickOptions {
                button: args.button,
                click_count: args.click_count.unwrap_or(1),
            },
        },
        methods::browser::HOVER => PageAction::Hover(target.required()?),
        methods::browser::DRAG => PageAction::Drag {
            source: target.required()?,
            to: args
                .to
                .ok_or_else(|| usage("drag needs to: { ref } or { selector }"))?,
        },
        methods::browser::FILL => PageAction::Fill {
            target: target.required()?,
            text: args.text.ok_or_else(|| usage("fill needs text"))?,
        },
        methods::browser::TYPE => PageAction::Type {
            target: target.required()?,
            text: args.text.ok_or_else(|| usage("type needs text"))?,
            clear: args.clear,
        },
        methods::browser::PRESS => PageAction::Press {
            target: target.optional()?,
            key: args.key.ok_or_else(|| usage("press needs key"))?,
        },
        methods::browser::SCROLL_TO => PageAction::ScrollTo(target.required()?),
        methods::browser::SELECT_OPTION => PageAction::SelectOption {
            target: target.required()?,
            options: args.options,
        },
        methods::browser::CHECK | methods::browser::UNCHECK => PageAction::SetChecked {
            target: target.required()?,
            checked: handler == methods::browser::CHECK,
        },
        methods::browser::SET_INPUT_FILES => PageAction::SetInputFiles {
            target: target.required()?,
            files: args.files,
        },
        other => return Err(usage(format!("unknown browser handler: {other}"))),
    };
    let name = handler.strip_prefix("browser.").unwrap_or(handler);
    let outcome = block(async {
        page::perform(&tab_id, &action, &options, navigation)
            .await
            .map_err(control_error)
    })?;
    Ok(Some(action_response(
        name,
        &tab_id,
        outcome.result,
        outcome.navigation,
    )))
}
