use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::{AppRuntime, LxAppOpenMode};
use lingxia_shell::{
    ActivatorKind, NativeShellCapability, ResolvedShellActivator, ShellActivationIntent,
    ShellActivator, ShellActivatorTarget, ShellError, ShellHost, ShellResult,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

struct HostShell {
    platform: Arc<Platform>,
}

pub(crate) fn initialize(platform: Arc<Platform>) -> ShellResult<()> {
    let root = platform.app_data_dir();
    lingxia_shell::initialize(root, Arc::new(HostShell { platform }))?;
    if lingxia_shell::manager()?.snapshot().activators.declared() {
        let _ = lingxia_shell::apply_current_activators();
    }
    let _ = lingxia_shell::apply_current_pins();
    Ok(())
}

impl ShellHost for HostShell {
    fn resolve_activators(
        &self,
        items: &[ShellActivator],
    ) -> ShellResult<Vec<ResolvedShellActivator>> {
        let plan = shell_owner().and_then(|owner| owner.surface_derived_layout());
        items
            .iter()
            .map(|item| {
                let (kind, fallback_label, fallback_icon, active, unavailable) = match &item.target
                {
                    ShellActivatorTarget::Lxapp { key } => {
                        let info = lxapp::try_get(key).map(|app| app.get_lxapp_info());
                        let label = info
                            .as_ref()
                            .map(|info| info.app_name.trim())
                            .filter(|label| !label.is_empty())
                            .unwrap_or(key)
                            .to_string();
                        let icon = info
                            .map(|info| info.icon)
                            .filter(|icon| !icon.trim().is_empty());
                        (
                            ActivatorKind::Lxapp,
                            label,
                            icon,
                            lxapp_target_active(key, plan.as_ref()),
                            false,
                        )
                    }
                    ShellActivatorTarget::Native { key } => match key {
                        NativeShellCapability::Terminal => {
                            let available = lingxia_app_context::terminal_enabled();
                            (
                                ActivatorKind::Native,
                                "Terminal".to_string(),
                                None,
                                available && self.platform.shell_native_active(*key),
                                !available,
                            )
                        }
                    },
                    ShellActivatorTarget::Action => {
                        (ActivatorKind::Action, item.id.clone(), None, false, false)
                    }
                };
                Ok(ResolvedShellActivator {
                    id: item.id.clone(),
                    kind,
                    label: item.label.clone().unwrap_or(fallback_label),
                    icon_path: item.icon.clone().or(fallback_icon),
                    active: kind != ActivatorKind::Action && active,
                    disabled: item.disabled || unavailable,
                })
            })
            .collect()
    }

    fn apply_activators(&self, items: &[ResolvedShellActivator]) -> ShellResult<()> {
        self.platform
            .set_shell_activators(items)
            .map_err(|error| ShellError::Host(error.to_string()))
    }

    fn apply_pins(&self, items: &[lingxia_shell::ShellPin]) -> ShellResult<()> {
        self.platform
            .set_shell_pins(items)
            .map_err(|error| ShellError::Host(error.to_string()))
    }

    fn activate(&self, intent: ShellActivationIntent) -> ShellResult<()> {
        match intent {
            ShellActivationIntent::Lxapp { key } => activate_lxapp(&key),
            ShellActivationIntent::Native { key } => self
                .platform
                .activate_shell_native(key)
                .map_err(|error| ShellError::Host(error.to_string())),
            ShellActivationIntent::Action { id, generation } => {
                let owner = lingxia_app_context::home_app_id().ok_or(ShellError::NotInitialized)?;
                let event = format!("lx.shell.activators:{generation}:{id}");
                lxapp::publish_app_event(owner, &event, None);
                Ok(())
            }
        }
    }
}

fn shell_owner() -> Option<Arc<lxapp::LxApp>> {
    lingxia_app_context::home_app_id().and_then(lxapp::try_get)
}

fn lxapp_target_active(
    appid: &str,
    plan: Option<&lingxia_surface::LayoutPresentationPlan>,
) -> bool {
    match lxapp::open_region(appid) {
        Some(lxapp::LxAppOpenRegion::Main) => {
            plan.and_then(|plan| plan.active_main_id.as_deref()) == Some(appid)
        }
        Some(lxapp::LxAppOpenRegion::Aside) => {
            let surface_id = lxapp_aside_surface_id(appid);
            plan.is_some_and(|plan| {
                plan.aside_slots.iter().any(|slot| {
                    slot.visible && slot.active_child.as_deref() == Some(surface_id.as_str())
                })
            })
        }
        None => false,
    }
}

fn activate_lxapp(appid: &str) -> ShellResult<()> {
    let owner = shell_owner().ok_or(ShellError::NotInitialized)?;
    let surface_id = lxapp_aside_surface_id(appid);
    match lxapp::open_region(appid) {
        Some(lxapp::LxAppOpenRegion::Main) => {
            let target = lxapp::try_get(appid)
                .ok_or_else(|| ShellError::Host(format!("opened lxapp is unavailable: {appid}")))?;
            target.set_active_main();
        }
        Some(lxapp::LxAppOpenRegion::Aside)
            if lxapp_target_active(appid, owner.surface_derived_layout().as_ref()) =>
        {
            owner
                .set_shell_surface_visible(&surface_id, false, None)
                .map_err(|error| ShellError::Host(error.to_string()))?;
        }
        Some(lxapp::LxAppOpenRegion::Aside) => {
            let in_graph = owner
                .surface_derived_layout()
                .is_some_and(|plan| plan.asides.iter().any(|aside| aside.id == surface_id));
            if in_graph {
                owner.focus_shell_surface(&surface_id);
            } else {
                open_lxapp_aside(&owner, appid, &surface_id)?;
            }
        }
        None => schedule_lxapp_aside_open(owner.appid.clone(), appid.to_string()),
    }
    Ok(())
}

fn lxapp_aside_surface_id(appid: &str) -> String {
    lxapp::try_get(appid)
        .and_then(|app| app.open_panel_id())
        .or_else(|| {
            lingxia_app_context::app_config()
                .and_then(|config| config.panels.as_ref().cloned())
                .and_then(|panels| {
                    panels.items.into_iter().find_map(|item| {
                        (item.content.kind.is_lxapp() && item.content.app_id == appid)
                            .then_some(item.id)
                    })
                })
        })
        .unwrap_or_else(|| appid.to_string())
}

fn schedule_lxapp_aside_open(owner_appid: String, appid: String) {
    let pending = pending_lxapp_opens();
    if !pending
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(appid.clone())
    {
        return;
    }
    std::mem::drop(crate::task::spawn(async move {
        if let Err(error) = lxapp::prepare_lxapp_open(&appid, lxapp::ReleaseType::Release).await {
            log::error!("shell activator could not prepare lxapp {appid}: {error}");
            release_pending_lxapp_open(&appid);
            return;
        }
        let Some(owner) = lxapp::try_get(&owner_appid) else {
            log::warn!("shell owner disappeared before opening activator target {appid}");
            release_pending_lxapp_open(&appid);
            return;
        };
        let surface_id = lxapp_aside_surface_id(&appid);
        if let Err(error) = open_lxapp_aside(&owner, &appid, &surface_id) {
            log::error!("shell activator could not open lxapp {appid}: {error}");
        } else {
            lxapp::schedule_lxapp_update_check(&appid, lxapp::ReleaseType::Release);
            let _ = lingxia_shell::apply_current_activators();
        }
        release_pending_lxapp_open(&appid);
    }));
}

fn pending_lxapp_opens() -> &'static Mutex<HashSet<String>> {
    static PENDING: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashSet::new()))
}

