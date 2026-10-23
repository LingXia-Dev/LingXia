//! `lx.automation().browser` page primitives over `lingxia_browser::page`,
//! the same code `lxdev browser` runs: options as JS passes them, results
//! with camelCase keys, and failures coded by their kind.

use crate::error::{coded, with_json_data};
use crate::resolve::{js_object_to_json, json_to_js};
use lingxia_browser::page::{
    self, ActionOptions, AfterAction, ClickOptions, ElementTarget, NavigationWait, OptionChoice,
    PageAction, kinds,
};
use lingxia_browser::{BrowserAutomationError, PointerButton};
use rong::{JSContext, JSObject, JSResult, JSValue, RongJSError};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::time::Duration;

const MAX_TIMEOUT: Duration = Duration::from_secs(60);

/// A failure kind as a `&'static` error code.
fn kind_code(kind: &str) -> &'static str {
    [
        kinds::NOT_FOUND,
        kinds::STALE_REF,
        kinds::NOT_VISIBLE,
        kinds::DISABLED,
        kinds::COVERED,
        kinds::NOT_EDITABLE,
        kinds::TIMEOUT,
        kinds::UNSUPPORTED,
        kinds::USAGE,
        kinds::PERMISSION,
    ]
    .into_iter()
    .find(|known| *known == kind)
    .unwrap_or(kinds::FAILED)
}

/// Throw a browser failure: `code` is its kind, `data` its details.
pub(crate) fn browser_err(error: BrowserAutomationError) -> RongJSError {
    let failure = page::typed_failure(error);
    let data = camel_keys(Value::Object(failure.details));
    with_json_data(coded(kind_code(&failure.kind), failure.message), &data).into()
}

fn usage(message: impl Into<String>) -> RongJSError {
    browser_err(page::usage(message))
}

fn camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for ch in key.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// The page payloads use the wire's snake_case keys; JS sees camelCase.
/// Page payload keys are fixed names, never page data.
pub(crate) fn camel_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| (camel(&key), camel_keys(value)))
                .collect::<Map<_, _>>(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(camel_keys).collect()),
        other => other,
    }
}

fn parse<T: DeserializeOwned + Default>(method: &str, options: Option<JSObject>) -> JSResult<T> {
    match options {
        None => Ok(T::default()),
        Some(object) => serde_json::from_value(js_object_to_json(&object)?)
            .map_err(|err| usage(format!("browser.{method}: invalid options: {err}"))),
    }
}

/// Resolve `"current"` (or omitted) to the current tab's id.
pub(crate) fn resolve_tab_id(tab: &Option<String>) -> JSResult<String> {
    let raw = tab.as_deref().unwrap_or("current").trim();
    if !raw.eq_ignore_ascii_case("current") {
        return Ok(raw.to_string());
    }
    lingxia_browser::automation_current_tab()
        .map(|tab| tab.tab_id)
        .ok_or_else(|| {
            with_json_data(
                coded(kinds::NOT_FOUND, "no current browser tab"),
                &json!({ "reason": "tab_not_found" }),
            )
            .into()
        })
}

fn timeout(timeout_ms: Option<u64>) -> Duration {
    timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(page::DEFAULT_ACTION_TIMEOUT)
        .min(MAX_TIMEOUT)
}

/// `{ ref }` or `{ css, index? }`.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct TargetOpt {
    #[serde(rename = "ref")]
    reference: Option<String>,
    css: Option<String>,
    index: Option<usize>,
}

impl TargetOpt {
    fn optional(self) -> JSResult<Option<ElementTarget>> {
        ElementTarget::from_parts(self.reference, self.css, self.index).map_err(browser_err)
    }

    fn required(self) -> JSResult<ElementTarget> {
        ElementTarget::required(self.reference, self.css, self.index).map_err(browser_err)
    }
}

/// Every action's options; each action reads the fields it takes.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct ActionOpt {
    tab: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    css: Option<String>,
    index: Option<usize>,
    timeout_ms: Option<u64>,
    snapshot: AfterAction,
    wait_navigation: bool,
    complete: bool,
    button: PointerButton,
    click_count: Option<u32>,
    text: Option<String>,
    clear: bool,
    key: Option<String>,
    to: Option<TargetOpt>,
    options: Option<Vec<OptionChoice>>,
    files: Option<Vec<String>>,
}

