use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::ProjectConfig;
use crate::versions::LingXiaVersions;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

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
    vars.insert("PACKAGE_ID".to_string(), config.package_id.clone());

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

/// Remove icon references from AndroidManifest.xml when no icon is provided.
pub fn remove_android_icon_references(project_root: &Path) -> Result<()> {
    let manifest_path = project_root.join("android/app/src/main/AndroidManifest.xml");
    if !manifest_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&manifest_path)?;
    let Some(app_idx) = content.find("<application") else {
        return Ok(());
    };
    let Some(tag_end_rel) = find_xml_tag_end(&content[app_idx..]) else {
        return Ok(());
    };
    let tag_end = app_idx + tag_end_rel;

    let original_tag = &content[app_idx..=tag_end];
    let updated_tag = strip_xml_attr(original_tag, "android:icon");
    let updated_tag = strip_xml_attr(&updated_tag, "android:roundIcon");
    let updated = format!(
        "{}{}{}",
        &content[..app_idx],
        updated_tag,
        &content[tag_end + 1..]
    );

    fs::write(&manifest_path, updated)?;
    Ok(())
}

fn find_xml_tag_end(s: &str) -> Option<usize> {
    let mut in_quote: Option<char> = None;
    for (idx, ch) in s.char_indices() {
        match in_quote {
            Some(q) => {
                if ch == q {
                    in_quote = None;
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    in_quote = Some(ch);
                } else if ch == '>' {
                    return Some(idx);
                }
            }
        }
    }
    None
}

fn strip_xml_attr(tag: &str, attr: &str) -> String {
    let mut out = tag.to_string();
    let mut search_from = 0usize;

    loop {
        let Some(rel) = out.get(search_from..).and_then(|s| s.find(attr)) else {
            break;
        };
        let pos = search_from + rel;

        if pos == 0 {
            search_from = pos + attr.len();
            continue;
        }

        let bytes = out.as_bytes();
        if !bytes[pos - 1].is_ascii_whitespace() {
            search_from = pos + attr.len();
            continue;
        }

        // Start removal at the whitespace before attr.
        let mut start = pos - 1;
        while start > 0 && out.as_bytes()[start - 1].is_ascii_whitespace() {
            start -= 1;
        }

        // Scan forward to consume: attr [ws] = [ws] value
        let mut i = pos + attr.len();
        while i < out.len() && out.as_bytes()[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < out.len() && out.as_bytes()[i] == b'=' {
            i += 1;
            while i < out.len() && out.as_bytes()[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < out.len() && (out.as_bytes()[i] == b'"' || out.as_bytes()[i] == b'\'') {
                let quote = out.as_bytes()[i] as char;
                i += 1;
                while i < out.len() {
                    let ch = out.as_bytes()[i] as char;
                    if ch == quote {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            } else {
                while i < out.len()
                    && !out.as_bytes()[i].is_ascii_whitespace()
                    && out.as_bytes()[i] != b'>'
                {
                    i += 1;
                }
            }
        }

        out.replace_range(start..i, "");
        search_from = start;
    }

    out
}
