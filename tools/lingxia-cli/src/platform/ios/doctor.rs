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

fn check_actool() -> CheckResult {
    match command_output_line("xcrun", &["--find", "actool"], false) {
        Some(path) => CheckResult::pass("actool", format!("Found: {}", path)),
        None => CheckResult::fail(
            "actool",
            "AssetCatalog compiler not found".to_string(),
            Some("Install full Xcode (not only minimal CLT)"),
        ),
    }
}