impl ActionOpt {
    fn target(&mut self) -> TargetOpt {
        TargetOpt {
            reference: self.reference.take(),
            css: self.css.take(),
            index: self.index.take(),
        }
    }

    fn action(&mut self, method: &str) -> JSResult<PageAction> {
        let target = self.target();
        Ok(match method {
            "click" => PageAction::Click {
                target: target.required()?,
                click: ClickOptions {
                    button: self.button,
                    click_count: self.click_count.unwrap_or(1),
                },
            },
            "hover" => PageAction::Hover(target.required()?),
            "drag" => PageAction::Drag {
                source: target.required()?,
                to: self
                    .to
                    .take()
                    .ok_or_else(|| usage("drag needs to: { ref } or { css }"))?
                    .required()?,
            },
            "fill" => PageAction::Fill {
                target: target.required()?,
                text: self.text.take().ok_or_else(|| usage("fill needs text"))?,
            },
            "type" => PageAction::Type {
                target: target.required()?,
                text: self.text.take().ok_or_else(|| usage("type needs text"))?,
                clear: self.clear,
            },
            "press" => PageAction::Press {
                target: target.optional()?,
                key: self.key.take().ok_or_else(|| usage("press needs key"))?,
            },
            "scrollTo" => PageAction::ScrollTo(target.required()?),
            "selectOption" => PageAction::SelectOption {
                target: target.required()?,
                options: self
                    .options
                    .take()
                    .ok_or_else(|| usage("selectOption needs options"))?,
            },
            "check" | "uncheck" => PageAction::SetChecked {
                target: target.required()?,
                checked: method == "check",
            },
            "setInputFiles" => PageAction::SetInputFiles {
                target: target.required()?,
                files: self
                    .files
                    .take()
                    .ok_or_else(|| usage("setInputFiles needs files"))?,
            },
            other => return Err(usage(format!("unknown browser action: {other}"))),
        })
    }
}

