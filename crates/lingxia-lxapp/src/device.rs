//! Simulated-environment control shared by the devtool (`lxdev runner`) and
//! the `lx.automation()` host tier.
//!
//! The host runner owns the device presets and the window frame; it registers
//! a [`DeviceController`] here at startup. Both automation front-ends call the
//! `device_list` / `device_get` / `device_set` helpers, so neither embeds
//! runner specifics and the two can never drift.

use std::sync::OnceLock;

/// Simulated system appearance of the runner's device screen.
///
/// `System` follows the host OS; `Light`/`Dark` pin the scheme for the
/// simulated device only. The runner applies this at the WebView host level so
/// pages observe it through `prefers-color-scheme` — never through a DOM
/// override, which belongs to apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    System,
    Light,
    Dark,
}

impl Appearance {
    fn default_system() -> Self {
        Appearance::System
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }
}

impl std::str::FromStr for Appearance {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "system" => Ok(Appearance::System),
            "light" => Ok(Appearance::Light),
            "dark" => Ok(Appearance::Dark),
            other => Err(format!(
                "unknown appearance: {other} (expected system|light|dark)"
            )),
        }
    }
}

/// A device preset the host runner can simulate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceEntry {
    /// Stable preset id (e.g. "iphone-15-pro").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Form-factor group ("phone" | "tablet" | "desktop").
    pub group: String,
    /// Logical width in points.
    pub width: u32,
    /// Logical height in points.
    pub height: u32,
    /// True for the currently selected device.
    pub current: bool,
}

/// The active device selection reported by the host runner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceState {
    /// Selected preset id.
    pub id: String,
    /// Selected preset name.
    pub name: String,
    /// Form-factor group.
    pub group: String,
    /// Logical width in points (accounts for orientation).
    pub width: u32,
    /// Logical height in points (accounts for orientation).
    pub height: u32,
    /// True when the device is rotated to landscape.
    pub landscape: bool,
    /// Simulated system appearance of the device screen.
    #[serde(default = "Appearance::default_system")]
    pub appearance: Appearance,
    /// Whether the simulated host capsule is enabled. This is the setting, not
    /// per-device visibility: a desktop preset draws no phone chrome either
    /// way. Defaults to true — the capsule is real host chrome for every
    /// non-home lxapp, so hiding it is the opt-in.
    #[serde(default = "default_capsule")]
    pub capsule: bool,
}

fn default_capsule() -> bool {
    true
}

/// Host-provided controller for switching the simulated device. Implemented by
/// the runner binary (which owns the device presets and window frame) and
/// registered via [`register_device_controller`]; the `device_*` helpers call
/// through this indirection so callers stay platform-agnostic.
pub trait DeviceController: Send + Sync {
    fn list(&self) -> Result<Vec<DeviceEntry>, String>;
    fn get(&self) -> Result<DeviceState, String>;
    /// Partial update: only the provided fields change. `id: None` keeps the
    /// current preset, so orientation or appearance can flip on their own.
    fn set(
        &self,
        id: Option<&str>,
        landscape: Option<bool>,
        appearance: Option<Appearance>,
        capsule: Option<bool>,
    ) -> Result<DeviceState, String>;
}

static DEVICE_CONTROLLER: OnceLock<Box<dyn DeviceController>> = OnceLock::new();

/// Registers the host device controller for this process. First registration
/// wins; later ones are ignored.
pub fn register_device_controller(controller: Box<dyn DeviceController>) {
    if DEVICE_CONTROLLER.set(controller).is_err() {
        crate::warn!("device controller already registered; ignoring");
    }
}

fn device_controller() -> Result<&'static dyn DeviceController, String> {
    DEVICE_CONTROLLER
        .get()
        .map(|c| c.as_ref())
        .ok_or_else(|| "device switching is not supported by this host".to_string())
}

/// List the device presets the host runner offers.
pub fn device_list() -> Result<Vec<DeviceEntry>, String> {
    device_controller()?.list()
}

/// Report the currently selected device and orientation.
pub fn device_get() -> Result<DeviceState, String> {
    device_controller()?.get()
}

// Admission and enqueueing use the same lock, so an app cannot reserve a new
// Logic context between the transition's snapshot and its shutdown barrier.
static LOGIC_CREATION_PAUSED: std::sync::Mutex<bool> = std::sync::Mutex::new(false);
static DEVICE_CHANGE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) fn logic_creation_guard()
-> Result<std::sync::MutexGuard<'static, bool>, crate::LxAppError> {
    // The guard is held across Logic creation, so a panicking creation must not
    // wedge every later one behind a poisoned lock.
    let guard = LOGIC_CREATION_PAUSED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if *guard {
        return Err(crate::LxAppError::Runtime(
            "Runner device change is in progress".into(),
        ));
    }
    Ok(guard)
}

