use super::android;
use super::harmony;
use super::ios;
use super::locate_templates_dir;
use super::macos;
use super::template::process_template_dir;
use super::types::{AppServiceMode, Platform, ProjectConfig};
use crate::versions::LingXiaVersions;
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;

pub(super) fn create_project(config: &ProjectConfig, versions: &LingXiaVersions) -> Result<()> {
    if config.target_dir.exists() {
        return Err(anyhow!(
            "Directory '{}' already exists",
            config.target_dir.display()
        ));
    }

    println!();
    println!("{}", "Creating project structure...".bold());

    fs::create_dir_all(&config.target_dir)?;
    create_root_gitignore(config)?;

    let mut created_any = false;
    for platform in &config.platforms {
        match platform {
            Platform::Android => {
                android::create_android_project(config, versions)?;
                created_any = true;
            }
            Platform::Ios => {
                ios::create_ios_project(config)?;
                created_any = true;
            }
            Platform::Macos => {
                macos::create_macos_project(config)?;
                created_any = true;
            }
            Platform::Harmony => {
                harmony::create_harmony_project(config)?;
                created_any = true;
            }
        }
    }

    if !created_any {
        return Err(anyhow!("No platforms selected"));
    }

    Ok(())
}

fn create_root_gitignore(config: &ProjectConfig) -> Result<()> {
    let mut lines: Vec<&str> = vec!["# LingXia generated", ".lingxia/", "target/"];

    if config.platforms.contains(&Platform::Android) {
        lines.extend([
            "",
            "# Android generated",
            "android/.gradle/",
            "android/build/",
            "android/app/build/",
            "android/app/src/main/assets/",
            "android/app/src/main/jniLibs/",
        ]);
    }

    if config.platforms.contains(&Platform::Harmony) {
        lines.extend([
            "",
            "# Harmony generated",
            "harmony/.hvigor/",
            "harmony/build/",
            "harmony/entry/build/",
            "harmony/entry/.preview/",
            "harmony/entry/src/main/resources/rawfile/",
        ]);
    }

    let mut content = lines.join("\n");
    content.push('\n');
    fs::write(config.target_dir.join(".gitignore"), content)?;
    Ok(())
}

pub(super) fn create_rust_library(
    config: &ProjectConfig,
    versions: &LingXiaVersions,
    app_service: AppServiceMode,
) -> Result<()> {
    let project_root = &config.target_dir;
    let lib_name = format!("{}-lib", config.name);
    let lib_dir = project_root.join(&lib_name);

    // Create library directory
    fs::create_dir_all(&lib_dir)?;

    // Locate templates directory
    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("rust-lib");

    if !template_dir.exists() {
        return Err(anyhow!(
            "Rust library template not found at: {}",
            template_dir.display()
        ));
    }

    // Build variable substitution map
    let mut vars = HashMap::new();
    vars.insert("PROJECT_NAME".to_string(), lib_name.clone());
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());

    // Convert package ID to underscore format for JNI function names
    // e.g., com.example.mouke -> com_example_mouke
    let package_id_underscore = config.package_id.replace('.', "_");
    vars.insert("PACKAGE_ID_UNDERSCORE".to_string(), package_id_underscore);

    vars.insert(
        "LINGXIA_VERSION".to_string(),
        versions.lingxia_crate.clone(),
    );
    vars.insert(
        "HOST_DEFAULT_FEATURES".to_string(),
        match app_service {
            AppServiceMode::Enabled => "[\"standard\"]".to_string(),
            AppServiceMode::Disabled => "[]".to_string(),
        },
    );
    vars.insert("LINGXIA_DEP_FEATURES".to_string(), "[]".to_string());
    vars.insert(
        "APPLE_REEXPORT".to_string(),
        if config.platforms.contains(&Platform::Ios) || config.platforms.contains(&Platform::Macos)
        {
            "#[cfg(any(target_os = \"ios\", target_os = \"macos\"))]\npub use lingxia::apple::*;"
                .to_string()
        } else {
            String::new()
        },
    );
    vars.insert(
        "HARMONY_REEXPORT".to_string(),
        if config.platforms.contains(&Platform::Harmony) {
            "#[cfg(target_env = \"ohos\")]\npub use lingxia::harmony::*;".to_string()
        } else {
            String::new()
        },
    );
    vars.insert(
        "ANDROID_EXPORT_BLOCK".to_string(),
        if config.platforms.contains(&Platform::Android) {
            format!(
                "// Android: JNI export\n#[cfg(target_os = \"android\")]\nmod android {{\n    use jni::EnvUnowned;\n    use jni::objects::JClass;\n\n    #[unsafe(no_mangle)]\n    pub extern \"system\" fn Java_{}_MainActivity_nativeRegisterHostAddon(\n        _env: EnvUnowned,\n        _class: JClass,\n    ) {{\n        super::register_host_addons();\n    }}\n}}",
                config.package_id.replace('.', "_")
            )
        } else {
            String::new()
        },
    );
    vars.insert(
        "HARMONY_EXPORT_BLOCK".to_string(),
        if config.platforms.contains(&Platform::Harmony) {
            "// Harmony: NAPI export\n#[cfg(target_env = \"ohos\")]\n#[napi_derive_ohos::napi]\npub fn lingxia_register_host_addon() {\n    register_host_addons();\n}".to_string()
        } else {
            String::new()
        },
    );
    vars.insert(
        "APPLE_EXPORT_BLOCK".to_string(),
        if config.platforms.contains(&Platform::Ios) || config.platforms.contains(&Platform::Macos)
        {
            "// iOS/macOS: C export\n#[cfg(any(target_os = \"ios\", target_os = \"macos\"))]\n#[unsafe(no_mangle)]\npub extern \"C\" fn lingxia_register_host_addon() {\n    register_host_addons();\n}".to_string()
        } else {
            String::new()
        },
    );
    vars.insert(
        "ANDROID_DEPS_BLOCK".to_string(),
        if config.platforms.contains(&Platform::Android) {
            "[target.'cfg(target_os = \"android\")'.dependencies]\njni = \"0.22.1\"".to_string()
        } else {
            String::new()
        },
    );
    vars.insert(
        "HARMONY_DEPS_BLOCK".to_string(),
        if config.platforms.contains(&Platform::Harmony) {
            "[target.'cfg(target_env = \"ohos\")'.dependencies]\nnapi-ohos = \"1.1\"\nnapi-derive-ohos = \"1.1\"".to_string()
        } else {
            String::new()
        },
    );

    // Process all template files into {project}-lib/ directory
    process_template_dir(&template_dir, &lib_dir, &vars)?;

    println!("  Created Rust library: {}", lib_name);

    Ok(())
}
