use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_shell::{
    ResolvedShellSidebarAction, ShellError, ShellHost, ShellPin, ShellPinTarget, ShellResult,
    ShellSidebarAction, SidebarActionIntent,
};
use std::path::{Component, Path};
use std::sync::Arc;

struct HostShell {
    platform: Arc<Platform>,
}

pub(crate) fn initialize(platform: Arc<Platform>) -> ShellResult<()> {
    let root = platform.app_data_dir();
    lingxia_shell::initialize(root, Arc::new(HostShell { platform }))?;
    let manager = lingxia_shell::manager()?;
    if let Some(home_appid) = lingxia_app_context::home_app_id() {
        let target = ShellPinTarget::Lxapp {
            key: home_appid.to_string(),
        };
        if let Err(error) = manager.unpin(&target) {
            log::warn!("failed to remove obsolete home lxapp Pin: {error}");
        }
    }
    let _ = lingxia_shell::apply_current_pins();
    Ok(())
}

impl ShellHost for HostShell {
    fn resolve_sidebar_actions(
        &self,
        generation: u64,
        items: &[ShellSidebarAction],
    ) -> ShellResult<Vec<ResolvedShellSidebarAction>> {
        let owner = shell_owner();
        items
            .iter()
            .map(|item| {
                Ok(ResolvedShellSidebarAction {
                    generation,
                    id: item.id.clone(),
                    placement: item.placement,
                    label: item.label.clone(),
                    icon_path: Some(resolve_declared_icon(owner.as_deref(), &item.icon)?),
                    disabled: item.disabled,
                })
            })
            .collect()
    }

    fn apply_sidebar_actions(&self, items: &[ResolvedShellSidebarAction]) -> ShellResult<()> {
        self.platform
            .set_shell_sidebar_actions(items)
            .map_err(|error| ShellError::Host(error.to_string()))
    }

    fn apply_pins(&self, items: &[lingxia_shell::ShellPin]) -> ShellResult<()> {
        let visible = visible_shell_pins(items, lingxia_app_context::home_app_id());
        self.platform
            .set_shell_pins(&visible)
            .map_err(|error| ShellError::Host(error.to_string()))
    }

    fn activate(&self, intent: SidebarActionIntent) -> ShellResult<()> {
        let owner = lingxia_app_context::home_app_id().ok_or(ShellError::NotInitialized)?;
        let event = format!(
            "lx.shell.sidebarActions:{}:{}",
            intent.generation, intent.id
        );
        lxapp::publish_app_event(owner, &event, None);
        Ok(())
    }
}

fn resolve_declared_icon(owner: Option<&lxapp::LxApp>, icon: &str) -> ShellResult<String> {
    let icon = icon.trim();
    let path = Path::new(icon);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ShellError::InvalidSidebarActionIcon {
            icon: icon.to_string(),
        });
    }
    let resolved = owner
        .ok_or(ShellError::NotInitialized)?
        .resolve_accessible_path(icon)
        .map_err(|_| ShellError::InvalidSidebarActionIcon {
            icon: icon.to_string(),
        })?;
    Ok(resolved.to_string_lossy().into_owned())
}

pub(crate) fn visible_shell_pins(items: &[ShellPin], home_appid: Option<&str>) -> Vec<ShellPin> {
    items
        .iter()
        .filter(|pin| {
            !matches!(
                (&pin.0, home_appid),
                (ShellPinTarget::Lxapp { key }, Some(home)) if key == home
            )
        })
        .cloned()
        .collect()
}

fn shell_owner() -> Option<Arc<lxapp::LxApp>> {
    lingxia_app_context::home_app_id().and_then(lxapp::try_get)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_shell::{ShellPin, ShellPinTarget};

    #[test]
    fn missing_declared_icon_stays_missing() {
        assert_eq!(
            resolve_declared_icon(None, "icons/action.svg"),
            Err(ShellError::NotInitialized)
        );
    }

    #[test]
    fn declared_icon_rejects_parent_traversal() {
        assert_eq!(
            resolve_declared_icon(None, "../action.svg"),
            Err(ShellError::InvalidSidebarActionIcon {
                icon: "../action.svg".to_string()
            })
        );
    }

    #[test]
    fn home_lxapp_is_never_applied_as_a_pin() {
        let pins = vec![
            ShellPin(ShellPinTarget::Lxapp {
                key: "home".to_string(),
            }),
            ShellPin(ShellPinTarget::Lxapp {
                key: "chat".to_string(),
            }),
            ShellPin(ShellPinTarget::Bookmark {
                key: "bookmark".to_string(),
            }),
        ];

        assert_eq!(visible_shell_pins(&pins, Some("home")), pins[1..]);
    }
}
