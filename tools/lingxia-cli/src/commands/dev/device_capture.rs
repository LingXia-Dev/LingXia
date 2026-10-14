//! Screenshots a host test run stages here for `lxdev test` to collect:
//! `session.capture.put` (a PNG the host captured in process and sent as a
//! binary frame) and `session.device.screenshot` (a phone's display, taken
//! from outside the app: adb, idevicescreenshot, hdc).
//!
//! The run's output directory belongs to `lxdev test`, which names it after
//! the run id only once the run has started, so the server cannot write
//! there. It stages each PNG under the run instead and answers the runtime
//! with the size alone; `lxdev` copies the staged files into its output
//! directory when the run ends (`session.device.captures`).

use super::DevServerState;
use lingxia_control_protocol::dev_session::{
    Capture, CaptureArgs, DevSessionMessage, DeviceCaptures, DeviceCapturesArgs,
};
use lingxia_control_protocol::methods::session::{capture, device};
use std::path::{Component, Path, PathBuf};

/// The display `session.device.screenshot` captures. Desktop hosts capture
/// their own screen in process and never ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeviceTarget {
    /// `adb exec-out screencap`; `None` lets adb pick its only device.
    Android {
        serial: Option<String>,
    },
    /// `idevicescreenshot` (libimobiledevice) on the device `devicectl`
    /// resolved; `None` takes the only available one.
    Ios {
        device: Option<String>,
    },
    /// `hdc shell snapshot_display`.
    Harmony {
        target: Option<String>,
    },
    Unsupported {
        platform: String,
    },
}

impl DeviceTarget {
    pub(super) fn for_platform(platform: &str) -> Self {
        match platform {
            "android" => Self::Android { serial: None },
            "ios" => Self::Ios { device: None },
            "harmony" => Self::Harmony { target: None },
            other => Self::Unsupported {
                platform: other.to_string(),
            },
        }
    }

    /// The device `lingxia dev` resolved for the session.
    pub(super) fn with_device(self, id: &str) -> Self {
        let id = Some(id.to_string());
        match self {
            Self::Android { .. } => Self::Android { serial: id },
            Self::Ios { .. } => Self::Ios { device: id },
            Self::Harmony { .. } => Self::Harmony { target: id },
            other => other,
        }
    }
}

type CaptureError = (&'static str, String);

impl DevServerState {
    pub(crate) fn set_device_target(&self, target: DeviceTarget) {
        *self
            .device_target
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = target;
    }

    pub(crate) fn set_capture_device(&self, id: &str) {
        let mut target = self
            .device_target
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *target = target.clone().with_device(id);
    }

    /// The staging root, created on first use and removed with the server.
    fn capture_root(&self) -> std::io::Result<PathBuf> {
        let mut root = self
            .capture_root
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if root.is_none() {
            *root = Some(
                tempfile::Builder::new()
                    .prefix("lingxia-dev-captures-")
                    .tempdir()?,
            );
        }
        Ok(root
            .as_ref()
            .map(|dir| dir.path().to_path_buf())
            .unwrap_or_default())
    }

    /// Answer the runtime's `session.capture.put`.
    pub(super) fn capture_put_reply(
        &self,
        id: String,
        params: Option<serde_json::Value>,
        png: &[u8],
    ) -> DevSessionMessage {
        let result = parse_args(capture::PUT, params).and_then(|args| self.stage(&args, png));
        reply(id, result)
    }

    /// Answer the runtime's `session.device.screenshot`.
    pub(super) fn device_screenshot_reply(
        &self,
        id: String,
        params: Option<serde_json::Value>,
    ) -> DevSessionMessage {
        let target = self
            .device_target
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let result = parse_args(device::SCREENSHOT, params).and_then(|args| {
            staged_path(&args.run_id, &args.name).map_err(invalid)?;
            let png = capture_display(&target)?;
            self.stage(&args, &png)
        });
        reply(id, result)
    }

    fn stage(&self, args: &CaptureArgs, png: &[u8]) -> Result<Capture, CaptureError> {
        let relative = staged_path(&args.run_id, &args.name).map_err(invalid)?;
        let (width, height) = png_size(png).ok_or_else(|| {
            (
                capture::FAILED,
                format!("the capture is not a PNG ({} bytes)", png.len()),
            )
        })?;
        let failed =
            |err: std::io::Error| (capture::FAILED, format!("cannot stage the capture: {err}"));
        let path = self.capture_root().map_err(failed)?.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(failed)?;
        }
        std::fs::write(&path, png).map_err(failed)?;
        Ok(Capture {
            path: args.name.clone(),
            width,
            height,
            scale: args.scale,
        })
    }

    /// Answer `lxdev`'s `session.device.captures`: what a run staged.
    pub(super) fn device_captures_reply(
        &self,
        id: String,
        params: Option<serde_json::Value>,
    ) -> DevSessionMessage {
        let args: DeviceCapturesArgs = match serde_json::from_value(params.unwrap_or_default()) {
            Ok(args) => args,
            Err(err) => {
                return DevSessionMessage::error(
                    id,
                    "invalid_params",
                    format!("{}: {err}", device::CAPTURES),
                );
            }
        };
        match self.staged_captures(&args.run_id) {
            Ok(captures) => DevSessionMessage::success(
                id,
                Some(serde_json::to_value(captures).unwrap_or_default()),
            ),
            Err(message) => DevSessionMessage::error(id, "invalid_params", message),
        }
    }

    fn staged_captures(&self, run_id: &str) -> Result<DeviceCaptures, String> {
        check_component(run_id, "run id")?;
        let root = self.capture_root.lock().unwrap_or_else(|p| p.into_inner());
        let Some(dir) = root.as_ref().map(|root| root.path().join(run_id)) else {
            return Ok(DeviceCaptures::default());
        };
        let mut files = Vec::new();
        collect_files(&dir, &dir, &mut files);
        files.sort();
        Ok(DeviceCaptures {
            dir: (!files.is_empty()).then(|| dir.display().to_string()),
            files,
        })
    }
}

