use super::super::doctor::{
    CheckResult, command_exists, command_output_line, command_version_line,
};
use super::apple;

pub fn doctor_checks() -> Vec<CheckResult> {
    if !apple::is_macos() {
        return vec![CheckResult::fail(
            "Host OS",
            format!(
                "iOS builds are only supported on macOS (current: {})",
                std::env::consts::OS
            ),
            None::<String>,
        )];
    }

    vec![
        check_xcode_select(),
        check_swift(),
        check_codesign(),
        check_iphoneos_sdk(),
        check_actool(),
    ]
}

fn check_xcode_select() -> CheckResult {
    match command_output_line("xcode-select", &["-p"], false) {
        Some(path) => {
            CheckResult::pass("Xcode Command Line Tools", format!("Active path: {}", path))
        }
        None => CheckResult::fail(
            "Xcode Command Line Tools",
            "xcode-select not configured".to_string(),
            Some("Install Xcode and run: sudo xcode-select -s /Applications/Xcode.app"),
        ),
    }
}

fn check_swift() -> CheckResult {
    match command_version_line("swift", &["--version"], false) {
        Some(version) => CheckResult::pass("Swift", version),
        None => CheckResult::fail(
            "Swift",
            "swift not found in PATH".to_string(),
            Some("Install Xcode and Xcode Command Line Tools"),
        ),
    }
}

fn check_codesign() -> CheckResult {
    if command_exists("codesign") {
        CheckResult::pass("codesign", "Available".to_string())
    } else {
        CheckResult::fail(
            "codesign",
            "codesign not found in PATH".to_string(),
            Some("Install Xcode Command Line Tools"),
        )
    }
}

fn check_iphoneos_sdk() -> CheckResult {
    match command_output_line("xcrun", &["--sdk", "iphoneos", "--show-sdk-path"], false) {
        Some(path) => CheckResult::pass("iPhoneOS SDK", format!("Found: {}", path)),
        None => CheckResult::fail(
            "iPhoneOS SDK",
            "Unable to locate SDK via xcrun".to_string(),
            Some("Open Xcode once and ensure iOS platform support is installed"),
        ),
    }
}

/// Compile a throwaway catalog rather than just locating the binary: `actool`
/// is present but unusable when the iOS platform files are missing, and it
/// signals that by failing at compile time. Trusting `--find` alone lets a
/// build ship with no app icon and a white launch frame.
fn check_actool() -> CheckResult {
    let Some(path) = command_output_line("xcrun", &["--find", "actool"], false) else {
        return CheckResult::fail(
            "actool",
            "AssetCatalog compiler not found".to_string(),
            Some("Install full Xcode (not only minimal CLT)"),
        );
    };

    match try_compile_probe_catalog() {
        Ok(()) => CheckResult::pass("actool", format!("Found and usable: {}", path)),
        Err(err) => CheckResult::fail(
            "actool",
            format!("Found but cannot compile asset catalogs: {err}"),
            Some(
                "App icons and the splash launch frame will be missing. \
                 Run `xcodebuild -downloadPlatform iOS` to install the platform files.",
            ),
        ),
    }
}

/// Compile a one-color catalog into a scratch dir; success means a real build
/// will produce `Assets.car`.
fn try_compile_probe_catalog() -> Result<(), String> {
    use std::fs;

    let root = std::env::temp_dir().join(format!("lingxia-actool-probe-{}", std::process::id()));
    let colorset = root.join("Probe.xcassets/ProbeColor.colorset");
    let out = root.join("out");
    let cleanup = || {
        let _ = fs::remove_dir_all(&root);
    };

    let write = || -> std::io::Result<()> {
        fs::create_dir_all(&colorset)?;
        fs::create_dir_all(&out)?;
        fs::write(
            colorset.join("Contents.json"),
            r#"{"colors":[{"idiom":"universal","color":{"color-space":"srgb","components":{"red":"0x00","green":"0x00","blue":"0x00","alpha":"1.000"}}}],"info":{"author":"lingxia","version":1}}"#,
        )
    };
    if let Err(err) = write() {
        cleanup();
        return Err(format!("could not stage a probe catalog: {err}"));
    }

    let output = std::process::Command::new("xcrun")
        .args(["--sdk", "iphoneos", "actool"])
        .env_remove("SDKROOT")
        .args([
            "--output-format",
            "human-readable-text",
            "--output-partial-info-plist",
            &root.join("probe.plist").to_string_lossy(),
            "--minimum-deployment-target",
            "17.0",
            "--platform",
            "iphoneos",
            "--target-device",
            "iphone",
            "--compile",
            &out.to_string_lossy(),
            &root.join("Probe.xcassets").to_string_lossy(),
        ])
        .output();

    let result = match output {
        Ok(output) if out.join("Assets.car").is_file() => {
            let _ = output;
            Ok(())
        }
        Ok(output) => {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            Err(text
                .lines()
                .find(|line| line.contains("error:"))
                .map(|line| line.trim().to_string())
                .unwrap_or_else(|| "no Assets.car produced".to_string()))
        }
        Err(err) => Err(err.to_string()),
    };

    cleanup();
    result
}
