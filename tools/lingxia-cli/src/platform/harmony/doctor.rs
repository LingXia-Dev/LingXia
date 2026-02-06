use crate::platform::doctor::{CheckResult, command_exists};
use std::env;
use std::path::PathBuf;

pub fn doctor_checks() -> Vec<CheckResult> {
    vec![
        check_ohos_ndk_home(),
        check_harmony_command("ohpm", "ohpm package manager"),
        check_harmony_command("hvigorw", "hvigorw build tool"),
        check_harmony_command("hdc", "Harmony device bridge"),
    ]
}

fn check_ohos_ndk_home() -> CheckResult {
    match env::var("OHOS_NDK_HOME") {
        Ok(path) => {
            let ndk = PathBuf::from(&path);
            if ndk.exists() {
                CheckResult::pass("OHOS_NDK_HOME", format!("Found at: {}", ndk.display()))
            } else {
                CheckResult::fail(
                    "OHOS_NDK_HOME",
                    format!("Path does not exist: {}", path),
                    Some("Set OHOS_NDK_HOME to your Harmony SDK root"),
                )
            }
        }
        Err(_) => CheckResult::fail(
            "OHOS_NDK_HOME",
            "Environment variable is not set".to_string(),
            Some("Example: export OHOS_NDK_HOME=/path/to/ohos-sdk"),
        ),
    }
}

fn check_harmony_command(cmd: &str, display_name: &str) -> CheckResult {
    if command_exists(cmd) {
        CheckResult::pass(display_name, format!("Found: {}", cmd))
    } else {
        CheckResult::fail(
            display_name,
            format!("'{}' not found in PATH", cmd),
            Some("Install DevEco Studio command-line tools and add them to PATH"),
        )
    }
}
