//! Host-shell state used to set up and assert end-to-end automation flows.

use crate::resolve::json_to_js;
use crate::{auto_err, require_host_context};
use lingxia_shell::ShellPinTarget;
use rong::{FromJSObject, HostError, JSContext, JSResult, JSValue, js_class, js_method};

#[js_class(clone)]
pub(crate) struct JSShellDriver {}

impl JSShellDriver {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

#[derive(FromJSObject)]
struct PinInput {
    kind: String,
    key: String,
}

#[derive(FromJSObject)]
struct SetPinOptions {
    kind: String,
    key: String,
    pinned: bool,
}

fn pin_target(kind: &str, key: String) -> JSResult<ShellPinTarget> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "lxapp" => Ok(ShellPinTarget::Lxapp { key }),
        "bookmark" => Ok(ShellPinTarget::Bookmark { key }),
        other => Err(auto_err(format!(
            "unknown shell Pin kind '{other}' (expected lxapp | bookmark)"
        ))),
    }
}

fn pins_to_js(ctx: &JSContext) -> JSResult<JSValue> {
    let pins = lingxia_shell::pins().map_err(|error| auto_err(error.to_string()))?;
    let value = serde_json::to_value(pins).map_err(|error| auto_err(error.to_string()))?;
    json_to_js(ctx, &value)
}

#[js_class(rename = "ShellDriver")]
impl JSShellDriver {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(
            rong::error::E_ILLEGAL_CONSTRUCTOR,
            "Use lx.automation().shell",
        )
        .into())
    }

    /// Read the ordered shortcuts currently projected into the host sidebar.
    #[js_method]
    async fn pins(&self, ctx: JSContext) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        pins_to_js(&ctx)
    }

    /// Commit a complete permutation of the current mixed list.
    #[js_method(rename = "reorderPins")]
    async fn reorder_pins(&self, ctx: JSContext, items: Vec<PinInput>) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let items = items
            .into_iter()
            .map(|item| pin_target(&item.kind, item.key).map(lingxia_shell::ShellPin))
            .collect::<JSResult<Vec<_>>>()?;
        lingxia_shell::reorder_pins(items).map_err(|error| auto_err(error.to_string()))?;
        pins_to_js(&ctx)
    }

    /// Idempotently set one shortcut through the real persisted shell manager.
    /// New Pins append; limit failures leave the existing order unchanged.
    /// Returning the complete order makes physical hit-testing deterministic.
    #[js_method(rename = "setPin")]
    async fn set_pin(&self, ctx: JSContext, options: SetPinOptions) -> JSResult<JSValue> {
        require_host_context(&ctx)?;
        let target = pin_target(&options.kind, options.key)?;
        lingxia_shell::set_pinned(target, options.pinned)
            .map_err(|error| auto_err(error.to_string()))?;
        pins_to_js(&ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::pin_target;

    #[test]
    fn pin_kind_is_explicit_and_case_insensitive() {
        assert!(matches!(
            pin_target("LxApp", "chat".to_string()).unwrap(),
            lingxia_shell::ShellPinTarget::Lxapp { key } if key == "chat"
        ));
        assert!(pin_target("surface", "chat".to_string()).is_err());
    }
}