/// Run one action with auto-wait; resolves to its camelCase result, with
/// `navigation` when it waited for one.
pub(crate) async fn act(
    ctx: &JSContext,
    method: &str,
    options: Option<JSObject>,
) -> JSResult<JSValue> {
    let mut opt: ActionOpt = parse(method, options)?;
    let tab = resolve_tab_id(&opt.tab)?;
    let action = opt.action(method)?;
    let options = ActionOptions {
        timeout: timeout(opt.timeout_ms),
        after: opt.snapshot,
    };
    let navigation = opt.wait_navigation.then_some(NavigationWait {
        complete: opt.complete,
    });
    let outcome = page::perform(&tab, &action, &options, navigation)
        .await
        .map_err(browser_err)?;
    let mut result = camel_keys(outcome.result);
    if let (Some(navigation), Some(object)) = (outcome.navigation, result.as_object_mut()) {
        object.insert(
            "navigation".to_string(),
            serde_json::to_value(navigation).unwrap_or_default(),
        );
    }
    json_to_js(ctx, &result)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct SnapshotOpt {
    tab: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    css: Option<String>,
    index: Option<usize>,
    format: Option<String>,
    max_nodes: Option<u32>,
    max_chars: Option<u32>,
    since_last: bool,
}

pub(crate) async fn snapshot(ctx: &JSContext, options: Option<JSObject>) -> JSResult<JSValue> {
    let opt: SnapshotOpt = parse("snapshot", options)?;
    let tab = resolve_tab_id(&opt.tab)?;
    let options = page::SnapshotOptions {
        root: ElementTarget::from_parts(opt.reference, opt.css, opt.index).map_err(browser_err)?,
        format: opt.format,
        max_nodes: opt.max_nodes,
        max_chars: opt.max_chars,
        since: opt.since_last.then(|| "last".to_string()),
        full: false,
    };
    let value = page::snapshot(&tab, &options).await.map_err(browser_err)?;
    json_to_js(ctx, &camel_keys(value))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct TextOpt {
    tab: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    css: Option<String>,
    index: Option<usize>,
    max_chars: Option<u32>,
    all: bool,
}

pub(crate) async fn text(ctx: &JSContext, options: Option<JSObject>) -> JSResult<JSValue> {
    let opt: TextOpt = parse("text", options)?;
    let tab = resolve_tab_id(&opt.tab)?;
    let options = page::TextOptions {
        target: ElementTarget::from_parts(opt.reference, opt.css, opt.index)
            .map_err(browser_err)?,
        max_chars: opt.max_chars,
        all: opt.all,
    };
    let value = page::text(&tab, &options).await.map_err(browser_err)?;
    json_to_js(ctx, &camel_keys(value))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct ScreenshotOpt {
    tab: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    css: Option<String>,
    index: Option<usize>,
    full_page: bool,
    annotate: bool,
    timeout_ms: Option<u64>,
}

/// A PNG of the viewport, the full page, or one element, as base64 with its
/// scope and (annotated) marks.
pub(crate) async fn screenshot(ctx: &JSContext, options: Option<JSObject>) -> JSResult<JSValue> {
    let opt: ScreenshotOpt = parse("screenshot", options)?;
    let tab = resolve_tab_id(&opt.tab)?;
    let target =
        ElementTarget::from_parts(opt.reference, opt.css, opt.index).map_err(browser_err)?;
    let scope = match (target, opt.full_page) {
        (Some(_), true) => return Err(usage("pass either an element or fullPage")),
        (Some(target), false) => page::ScreenshotScope::Element(target),
        (None, true) => page::ScreenshotScope::FullPage,
        (None, false) => page::ScreenshotScope::Viewport,
    };
    let shot = page::screenshot(&tab, &scope, opt.annotate, timeout(opt.timeout_ms))
        .await
        .map_err(browser_err)?;
    let (width, height) = crate::page::png_dimensions(&shot.png).unwrap_or((0, 0));
    let mut result = match camel_keys(Value::Object(shot.meta)) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    use base64::Engine as _;
    result.insert("format".to_string(), json!("png"));
    result.insert(
        "base64".to_string(),
        json!(base64::engine::general_purpose::STANDARD.encode(&shot.png)),
    );
    result.insert("width".to_string(), json!(width));
    result.insert("height".to_string(), json!(height));
    json_to_js(ctx, &Value::Object(result))
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct ConsoleListOpt {
    tab: Option<String>,
    since: Option<u64>,
    levels: Vec<String>,
}

pub(crate) async fn console_list(ctx: &JSContext, options: Option<JSObject>) -> JSResult<JSValue> {
    let opt: ConsoleListOpt = parse("console.list", options)?;
    let tab = resolve_tab_id(&opt.tab)?;
    let list = page::console_list(&tab, opt.since, &opt.levels).await;
    json_to_js(
        ctx,
        &camel_keys(serde_json::to_value(list).unwrap_or_default()),
    )
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct TabOnlyOpt {
    tab: Option<String>,
}

pub(crate) fn console_clear(options: Option<JSObject>) -> JSResult<()> {
    let opt: TabOnlyOpt = parse("console.clear", options)?;
    page::console_clear(&resolve_tab_id(&opt.tab)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_payload_keys_become_camel_case() {
        let value = camel_keys(json!({
            "ref_count": 3,
            "covered_by": { "ref": "e2", "selector": "div.modal" },
            "nodes": [{ "role": "button", "optionsTruncated": true }],
            "viewport": { "device_pixel_ratio": 2 },
        }));
        assert_eq!(
            value,
            json!({
                "refCount": 3,
                "coveredBy": { "ref": "e2", "selector": "div.modal" },
                "nodes": [{ "role": "button", "optionsTruncated": true }],
                "viewport": { "devicePixelRatio": 2 },
            })
        );
    }

    #[test]
    fn unknown_kinds_code_as_failed() {
        assert_eq!(kind_code("covered"), "covered");
        assert_eq!(kind_code("script"), "failed");
    }

    #[test]
    fn action_options_take_css_targets_and_reject_unknown_fields() {
        let mut opt: ActionOpt =
            serde_json::from_value(json!({ "css": "#go", "index": 1, "snapshot": "diff" }))
                .unwrap();
        assert_eq!(opt.snapshot, AfterAction::Diff);
        let PageAction::Hover(target) = opt.action("hover").unwrap() else {
            panic!("expected hover");
        };
        assert_eq!(target.selector.as_deref(), Some("#go"));
        assert_eq!(target.index, Some(1));
        assert!(serde_json::from_value::<ActionOpt>(json!({ "selector": "#go" })).is_err());
    }
}