fn release_pending_lxapp_open(appid: &str) {
    pending_lxapp_opens()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(appid);
}

fn open_lxapp_aside(owner: &lxapp::LxApp, appid: &str, surface_id: &str) -> ShellResult<()> {
    lxapp::open_lxapp(
        appid,
        lxapp::LxAppStartupOptions::new("")
            .set_open_mode(LxAppOpenMode::Panel)
            .set_panel_id(surface_id.to_string()),
    )
    .map_err(|error| ShellError::Host(error.to_string()))?;
    owner.register_host_aside(surface_id, lxapp_aside_edge(appid));
    Ok(())
}

fn lxapp_aside_edge(appid: &str) -> &'static str {
    let position = lingxia_app_context::app_config()
        .and_then(|config| config.panels.as_ref().cloned())
        .and_then(|panels| {
            panels.items.into_iter().find_map(|item| {
                (item.content.kind.is_lxapp() && item.content.app_id == appid)
                    .then_some(item.position)
            })
        });
    match position {
        Some(lingxia_app_context::PanelPosition::Left) => "left",
        Some(lingxia_app_context::PanelPosition::Top) => "top",
        Some(lingxia_app_context::PanelPosition::Bottom) => "bottom",
        Some(lingxia_app_context::PanelPosition::Right) | None => "right",
    }
}
