use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Process template directory recursively
pub fn process_template_dir(
    template_dir: &Path,
    target_dir: &Path,
    vars: &HashMap<String, String>,
) -> Result<()> {
    // Read all entries in the template directory
    let entries = fs::read_dir(template_dir)
        .context(format!("Failed to read template directory: {}", template_dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Skip build artifacts and cache directories
        if file_name_str == ".gradle"
            || file_name_str == "build"
            || file_name_str == ".idea"
            || file_name_str == "target" {
            continue;
        }

        if path.is_dir() {
            // Recursively process subdirectory
            let target_subdir = target_dir.join(&file_name);
            fs::create_dir_all(&target_subdir)?;
            process_template_dir(&path, &target_subdir, vars)?;
        } else if path.is_file() {
            // Check if file is binary or should be copied as-is
            let is_binary = is_binary_file(&path);

            let target_file = target_dir.join(&file_name);

            if is_binary {
                // Copy binary file as-is
                fs::copy(&path, &target_file)
                    .context(format!("Failed to copy binary file: {}", path.display()))?;
            } else {
                // Process template file
                let content = fs::read_to_string(&path)
                    .context(format!("Failed to read template file: {}", path.display()))?;

                let processed_content = substitute_variables(&content, vars);

                fs::write(&target_file, processed_content)
                    .context(format!("Failed to write file: {}", target_file.display()))?;
            }
        }
    }

    Ok(())
}

/// Check if a file should be treated as binary
fn is_binary_file(path: &Path) -> bool {
    // Check by extension
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy();
        if matches!(ext_str.as_ref(),
            "jar" | "so" | "a" | "dylib" | "db" |
            "png" | "jpg" | "jpeg" | "gif" | "ico" |
            "ttf" | "otf" | "woff" | "woff2" |
            "apk" | "aar" | "zip" | "tar" | "gz"
        ) {
            return true;
        }
    }

    // Check by file name
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if matches!(file_name,
        "gradlew" | "gradlew.bat" |
        ".lock" | "*.lock" |
        ".bin" | "*.bin"
    ) || file_name.ends_with(".lock") || file_name.ends_with(".bin") {
        return true;
    }

    false
}

/// Substitute template variables
fn substitute_variables(content: &str, vars: &HashMap<String, String>) -> String {
    let mut result = content.to_string();

    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }

    result
}
