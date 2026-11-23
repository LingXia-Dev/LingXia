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

/// Simulated contrast setting of the device: `High` is the system's increased contrast.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Contrast {
    #[default]
    Standard,
    High,
}

impl std::str::FromStr for Contrast {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "standard" => Ok(Contrast::Standard),
            "high" => Ok(Contrast::High),
            other => Err(format!(
                "unknown contrast: {other} (expected standard|high)"
            )),
        }
    }
}

/// The text scales a simulated device takes: the system's smallest to its largest
/// accessibility size.
pub const TEXT_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.5..=3.0;

/// A change to the simulated environment; `None` keeps what is set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceChange {
    /// Preset id.
    pub id: Option<String>,
    pub landscape: Option<bool>,
    pub appearance: Option<Appearance>,
    /// The system text size as a factor of the default, within [`TEXT_SCALE_RANGE`]; `f64` so
    /// the decimal a user wrote reads back as written.
    pub text_scale: Option<f64>,
    pub contrast: Option<Contrast>,
    /// Posture id of a folding device (see [`DeviceEntry::postures`]); applies to `id`, or to
    /// the current device when `id` is `None`.
    pub posture: Option<String>,
}

impl DeviceChange {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn validate(&self) -> Result<(), String> {
        match self.text_scale {
            Some(scale) if !TEXT_SCALE_RANGE.contains(&scale) => Err(format!(
                "text scale {scale} is outside {}..={}",
                TEXT_SCALE_RANGE.start(),
                TEXT_SCALE_RANGE.end()
            )),
            _ => Ok(()),
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
    /// Form-factor group ("phone" | "fold" | "tablet" | "desktop").
    pub group: String,
    /// Logical width in points; fractional where the device's pixels over its density are.
    pub width: f64,
    /// Logical height in points.
    pub height: f64,
    /// Device pixels per point: a GPU page renders at it, so its screenshot is
    /// `width * pixel_ratio` by `height * pixel_ratio` pixels.
    #[serde(rename = "pixelRatio", default = "one")]
    pub pixel_ratio: f32,
    /// True for the currently selected device.
    pub current: bool,
    /// The postures a folding device takes; empty for a flat one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub postures: Vec<DevicePostureEntry>,
}

/// One posture of a folding device: its own screen size.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DevicePostureEntry {
    /// Stable posture id (`outer`, `open`, `dual`, `tabletop`).
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub width: f64,
    pub height: f64,
}

/// The adaptive presets a test names instead of a device id.
pub const DEVICE_PRESETS: &[&str] = &["phone", "fold", "trifold", "pad", "desktop"];

/// Resolve an adaptive preset to the Runner's device id: `fold` is a folding device without a
/// `dual` posture, `trifold` one with it; with `posture`, one that has it. A preset or posture
/// the host lacks is an error, never a silent fallback to another form factor.
pub fn resolve_device_preset(preset: &str, posture: Option<&str>) -> Result<String, String> {
    let devices = device_list()?;
    let trifold = |entry: &DeviceEntry| entry.postures.iter().any(|p| p.id == "dual");
    // The group aliases the Runner's device manifest accepts.
    let folding = |entry: &DeviceEntry| matches!(entry.group.as_str(), "fold" | "foldable");
    let matches = |entry: &DeviceEntry| match preset {
        "phone" => entry.group == "phone",
        "fold" => folding(entry) && !trifold(entry),
        "trifold" => folding(entry) && trifold(entry),
        "pad" => matches!(entry.group.as_str(), "tablet" | "pad"),
        "desktop" => entry.group == "desktop",
        _ => false,
    };
    if !DEVICE_PRESETS.contains(&preset) {
        return Err(format!(
            "unknown device preset '{preset}' (expected {})",
            DEVICE_PRESETS.join("|")
        ));
    }
    if !devices.iter().any(&matches) {
        return Err(format!("this host has no '{preset}' device"));
    }
    let takes = |entry: &DeviceEntry| {
        matches(entry)
            && posture.is_none_or(|posture| entry.postures.iter().any(|p| p.id == posture))
    };
    // Repeating a preset keeps the device already chosen within it.
    devices
        .iter()
        .find(|entry| entry.current && takes(entry))
        .or_else(|| devices.iter().find(|entry| takes(entry)))
        .map(|entry| entry.id.clone())
        .ok_or_else(|| {
            format!(
                "no '{preset}' device has posture '{}'",
                posture.unwrap_or_default()
            )
        })
}

/// Build a [`DeviceChange`] target from the adaptive options both automation front-ends take:
/// an `id` or a `preset`, a `posture`, and `orientation` or `landscape`.
#[allow(clippy::type_complexity)]
pub fn adaptive_device_target(
    id: Option<String>,
    preset: Option<&str>,
    posture: Option<String>,
    orientation: Option<&str>,
    landscape: Option<bool>,
) -> Result<(Option<String>, Option<String>, Option<bool>), String> {
    let id = match (id, preset) {
        (Some(_), Some(_)) => return Err("pass either id or preset, not both".into()),
        (Some(id), None) => Some(id),
        (None, Some(preset)) => Some(resolve_device_preset(preset, posture.as_deref())?),
        (None, None) => None,
    };
    let oriented = match orientation {
        None => None,
        Some("landscape") => Some(true),
        Some("portrait") => Some(false),
        Some(other) => {
            return Err(format!(
                "orientation must be 'landscape' or 'portrait', not '{other}'"
            ));
        }
    };
    let landscape = match (landscape, oriented) {
        (Some(a), Some(b)) if a != b => {
            return Err("landscape and orientation disagree".into());
        }
        (value, oriented) => value.or(oriented),
    };
    Ok((id, posture, landscape))
}

fn one() -> f32 {
    1.0
}

fn default_text_scale() -> f64 {
    1.0
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
    pub width: f64,
    /// Logical height in points (accounts for orientation).
    pub height: f64,
    /// Device pixels per point.
    #[serde(rename = "pixelRatio", default = "one")]
    pub pixel_ratio: f32,
    /// True when the device is rotated to landscape.
    pub landscape: bool,
    /// Simulated system appearance of the device screen.
    #[serde(default = "Appearance::default_system")]
    pub appearance: Appearance,
    /// Simulated system text size, a factor of the default.
    #[serde(rename = "textScale", default = "default_text_scale")]
    pub text_scale: f64,
    /// Simulated system contrast.
    #[serde(default)]
    pub contrast: Contrast,
    /// Active posture of a folding device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub posture: Option<String>,
}

/// Host-provided controller for switching the simulated device. Implemented by
/// the runner binary (which owns the device presets and window frame) and
/// registered via [`register_device_controller`]; the `device_*` helpers call
/// through this indirection so callers stay platform-agnostic.
pub trait DeviceController: Send + Sync {
    fn list(&self) -> Result<Vec<DeviceEntry>, String>;
    fn get(&self) -> Result<DeviceState, String>;
    /// Partial update: only the provided fields change, so orientation,
    /// appearance, text scale or contrast can each change on their own.
    fn set(&self, change: &DeviceChange) -> Result<DeviceState, String>;
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
pub async fn device_set(change: DeviceChange) -> Result<DeviceState, String> {
    change.validate()?;
    crate::executor::spawn(async move { change_device(change).await })
        .await
        .map_err(|error| format!("device transition failed: {error}"))?
}

/// Native UI entry: never block a platform's main thread waiting for Logic.
pub fn request_device_change(change: DeviceChange) {
    std::mem::drop(crate::executor::spawn(async move {
        if let Err(error) = device_set(change).await {
            crate::error!("Runner device change failed: {error}");
        }
    }));
}

pub fn request_device_set(id: String, landscape: Option<bool>) {
    request_device_change(DeviceChange {
        id: Some(id),
        landscape,
        ..Default::default()
    });
}

async fn apply_device(change: DeviceChange) -> Result<DeviceState, String> {
    tokio::task::spawn_blocking(move || device_controller()?.set(&change))
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

async fn change_device(change: DeviceChange) -> Result<DeviceState, String> {
    let _change = DEVICE_CHANGE.lock().await;
    let (previous, target_group) = tokio::task::spawn_blocking({
        let id = change.id.clone();
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
        return apply_device(change).await;
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
    let applied = apply_device(change).await;
    if applied.is_err()
        && let Err(error) = apply_device(DeviceChange {
            id: Some(previous.id),
            landscape: Some(previous.landscape),
            appearance: Some(previous.appearance),
            text_scale: Some(previous.text_scale),
            contrast: Some(previous.contrast),
            posture: previous.posture,
        })
        .await
    {
        failures.push(format!("device rollback failed: {error}"));
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
