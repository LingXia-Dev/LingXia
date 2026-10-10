use super::super::doctor::{
    CheckResult, command_exists, command_output_line, command_version_line,
};
use super::apple;

pub fn doctor_checks() -> Vec<CheckResult> {
    if !apple::is_macos() {
        return vec![CheckResult::fail(
            "Host OS",
            format!(
                "macOS builds are only supported on macOS (current: {})",
                std::env::consts::OS
            ),
            None::<String>,
        )];
    }

    vec![
        check_xcode_select(),
        check_swift(),
        check_hdiutil(),
        check_actool(),
        check_osascript(),
    ]
}

fn check_xcode_select() -> CheckResult {
    match apple::active_developer_dir() {
        Some(path) if apple::is_full_xcode(&path) => {
            CheckResult::pass("Xcode", format!("Active path: {}", path))
        }
        Some(path) => CheckResult::warn(
            "Xcode",
            format!("Active path {path} is not a full Xcode; apps build without an icon"),
            Some(apple::select_xcode_hint()),
        ),
        None => CheckResult::fail(
            "Xcode",
            "xcode-select not configured".to_string(),
            Some(apple::select_xcode_hint()),
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

fn check_hdiutil() -> CheckResult {
    if command_exists("hdiutil") {
        CheckResult::pass("hdiutil", "Available".to_string())
    } else {
        CheckResult::fail(
            "hdiutil",
            "Not found in PATH".to_string(),
            Some("hdiutil is required for --dmg packaging"),
        )
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

fn check_osascript() -> CheckResult {
    if command_exists("osascript") {
        CheckResult::pass(
            "osascript",
            "Available (used for DMG Finder layout)".to_string(),
        )
    } else {
        CheckResult::warn(
            "osascript",
            "Not found; DMG creation still works but Finder layout customization may fail"
                .to_string(),
            None::<String>,
        )
    }
}
