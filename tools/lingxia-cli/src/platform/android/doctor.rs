use super::super::doctor::{CheckResult, command_exists, command_version_line};
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub fn doctor_checks() -> Vec<CheckResult> {
    let mut checks = Vec::new();
    checks.push(check_java());
    checks.extend(check_android_sdk_checks());
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

fn check_android_sdk_checks() -> Vec<CheckResult> {
    let mut checks = Vec::new();
    let android_home = env::var("ANDROID_HOME").or_else(|_| env::var("ANDROID_SDK_ROOT"));

    match android_home {
        Ok(path) => {
            let sdk_path = PathBuf::from(&path);
            if sdk_path.exists() {
                checks.push(CheckResult::pass(
                    "Android SDK",
                    format!("Found at: {}", sdk_path.display()),
                ));

                let adb_path = sdk_path.join("platform-tools").join("adb");
                if adb_path.exists() || command_exists("adb") {
                    let detail = if adb_path.exists() {
                        format!("Found at: {}", adb_path.display())
                    } else {
                        "Found in PATH".to_string()
                    };
                    checks.push(CheckResult::pass("Android platform-tools", detail));
                } else {
                    checks.push(CheckResult::warn(
                        "Android platform-tools",
                        "adb not found".to_string(),
                        Some("Install platform-tools via Android Studio SDK Manager"),
                    ));
                }
            } else {
                checks.push(CheckResult::fail(
                    "Android SDK",
                    format!(
                        "ANDROID_HOME/ANDROID_SDK_ROOT points to missing path: {}",
                        path
                    ),
                    Some("Install Android SDK and set ANDROID_HOME or ANDROID_SDK_ROOT"),
                ));
            }
        }
        Err(_) => checks.push(CheckResult::fail(
            "Android SDK",
            "ANDROID_HOME/ANDROID_SDK_ROOT not set".to_string(),
            Some("Install Android Studio and set ANDROID_HOME"),
        )),
    }

    checks
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
    if let Ok(path) = env::var("ANDROID_NDK_HOME").or_else(|_| env::var("NDK_HOME")) {
        let ndk_path = PathBuf::from(&path);
        if ndk_path.exists() {
            return CheckResult::pass("Android NDK", format!("Found at: {}", ndk_path.display()));
        }
        return CheckResult::fail(
            "Android NDK",
            format!("ANDROID_NDK_HOME/NDK_HOME points to missing path: {}", path),
            Some("Install Android NDK and fix ANDROID_NDK_HOME"),
        );
    }

    if let Ok(android_home) = env::var("ANDROID_HOME").or_else(|_| env::var("ANDROID_SDK_ROOT")) {
        let ndk_dir = PathBuf::from(android_home).join("ndk");
        if ndk_dir.exists() {
            return CheckResult::pass("Android NDK", format!("Found under: {}", ndk_dir.display()));
        }
    }

    CheckResult::fail(
        "Android NDK",
        "Not found".to_string(),
        Some("Install Android NDK via SDK Manager and set ANDROID_NDK_HOME"),
    )
}
