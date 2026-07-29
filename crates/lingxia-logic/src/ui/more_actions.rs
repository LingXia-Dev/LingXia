//! `lx.setMoreActions` — app-declared actions for the host's secondary menu.

use lxapp::{
    LXAPP_MORE_ACTION_LIMIT, LxApp, LxAppMoreAction, register_app_handler, unregister_app_handler,
};
use rong::{JSContext, JSContextService, JSFunc, JSObject, JSResult};
use std::cell::RefCell;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct MoreActionHandlerRegistry {
    state: RefCell<MoreActionHandlerState>,
}

#[derive(Default)]
struct MoreActionHandlerState {
    appid: String,
    generation: u64,
    count: usize,
}

impl JSContextService for MoreActionHandlerRegistry {
    fn on_shutdown(&self) {
        let state = self.state.borrow();
        if state.generation == 0 || state.appid.is_empty() {
            return;
        }
        if let Some(app) = lxapp::try_get(&state.appid) {
            app.clear_more_actions_if_generation(state.generation);
        }
    }
}

fn registry(ctx: &JSContext) -> &MoreActionHandlerRegistry {
    if ctx.get_service::<MoreActionHandlerRegistry>().is_none() {
        ctx.set_service(MoreActionHandlerRegistry::default());
    }
    ctx.get_service::<MoreActionHandlerRegistry>()
        .expect("More action handler registry was inserted above")
}

fn event_name(generation: u64, index: usize) -> String {
    format!("lx.moreActions:{generation}:{index}")
}

fn required_string(item: &JSObject, field: &'static str) -> JSResult<String> {
    let value = item.get::<_, String>(field).map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("More action {field} must be a string"),
        )
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("More action {field} must not be empty"),
        )
        .into());
    }
    if value.chars().any(char::is_control) {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("More action {field} must not contain control characters"),
        )
        .into());
    }
    Ok(value.to_string())
}

fn reject_unknown_keys(item: &JSObject) -> JSResult<()> {
    for key in item.keys_as::<String>()? {
        if !["icon", "label", "onClick"].contains(&key.as_str()) {
            return Err(rong::HostError::new(
                rong::error::E_INVALID_ARG,
                format!("unknown More action field '{key}'"),
            )
            .into());
        }
    }
    Ok(())
}

fn validate_icon(icon: &str) -> JSResult<()> {
    let path = Path::new(icon);
    let is_lx_uri = icon.starts_with("lx://");
    if icon.chars().any(char::is_control)
        || !is_lx_uri
            && (has_uri_scheme(icon)
                || path.is_absolute()
                || path.components().any(|component| {
                    matches!(
                        component,
                        Component::ParentDir | Component::RootDir | Component::Prefix(_)
                    )
                }))
    {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("More action icon '{icon}' must be an lxapp-accessible local resource path"),
        )
        .into());
    }
    Ok(())
}

fn has_uri_scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

struct ParsedMoreAction {
    item: LxAppMoreAction,
    handler: JSFunc,
}

fn parse_action(app: &LxApp, item: &JSObject) -> JSResult<ParsedMoreAction> {
    reject_unknown_keys(item)?;
    let icon = required_string(item, "icon")?;
    validate_icon(&icon)?;
    let label = required_string(item, "label")?;
    let handler = item.get::<_, JSFunc>("onClick").map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            "More action onClick must be a function",
        )
    })?;
    let icon_path = app.resolve_accessible_path(&icon).map_err(|_| {
        rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("More action icon '{icon}' could not be resolved"),
        )
    })?;
    Ok(ParsedMoreAction {
        item: LxAppMoreAction {
            label,
            icon_path: icon_path.to_string_lossy().into_owned(),
        },
        handler,
    })
}

/// Replace the current lxapp's complete app-declared More action list (two
/// entries maximum). Pass an empty array to clear it. Native hosts merge these
/// entries with their own lifecycle actions.
fn set_more_actions(ctx: JSContext, items: Vec<JSObject>) -> JSResult<()> {
    if items.len() > LXAPP_MORE_ACTION_LIMIT {
        return Err(rong::HostError::new(
            rong::error::E_INVALID_ARG,
            format!("setMoreActions accepts at most {LXAPP_MORE_ACTION_LIMIT} items"),
        )
        .into());
    }

    let app = LxApp::from_ctx(&ctx)?;
    let parsed = items
        .iter()
        .map(|item| parse_action(&app, item))
        .collect::<JSResult<Vec<_>>>()?;
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed).max(1);

    let mut registered = 0usize;
    for (index, action) in parsed.iter().enumerate() {
        if let Err(error) =
            register_app_handler(&ctx, &event_name(generation, index), action.handler.clone())
        {
            for rollback in 0..registered {
                unregister_app_handler(&ctx, &event_name(generation, rollback), None);
            }
            return Err(error);
        }
        registered += 1;
    }

    let next_items = parsed.into_iter().map(|action| action.item).collect();
    app.replace_more_actions(generation, next_items);

    let registry = registry(&ctx);
    let mut state = registry.state.borrow_mut();
    for index in 0..state.count {
        unregister_app_handler(&ctx, &event_name(state.generation, index), None);
    }
    state.appid = app.appid.clone();
    state.generation = generation;
    state.count = registered;
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let _ = registry(ctx);
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn setMoreActions(
            ts_params = "items: MoreAction[]"
        ) = set_more_actions;
    }
}

#[cfg(test)]
mod tests {
    use super::{has_uri_scheme, validate_icon};

    #[test]
    fn icon_validation_rejects_external_and_traversing_paths() {
        assert!(validate_icon("public/action.svg").is_ok());
        assert!(validate_icon("lx://userdata/action.png").is_ok());
        assert!(validate_icon("https://example.com/action.svg").is_err());
        assert!(validate_icon("../action.svg").is_err());
        assert!(validate_icon("public/action\n.svg").is_err());
    }

    #[test]
    fn uri_scheme_detection_does_not_treat_windows_fragments_as_relative() {
        assert!(has_uri_scheme("C:/action.png"));
        assert!(!has_uri_scheme("public/action.svg"));
    }
}
