use super::super::doctor::{CheckResult, CheckStatus, command_exists, command_version_line};
use std::env;
use std::path::PathBuf;
use std::process::Command;

const ANDROID_CMDLINE_TOOLS_URL: &str = "https://developer.android.com/studio#command-tools";

pub fn doctor_checks() -> Vec<CheckResult> {
    let mut checks = Vec::new();
    checks.push(check_java());
    let sdk_check = check_android_sdk();
    let sdk_ready = sdk_check.status == CheckStatus::Pass;
    checks.push(sdk_check);
    if sdk_ready {
        checks.push(check_android_cmdline_tools());
        checks.push(check_android_platform_tools());
    }
    checks.push(check_gradle());
    checks.push(check_android_ndk());
    checks
}

fn check_java() -> CheckResult {
    match command_version_line("java", &["-version"], true) {
        Some(version) => CheckResult::pass("Java/JDK", format!("Found: {}", version)),
        None => CheckResult::fail(
            "Java/JDK",
            "Not found in PATH".to_string(),
            Some("Install JDK 17+ (https://adoptium.net/)"),
        ),
    }
}

fn check_android_sdk() -> CheckResult {
    match resolve_android_sdk_root() {
        Some(path) => {
            let sdk_path = PathBuf::from(&path);
            if sdk_path.exists() {
                CheckResult::pass("Android SDK", format!("ANDROID_SDK_ROOT set: {}", path))
            } else {
                CheckResult::fail(
                    "Android SDK",
                    format!("ANDROID_SDK_ROOT points to missing path: {}", path),
                    Some(android_sdk_install_hint()),
                )
            }
        }
        None => CheckResult::fail(
            "Android SDK",
            "Missing required env var: ANDROID_SDK_ROOT".to_string(),
            Some(android_sdk_install_hint()),
        ),
    }
}

fn check_android_cmdline_tools() -> CheckResult {
    if let Some(sdk_root) = resolve_android_sdk_root() {
        let sdkmanager = PathBuf::from(&sdk_root)
            .join("cmdline-tools")
            .join("latest")
            .join("bin")
            .join("sdkmanager");
        if sdkmanager.exists() || command_exists("sdkmanager") {
            let detail = if sdkmanager.exists() {
                format!("Found at: {}", sdkmanager.display())
            } else {
                "sdkmanager found in PATH".to_string()
            };
            return CheckResult::pass("Android cmdline-tools", detail);
        }
    }

    CheckResult::warn(
        "Android cmdline-tools",
        "sdkmanager not found".to_string(),
        Some(
            "Install command-line tools under $ANDROID_SDK_ROOT/cmdline-tools/latest \
(see Android SDK hint above)"
                .to_string(),
        ),
    )
}

fn check_android_platform_tools() -> CheckResult {
    if let Some(sdk_root) = resolve_android_sdk_root() {
        let adb_path = PathBuf::from(&sdk_root).join("platform-tools").join("adb");
        if adb_path.exists() || command_exists("adb") {
            let detail = if adb_path.exists() {
                format!("Found at: {}", adb_path.display())
            } else {
                "adb found in PATH".to_string()
            };
            return CheckResult::pass("Android platform-tools", detail);
        }
    }

    CheckResult::warn(
        "Android platform-tools",
        "adb not found".to_string(),
        Some("Install with sdkmanager: \"platform-tools\" (after SDK setup)".to_string()),
    )
}

fn check_gradle() -> CheckResult {
    let output = match Command::new("gradle").arg("--version").output() {
        Ok(output) => output,
        Err(_) => {
            return CheckResult::warn(
                "Gradle",
                "Not found in PATH".to_string(),
                Some("Optional when using Gradle wrapper (./gradlew)"),
            );
        }
    };

    if !output.status.success() {
        return CheckResult::warn(
            "Gradle",
            "Found but failed to query version".to_string(),
            Some("Optional when using Gradle wrapper (./gradlew)"),
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let version_line = stdout
        .lines()
        .find(|line| line.starts_with("Gradle"))
        .unwrap_or("Gradle (version unknown)");
    CheckResult::pass("Gradle", format!("Found: {}", version_line))
}

fn check_android_ndk() -> CheckResult {
    if let Ok(path) = env::var("ANDROID_NDK_ROOT") {
        let ndk_path = PathBuf::from(&path);
        if ndk_path.exists() {
            return CheckResult::pass("Android NDK", format!("ANDROID_NDK_ROOT set: {}", path));
        }
        return CheckResult::fail(
            "Android NDK",
            format!("ANDROID_NDK_ROOT points to missing path: {}", path),
            Some(android_ndk_install_hint()),
        );
    }

    CheckResult::fail(
        "Android NDK",
        "Missing required env var: ANDROID_NDK_ROOT".to_string(),
        Some(android_ndk_install_hint()),
    )
}

fn resolve_android_sdk_root() -> Option<String> {
    env::var("ANDROID_SDK_ROOT").ok()
}

fn android_sdk_install_hint() -> String {
    format!(
        "Download Android command-line tools: {}\n\
Set env var and install SDK tools:\n\
export ANDROID_SDK_ROOT=$HOME/android-sdk\n\
$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager --install \"build-tools;34.0.0\" \"platform-tools\" \"platforms;android-33\" \"ndk;28.2.13676358\"\n\
If permission is denied, retry with:\n\
sudo $ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager --install ...",
        ANDROID_CMDLINE_TOOLS_URL
    )
}

fn android_ndk_install_hint() -> String {
    "Set env var to an installed NDK directory, for example:\n\
export ANDROID_NDK_ROOT=$ANDROID_SDK_ROOT/ndk/28.2.13676358"
        .to_string()
}