fn invalid(message: String) -> CaptureError {
    ("invalid_params", message)
}

fn parse_args(
    method: &str,
    params: Option<serde_json::Value>,
) -> Result<CaptureArgs, CaptureError> {
    serde_json::from_value(params.unwrap_or_default())
        .map_err(|err| invalid(format!("{method}: {err}")))
}

fn reply(id: String, result: Result<Capture, CaptureError>) -> DevSessionMessage {
    match result {
        Ok(capture) => {
            DevSessionMessage::success(id, Some(serde_json::to_value(capture).unwrap_or_default()))
        }
        Err((code, message)) => DevSessionMessage::error(id, code, message),
    }
}

fn capture_display(target: &DeviceTarget) -> Result<Vec<u8>, CaptureError> {
    let failed = |err: anyhow::Error| (capture::FAILED, format!("{err:#}"));
    match target {
        DeviceTarget::Android { serial } => {
            crate::platform::android::screencap_png(serial.as_deref()).map_err(failed)
        }
        DeviceTarget::Ios { device } => iphone_png(device.as_deref()),
        DeviceTarget::Harmony { target } => {
            crate::platform::harmony::snapshot_display(target.as_deref())
                .and_then(|image| to_png(&image))
                .map_err(failed)
        }
        DeviceTarget::Unsupported { platform } => Err((
            capture::UNSUPPORTED,
            format!("device capture is not supported on {platform}"),
        )),
    }
}