pub(crate) fn logic_creation_paused() -> bool {
    *LOGIC_CREATION_PAUSED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct CreationPause;
impl CreationPause {
    fn begin() -> Self {
        *LOGIC_CREATION_PAUSED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        Self
    }
}
impl Drop for CreationPause {
    fn drop(&mut self) {
        *LOGIC_CREATION_PAUSED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
    }
}

/// Change the simulated environment and await replacement Logic/page readiness.
/// Runs independently of the caller, so disconnecting cannot strand paused apps.
pub async fn device_set(
    id: Option<&str>,
    landscape: Option<bool>,
    appearance: Option<Appearance>,
    capsule: Option<bool>,
) -> Result<DeviceState, String> {
    let id = id.map(str::to_owned);
    crate::executor::spawn(async move { change_device(id, landscape, appearance, capsule).await })
        .await
        .map_err(|error| format!("device transition failed: {error}"))?
}

/// Native UI entry: never block a platform's main thread waiting for Logic.
pub fn request_device_set(id: String, landscape: Option<bool>) {
    std::mem::drop(crate::executor::spawn(async move {
        if let Err(error) = device_set(Some(&id), landscape, None, None).await {
            crate::error!("Runner device change failed: {error}");
        }
    }));
}

async fn apply_device(
    id: Option<String>,
    landscape: Option<bool>,
    appearance: Option<Appearance>,
    capsule: Option<bool>,
) -> Result<DeviceState, String> {
    tokio::task::spawn_blocking(move || {
        device_controller()?.set(id.as_deref(), landscape, appearance, capsule)
    })
    .await
    .map_err(|error| format!("device controller failed: {error}"))?
}

async fn resume_apps(apps: &[std::sync::Arc<crate::LxApp>]) -> Vec<String> {
    futures::future::join_all(apps.iter().map(|app| async move {
        app.resume_after_device_change()
            .await
            .err()
            .map(|error| format!("{}: {error}", app.appid))
    }))
    .await
    .into_iter()
    .flatten()
    .collect()
}

async fn change_device(
    id: Option<String>,
    landscape: Option<bool>,
    appearance: Option<Appearance>,
    capsule: Option<bool>,
) -> Result<DeviceState, String> {
    let _change = DEVICE_CHANGE.lock().await;
    let (previous, target_group) = tokio::task::spawn_blocking({
        let id = id.clone();
        move || -> Result<_, String> {
            let previous = device_get()?;
            let group = match id {
                Some(id) => {
                    device_list()?
                        .into_iter()
                        .find(|entry| entry.id == id)
                        .ok_or_else(|| format!("unknown device id: {id}"))?
                        .group
                }
                None => previous.group.clone(),
            };
            Ok((previous, group))
        }
    })
    .await
    .map_err(|error| error.to_string())??;
    if (previous.group == "desktop") == (target_group == "desktop") {
        return apply_device(id, landscape, appearance, capsule).await;
    }

    let pause = CreationPause::begin();
    let apps = crate::lxapp::get_lxapps_manager()
        .map(|manager| manager.live_logic_instances())
        .unwrap_or_default();
    let mut failures: Vec<String> = futures::future::join_all(apps.iter().map(|app| async move {
        app.quiesce_for_device_change()
            .await
            .err()
            .map(|error| format!("{}: {error}", app.appid))
    }))
    .await
    .into_iter()
    .flatten()
    .collect();
    if !failures.is_empty() {
        // The old environment remains authoritative if shutdown was not proven.
        drop(pause);
        failures.extend(resume_apps(&apps).await);
        return Err(failures.join("; "));
    }
    let applied = apply_device(id, landscape, appearance, capsule).await;
    if applied.is_err() {
        if let Err(error) = apply_device(
            Some(previous.id),
            Some(previous.landscape),
            Some(previous.appearance),
            Some(previous.capsule),
        )
        .await
        {
            failures.push(format!("device rollback failed: {error}"));
        }
    }
    // Every old context has terminated, and the selected environment is now
    // published. New admissions and replacement contexts may observe it.
    drop(pause);
    failures.extend(resume_apps(&apps).await);
    match applied {
        Ok(state) if failures.is_empty() => Ok(state),
        Ok(_) => Err(failures.join("; ")),
        Err(error) => {
            failures.insert(0, error);
            Err(failures.join("; "))
        }
    }
}
