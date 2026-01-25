use anyhow::Result;
use std::process::Command;

/// Execute the doctor command to check Android development environment
pub fn execute() -> Result<()> {
    println!("🔍 Checking Android development environment...\n");

    let mut all_checks_passed = true;

    // Check Java/JDK
    all_checks_passed &= check_java();

    // Check Android SDK
    all_checks_passed &= check_android_sdk();

    // Check Gradle
    all_checks_passed &= check_gradle();

    // Check Rust
    all_checks_passed &= check_rust();

    // Check Android NDK
    all_checks_passed &= check_android_ndk();

    println!("\n{}", "=".repeat(60));
    if all_checks_passed {
        println!("✅ All checks passed! Your Android development environment is ready.");
    } else {
        println!("⚠️  Some checks failed. Please install the missing tools.");
    }
    println!("{}", "=".repeat(60));

    Ok(())
}

/// Check if Java/JDK is installed
fn check_java() -> bool {
    print!("Checking Java/JDK... ");
    match Command::new("java").arg("-version").output() {
        Ok(output) => {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stderr);
                let version_line = version_output.lines().next().unwrap_or("unknown");
                println!("✅ Found: {}", version_line);
                true
            } else {
                println!("❌ Java found but failed to get version");
                false
            }
        }
        Err(_) => {
            println!("❌ Not found");
            println!("   Please install JDK 17 or later:");
            println!("   - Download from: https://adoptium.net/");
            false
        }
    }
}

/// Check if Android SDK is installed
fn check_android_sdk() -> bool {
    print!("Checking Android SDK... ");

    let android_home = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"));

    match android_home {
        Ok(path) => {
            if std::path::Path::new(&path).exists() {
                println!("✅ Found at: {}", path);

                // Check for platform-tools
                let adb_path = format!("{}/platform-tools/adb", path);
                if std::path::Path::new(&adb_path).exists() {
                    println!("   - platform-tools: ✅");
                } else {
                    println!("   - platform-tools: ⚠️  Not found (install via Android Studio SDK Manager)");
                }

                true
            } else {
                println!("⚠️  ANDROID_HOME set to '{}' but path doesn't exist", path);
                false
            }
        }
        Err(_) => {
            println!("❌ Not found (ANDROID_HOME not set)");
            println!("   Please install Android SDK:");
            println!("   - Install Android Studio from: https://developer.android.com/studio");
            println!("   - Set ANDROID_HOME environment variable to SDK location");
            false
        }
    }
}

/// Check if Gradle is installed
fn check_gradle() -> bool {
    print!("Checking Gradle... ");
    match Command::new("gradle").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version_output = String::from_utf8_lossy(&output.stdout);
                let version_line = version_output
                    .lines()
                    .find(|line| line.starts_with("Gradle"))
                    .unwrap_or("Gradle (version unknown)");
                println!("✅ Found: {}", version_line);
                true
            } else {
                println!("⚠️  Gradle found but failed to get version");
                println!("   (This is OK if your project uses Gradle wrapper)");
                true // Not critical if using wrapper
            }
        }
        Err(_) => {
            println!("⚠️  Not found in PATH");
            println!("   (This is OK if your project uses Gradle wrapper)");
            true // Not critical if using wrapper
        }
    }
}

/// Check if Rust is installed
fn check_rust() -> bool {
    print!("Checking Rust... ");
    match Command::new("rustc").arg("--version").output() {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("✅ {}", version.trim());
                true
            } else {
                println!("❌ rustc found but failed to get version");
                false
            }
        }
        Err(_) => {
            println!("❌ Not found");
            println!("   Please install Rust:");
            println!("   - Visit: https://rustup.rs/");
            false
        }
    }
}

/// Check if Android NDK is installed
fn check_android_ndk() -> bool {
    print!("Checking Android NDK... ");

    let ndk_home = std::env::var("ANDROID_NDK_HOME")
        .or_else(|_| std::env::var("NDK_HOME"));

    match ndk_home {
        Ok(path) => {
            if std::path::Path::new(&path).exists() {
                println!("✅ Found at: {}", path);
                true
            } else {
                println!("⚠️  NDK_HOME set to '{}' but path doesn't exist", path);
                false
            }
        }
        Err(_) => {
            // Try to find NDK in ANDROID_HOME
            if let Ok(android_home) = std::env::var("ANDROID_HOME") {
                let ndk_dir = format!("{}/ndk", android_home);
                if std::path::Path::new(&ndk_dir).exists() {
                    println!("✅ Found in ANDROID_HOME/ndk");
                    return true;
                }
            }

            println!("❌ Not found (ANDROID_NDK_HOME not set)");
            println!("   Please install Android NDK:");
            println!("   - Install via Android Studio SDK Manager");
            println!("   - Set ANDROID_NDK_HOME environment variable");
            false
        }
    }
}