fn iphone_png(device: Option<&str>) -> Result<Vec<u8>, CaptureError> {
    use crate::platform::apple::devicectl::DeviceCtl;
    use anyhow::Context;
    let tool = which::which("idevicescreenshot").map_err(|_| {
        (
            capture::UNSUPPORTED,
            "device capture on an iPhone needs idevicescreenshot (`brew install libimobiledevice`)"
                .to_string(),
        )
    })?;
    let failed = |err: anyhow::Error| (capture::FAILED, format!("{err:#}"));
    let iphone = match device {
        Some(id) => DeviceCtl::get_device(id),
        None => DeviceCtl::list_devices().and_then(|devices| {
            devices
                .into_iter()
                .find(|device| device.is_available())
                .context("no available iPhone")
        }),
    }
    .map_err(failed)?;
    let udid = iphone
        .udid()
        .ok_or_else(|| {
            (
                capture::FAILED,
                format!("{} reports no UDID", iphone.description()),
            )
        })?
        .to_string();
    let file = tempfile::Builder::new()
        .prefix("lingxia-capture-")
        .suffix(".png")
        .tempfile()
        .map_err(|err| {
            (
                capture::FAILED,
                format!("cannot create a capture file: {err}"),
            )
        })?;
    let mut command = std::process::Command::new(tool);
    command.args(["-u", &udid]);
    if iphone.is_wifi() {
        command.arg("-n");
    }
    let output = command.arg(file.path()).output().map_err(|err| {
        (
            capture::FAILED,
            format!("cannot run idevicescreenshot: {err}"),
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // iOS 17 and later no longer offer the lockdown screenshotr service.
        if stderr.contains("screenshotr") {
            return Err((
                capture::UNSUPPORTED,
                format!(
                    "device capture is not supported on this iPhone (iOS {}): it no longer offers the screenshotr service idevicescreenshot uses",
                    iphone.os_version().unwrap_or("?")
                ),
            ));
        }
        return Err((
            capture::FAILED,
            format!("idevicescreenshot failed: {stderr}"),
        ));
    }
    let image = std::fs::read(file.path())
        .map_err(|err| (capture::FAILED, format!("cannot read the capture: {err}")))?;
    to_png(&image).map_err(failed)
}

fn to_png(image: &[u8]) -> anyhow::Result<Vec<u8>> {
    if png_size(image).is_some() {
        return Ok(image.to_vec());
    }
    let decoded = image::load_from_memory(image)?;
    let mut png = std::io::Cursor::new(Vec::new());
    decoded.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(png.into_inner())
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else if let Ok(relative) = path.strip_prefix(root) {
            let parts: Vec<_> = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.push(parts.join("/"));
        }
    }
}

fn check_component(value: &str, what: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && value != "."
        && value != "..";
    if valid {
        Ok(())
    } else {
        Err(format!("invalid {what} {value:?}"))
    }
}

/// `<run id>/<name>`, refusing anything that could leave the run's staging
/// directory: the runtime names the file.
fn staged_path(run_id: &str, name: &str) -> Result<PathBuf, String> {
    check_component(run_id, "run id")?;
    let relative = Path::new(name);
    let plain = relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    if name.is_empty() || !plain || !name.ends_with(".png") {
        return Err(format!(
            "capture name {name:?} must be a relative .png path"
        ));
    }
    Ok(Path::new(run_id).join(relative))
}

/// Width and height from a PNG's IHDR chunk.
fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if png.len() < 24 || !png.starts_with(SIGNATURE) || &png[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(png[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    fn state() -> DevServerState {
        DevServerState::new(
            std::env::temp_dir(),
            Arc::new(AtomicBool::new(false)),
            None,
            false,
        )
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    fn response(message: DevSessionMessage) -> lingxia_control_protocol::ControlResponse {
        let DevSessionMessage::Response(response) = message else {
            panic!("expected a response");
        };
        response
    }

    #[test]
    fn png_size_reads_the_header() {
        assert_eq!(png_size(&png(1080, 2400)), Some((1080, 2400)));
        assert_eq!(png_size(b"not a png at all, not even close"), None);
    }

    #[test]
    fn a_capture_name_cannot_leave_the_run() {
        assert!(staged_path("run-1", "attachments/a%20b/attempt-1/device-1.png").is_ok());
        assert!(staged_path("run-1", "../device.png").is_err());
        assert!(staged_path("run-1", "/tmp/device.png").is_err());
        assert!(staged_path("run-1", "attachments/x/device.txt").is_err());
        assert!(staged_path("../run", "device.png").is_err());
        assert!(staged_path("..", "device.png").is_err());
    }

    #[test]
    fn a_desktop_target_has_no_device_capture() {
        let state = state();
        state.set_device_target(DeviceTarget::for_platform("macos"));
        let error = response(state.device_screenshot_reply(
            "1".into(),
            Some(serde_json::json!({ "run_id": "r1", "name": "attachments/s/device-1.png", "scale": 2.0 })),
        ))
        .error
        .expect("unsupported");
        assert_eq!(error.code, capture::UNSUPPORTED);
        assert!(error.message.contains("macos"), "{}", error.message);
    }

    #[test]
    fn a_put_capture_is_staged_and_listed_for_the_run() {
        let state = state();
        assert_eq!(
            state.staged_captures("r1").unwrap(),
            DeviceCaptures::default()
        );
        let args = serde_json::json!({ "run_id": "r1", "name": "attachments/s/attempt-1/app-1.png", "scale": 2.0 });
        let result = response(state.capture_put_reply("1".into(), Some(args), &png(2, 1)))
            .result
            .expect("staged");
        let capture: Capture = serde_json::from_value(result).unwrap();
        assert_eq!((capture.width, capture.height, capture.scale), (2, 1, 2.0));
        assert_eq!(capture.path, "attachments/s/attempt-1/app-1.png");

        let captures = state.staged_captures("r1").unwrap();
        assert_eq!(captures.files, ["attachments/s/attempt-1/app-1.png"]);
        let root = state.capture_root().unwrap();
        assert_eq!(
            Path::new(captures.dir.as_deref().unwrap()),
            root.join("r1").as_path()
        );
        assert!(state.staged_captures("../r1").is_err());
        assert!(state.staged_captures("r2").unwrap().files.is_empty());

        let refused = response(state.capture_put_reply(
            "2".into(),
            Some(serde_json::json!({ "run_id": "r1", "name": "x.png", "scale": 1.0 })),
            b"GIF89a",
        ));
        assert_eq!(refused.error.unwrap().code, capture::FAILED);
    }
}
