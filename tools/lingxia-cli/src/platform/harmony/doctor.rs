use super::deploy::resolve_command_path;
use crate::platform::doctor::{CheckResult, CheckStatus};
use std::env;
use std::path::PathBuf;

const HMOS_CMDLINE_TOOLS_URL: &str =
    "https://developer.huawei.com/consumer/en/download/command-line-tools-for-hmos";

pub fn doctor_checks() -> Vec<CheckResult> {
    let mut checks = Vec::new();
    let sdk_check = check_harmony_sdk_env();
    let sdk_ready = sdk_check.status == CheckStatus::Pass;
    checks.push(sdk_check);

    if sdk_ready {
        checks.push(check_harmony_command("ohpm", "ohpm package manager"));
        checks.push(check_harmony_command("hvigorw", "hvigorw build tool"));
        checks.push(check_harmony_command("hdc", "Harmony device bridge"));
    }

    checks
}

fn check_harmony_sdk_env() -> CheckResult {
    match env::var("OHOS_NDK_HOME") {
        Ok(path) => {
            let sdk_root = PathBuf::from(&path);
            if !sdk_root.exists() {
                return CheckResult::fail(
                    "Harmony command-line SDK",
                    format!("OHOS_NDK_HOME points to missing path: {path}"),
                    Some(harmony_sdk_setup_hint()),
                );
            }

            if !sdk_root.join("native").exists() {
                return CheckResult::fail(
                    "Harmony command-line SDK",
                    format!(
                        "OHOS_NDK_HOME set: {} (missing native/ directory)",
                        sdk_root.display()
                    ),
                    Some(
                        "Set OHOS_NDK_HOME to command-line-tools/sdk/default/openharmony"
                            .to_string(),
                    ),
                );
            }

            CheckResult::pass(
                "Harmony command-line SDK",
                format!("OHOS_NDK_HOME set: {}", sdk_root.display()),
            )
        }
        Err(_) => CheckResult::fail(
            "Harmony command-line SDK",
            "Missing required env var: OHOS_NDK_HOME".to_string(),
            Some(harmony_sdk_setup_hint()),
        ),
    }
}

fn check_harmony_command(cmd: &str, display_name: &str) -> CheckResult {
    if let Some(path) = resolve_command_path(cmd) {
        CheckResult::pass(display_name, format!("Found at: {}", path.display()))
    } else {
        CheckResult::fail(
            display_name,
            format!("'{}' not found", cmd),
            Some(format!(
                "Install Harmony command-line tools and set OHOS_NDK_HOME \
(or make sure '{}' is in PATH).\n\
This check also auto-resolves tools under OHOS_NDK_HOME.",
                cmd
            )),
        )
    }
}

fn harmony_sdk_setup_hint() -> String {
    format!(
        "Download Harmony command-line tools: {HMOS_CMDLINE_TOOLS_URL}\n\
Set environment variable to SDK root, for example:\n\
export OHOS_NDK_HOME=$HOME/OpenHarmony/command-line-tools/sdk/default/openharmony"
    )
}
