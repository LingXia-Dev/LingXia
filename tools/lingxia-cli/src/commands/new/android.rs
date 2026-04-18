use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::ProjectConfig;
use crate::versions::LingXiaVersions;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;

pub fn create_android_project(config: &ProjectConfig, versions: &LingXiaVersions) -> Result<()> {
    let project_root = &config.target_dir;

    // Create root directory
    fs::create_dir_all(project_root)?;

    // Create android subdirectory
    let android_dir = project_root.join("android");
    fs::create_dir_all(&android_dir)?;

    // Locate templates directory
    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("android-native");

    if !template_dir.exists() {
        return Err(anyhow!(
            "Android template not found at: {}",
            template_dir.display()
        ));
    }

    // Build variable substitution map
    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), config.name.clone());
    vars.insert("PRODUCT_NAME".to_string(), config.product_name.clone());
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());
    vars.insert(
        "APP_LINK_INTENT_FILTERS".to_string(),
        render_android_app_link_intent_filters(&config.app_link_hosts),
    );

    // Add SDK version variables
    vars.insert("MIN_SDK".to_string(), "29".to_string());
    vars.insert("TARGET_SDK".to_string(), "35".to_string());
    vars.insert("COMPILE_SDK".to_string(), "35".to_string());
    vars.insert("SDK_VERSION".to_string(), versions.sdk.clone());

    // Process all template files into android/ subdirectory
    process_template_dir(&template_dir, &android_dir, &vars)?;

    // Special handling: Create package directory structure for MainActivity.kt
    let package_path = config.package_id.replace('.', "/");
    let kotlin_dir = android_dir.join(format!("app/src/main/java/{}", package_path));
    fs::create_dir_all(&kotlin_dir)?;

    // Move MainActivity.kt to the correct package directory
    let temp_main_activity = android_dir.join("app/src/main/java/MainActivity.kt");
    if temp_main_activity.exists() {
        let target_main_activity = kotlin_dir.join("MainActivity.kt");
        fs::rename(&temp_main_activity, &target_main_activity)?;
    }

    println!("  Created Android project structure");
    Ok(())
}

fn render_android_app_link_intent_filters(hosts: &[String]) -> String {
    if hosts.is_empty() {
        return String::new();
    }
    let filters = hosts
        .iter()
        .map(|host| {
            format!(
                r#"            <intent-filter android:autoVerify="true">
                <action android:name="android.intent.action.VIEW" />
                <category android:name="android.intent.category.DEFAULT" />
                <category android:name="android.intent.category.BROWSABLE" />
                <data android:scheme="https" android:host="{host}" />
            </intent-filter>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!(
        "            <!-- LingXia AppLinks BEGIN -->\n{filters}\n            <!-- LingXia AppLinks END -->"
    )
}
