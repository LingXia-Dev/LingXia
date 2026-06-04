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
    let entries = fs::read_dir(template_dir).context(format!(
        "Failed to read template directory: {}",
        template_dir.display()
    ))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        let output_name = if file_name_str == "gitignore" {
            ".gitignore".to_string()
        } else {
            file_name_str.to_string()
        };

        // Skip build artifacts and cache directories
        if file_name_str == ".gradle"
            || file_name_str == "build"
            || file_name_str == ".idea"
            || file_name_str == "target"
        {
            continue;
        }

        // Skip framework-specific files that don't match the selected framework.
        // .vue files → Vue only; .tsx files → React only.
        if let Some(framework) = vars.get("FRAMEWORK").map(|s| s.as_str())
            && let Some(ext) = path.extension().and_then(|e| e.to_str())
        {
            if ext == "vue" && framework != "vue" {
                continue;
            }
            if ext == "tsx" && framework != "react" {
                continue;
            }
        }

        if path.is_dir() {
            // Recursively process subdirectory
            let target_subdir = target_dir.join(output_name.as_str());
            fs::create_dir_all(&target_subdir)?;
            process_template_dir(&path, &target_subdir, vars)?;
        } else if path.is_file() {
            // Check if file is binary or should be copied as-is
            let is_binary = is_binary_file(&path);

            let target_file = target_dir.join(output_name.as_str());

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
        if matches!(
            ext_str.as_ref(),
            "jar"
                | "so"
                | "a"
                | "dylib"
                | "db"
                | "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "ico"
                | "ttf"
                | "otf"
                | "woff"
                | "woff2"
                | "apk"
                | "aar"
                | "zip"
                | "tar"
                | "gz"
        ) {
            return true;
        }
    }

    // Check by file name
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if matches!(
        file_name,
        "gradlew" | "gradlew.bat" | ".lock" | "*.lock" | ".bin" | "*.bin"
    ) || file_name.ends_with(".lock")
        || file_name.ends_with(".bin")
    {
        return true;
    }

    false
}

/// Substitute template variables
pub(super) fn substitute_variables(content: &str, vars: &HashMap<String, String>) -> String {
    let mut result = content.to_string();

    for (key, value) in vars {
        // Support both `{{KEY}}` and `__KEY__` placeholder styles.
        result = result.replace(&format!("{{{{{}}}}}", key), value);
        result = result.replace(&format!("__{}__", key), value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // --- substitute_variables ---

    #[test]
    fn substitutes_double_brace_placeholders() {
        let mut vars = HashMap::new();
        vars.insert("NAME".to_string(), "Alice".to_string());
        assert_eq!(
            substitute_variables("Hello, {{NAME}}!", &vars),
            "Hello, Alice!"
        );
    }

    #[test]
    fn substitutes_underscore_placeholders() {
        let mut vars = HashMap::new();
        vars.insert("NAME".to_string(), "Alice".to_string());
        assert_eq!(
            substitute_variables("Hello, __NAME__!", &vars),
            "Hello, Alice!"
        );
    }

    #[test]
    fn leaves_unknown_placeholder_unchanged() {
        let vars = HashMap::new();
        assert_eq!(substitute_variables("{{MISSING}}", &vars), "{{MISSING}}");
    }

    #[test]
    fn substitutes_multiple_vars_in_one_string() {
        let mut vars = HashMap::new();
        vars.insert("A".to_string(), "foo".to_string());
        vars.insert("B".to_string(), "bar".to_string());
        assert_eq!(substitute_variables("{{A}}-{{B}}", &vars), "foo-bar");
    }

    // --- process_template_dir: gitignore rename ---

    #[test]
    fn renames_gitignore_to_dotgitignore() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("gitignore"), "node_modules/").unwrap();
        process_template_dir(src.path(), dst.path(), &HashMap::new()).unwrap();
        assert!(
            dst.path().join(".gitignore").exists(),
            ".gitignore must exist"
        );
        assert!(
            !dst.path().join("gitignore").exists(),
            "bare gitignore must not remain"
        );
    }

    // --- process_template_dir: variable substitution in files ---

    #[test]
    fn substitutes_variables_in_file_content() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("config.json"), r#"{"name":"{{APP_NAME}}"}"#).unwrap();
        let mut vars = HashMap::new();
        vars.insert("APP_NAME".to_string(), "myapp".to_string());
        process_template_dir(src.path(), dst.path(), &vars).unwrap();
        let content = fs::read_to_string(dst.path().join("config.json")).unwrap();
        assert_eq!(content, r#"{"name":"myapp"}"#);
    }

    // --- process_template_dir: framework file filtering ---

    #[test]
    fn react_framework_skips_vue_files() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let pages = src.path().join("pages");
        fs::create_dir_all(&pages).unwrap();
        fs::write(pages.join("index.tsx"), "react page").unwrap();
        fs::write(pages.join("index.vue"), "vue page").unwrap();
        fs::write(src.path().join("util.ts"), "shared").unwrap();

        let mut vars = HashMap::new();
        vars.insert("FRAMEWORK".to_string(), "react".to_string());
        process_template_dir(src.path(), dst.path(), &vars).unwrap();

        assert!(
            dst.path().join("pages/index.tsx").exists(),
            "tsx must be copied"
        );
        assert!(
            !dst.path().join("pages/index.vue").exists(),
            "vue must be skipped"
        );
        assert!(
            dst.path().join("util.ts").exists(),
            "shared .ts must be copied"
        );
    }

    #[test]
    fn vue_framework_skips_tsx_files() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let pages = src.path().join("pages");
        fs::create_dir_all(&pages).unwrap();
        fs::write(pages.join("index.tsx"), "react page").unwrap();
        fs::write(pages.join("index.vue"), "vue page").unwrap();
        fs::write(src.path().join("util.ts"), "shared").unwrap();

        let mut vars = HashMap::new();
        vars.insert("FRAMEWORK".to_string(), "vue".to_string());
        process_template_dir(src.path(), dst.path(), &vars).unwrap();

        assert!(
            dst.path().join("pages/index.vue").exists(),
            "vue must be copied"
        );
        assert!(
            !dst.path().join("pages/index.tsx").exists(),
            "tsx must be skipped"
        );
        assert!(
            dst.path().join("util.ts").exists(),
            "shared .ts must be copied"
        );
    }

    #[test]
    fn no_framework_var_copies_all_files() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        fs::write(src.path().join("index.tsx"), "react").unwrap();
        fs::write(src.path().join("index.vue"), "vue").unwrap();
        process_template_dir(src.path(), dst.path(), &HashMap::new()).unwrap();
        assert!(dst.path().join("index.tsx").exists());
        assert!(dst.path().join("index.vue").exists());
    }

    // --- process_template_dir: subdirectory recursion ---

    #[test]
    fn recursively_processes_subdirectories() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        let sub = src.path().join("a").join("b");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("file.ts"), "{{X}}").unwrap();

        let mut vars = HashMap::new();
        vars.insert("X".to_string(), "ok".to_string());
        process_template_dir(src.path(), dst.path(), &vars).unwrap();

        let content = fs::read_to_string(dst.path().join("a/b/file.ts")).unwrap();
        assert_eq!(content, "ok");
    }
}
