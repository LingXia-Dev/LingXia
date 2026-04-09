use super::icons;
use super::locate_templates_dir;
use super::template::process_template_dir;
use super::types::{LxAppInfo, ProjectConfig};
use crate::versions::LingXiaVersions;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub(super) fn create_lxapp_from_template(
    target_dir: &Path,
    project_name: &str,
    product_name: &str,
    framework: &str,
    versions: &LingXiaVersions,
    lingxia_bridge_version: &str,
    lingxia_types_version: &str,
) -> Result<()> {
    if target_dir.exists() {
        return Err(anyhow!(
            "Directory '{}' already exists",
            target_dir.display()
        ));
    }

    fs::create_dir_all(target_dir)?;

    let templates_base = locate_templates_dir()?;
    let template_dir = templates_base.join("lxapp-create");
    if !template_dir.exists() {
        return Err(anyhow!(
            "LxApp template not found at: {}",
            template_dir.display()
        ));
    }

    let vars = build_framework_vars(
        framework,
        project_name,
        product_name,
        versions,
        lingxia_bridge_version,
        lingxia_types_version,
    )?;

    process_template_dir(&template_dir, target_dir, &vars)?;
    icons::ensure_lxapp_public_icon(target_dir)?;

    Ok(())
}

/// Build the template variable map for the given framework.
/// Extracted so it can be tested independently of the filesystem.
pub(super) fn build_framework_vars(
    framework: &str,
    project_name: &str,
    product_name: &str,
    versions: &LingXiaVersions,
    lingxia_bridge_version: &str,
    lingxia_types_version: &str,
) -> Result<HashMap<String, String>> {
    let fw = framework.to_lowercase();
    let slug = slugify(project_name);
    let mut vars = HashMap::new();

    vars.insert("APP_PACKAGE_NAME".to_string(), slug.clone());
    vars.insert("APP_ID".to_string(), slug);
    vars.insert("APP_DISPLAY_NAME".to_string(), product_name.to_string());
    vars.insert("RONG_VERSION".to_string(), versions.rong.clone());
    vars.insert(
        "LINGXIA_BRIDGE_VERSION".to_string(),
        lingxia_bridge_version.to_string(),
    );
    vars.insert(
        "LINGXIA_TYPES_VERSION".to_string(),
        lingxia_types_version.to_string(),
    );

    let (
        fw_display,
        fw_pkg,
        fw_page_ext,
        fw_jsx_mode,
        fw_tsconfig_include,
        fw_app_root_selector,
        fw_runtime_deps,
        fw_dev_deps_prefix,
        fw_vite_dev_deps,
    ) = match fw.as_str() {
        "react" => (
            "React",
            "@lingxia/page-runtime",
            "tsx",
            "react-jsx",
            r#""**/*.ts", "**/*.tsx", ".lingxia/types/**/*.d.ts""#,
            "#root",
            "\"react\": \"^19.2.4\",\n    \"react-dom\": \"^19.2.4\"",
            "\"@types/react\": \"^19.2.10\",\n    \"@types/react-dom\": \"^19.2.3\",\n    ",
            "\"@vitejs/plugin-react\": \"^6.0.1\",\n    \"esbuild\": \"^0.27.0\",\n    \"vite\": \"^8.0.0\",\n    ",
        ),
        "vue" => (
            "Vue",
            "@lingxia/page-runtime",
            "vue",
            "preserve",
            r#""**/*.ts", "**/*.tsx", "**/*.vue", ".lingxia/types/**/*.d.ts""#,
            "#app",
            "\"vue\": \"^3.5.0\"",
            "\"vue-tsc\": \"^3.2.4\",\n    ",
            "\"@vitejs/plugin-vue\": \"^6.0.5\",\n    \"esbuild\": \"^0.27.0\",\n    \"vite\": \"^8.0.0\",\n    ",
        ),
        other => {
            return Err(anyhow!(
                "Unsupported framework: {other}. Use 'react' or 'vue'."
            ));
        }
    };

    vars.insert("FRAMEWORK".to_string(), fw);
    vars.insert("FRAMEWORK_DISPLAY".to_string(), fw_display.to_string());
    vars.insert("FRAMEWORK_PKG".to_string(), fw_pkg.to_string());
    vars.insert("PAGE_EXT".to_string(), fw_page_ext.to_string());
    vars.insert("JSX_MODE".to_string(), fw_jsx_mode.to_string());
    vars.insert(
        "TSCONFIG_INCLUDE".to_string(),
        fw_tsconfig_include.to_string(),
    );
    vars.insert(
        "APP_ROOT_SELECTOR".to_string(),
        fw_app_root_selector.to_string(),
    );
    vars.insert(
        "FRAMEWORK_RUNTIME_DEPS".to_string(),
        fw_runtime_deps.to_string(),
    );
    vars.insert(
        "FRAMEWORK_DEV_DEPS_PREFIX".to_string(),
        fw_dev_deps_prefix.to_string(),
    );
    vars.insert(
        "FRAMEWORK_VITE_DEV_DEPS".to_string(),
        fw_vite_dev_deps.to_string(),
    );

    Ok(vars)
}

pub(super) fn create_lxapp_project(
    config: &ProjectConfig,
    lxapp_dir_name: &str,
    framework: &str,
    versions: &LingXiaVersions,
    lingxia_bridge_version: &str,
    lingxia_types_version: &str,
) -> Result<LxAppInfo> {
    let lxapp_dir_name = lxapp_dir_name.trim();
    let lxapp_dir = config.target_dir.join(lxapp_dir_name);
    println!("  Creating LxApp project...");
    create_lxapp_from_template(
        &lxapp_dir,
        lxapp_dir_name,
        &config.product_name,
        framework,
        versions,
        lingxia_bridge_version,
        lingxia_types_version,
    )?;
    Ok(LxAppInfo {
        app_id: lxapp_dir_name.to_string(),
    })
}

pub(super) fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = false;

    for ch in value.trim().chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }

    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "lingxia-app".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versions::LingXiaVersions;
    use std::fs;
    use tempfile::tempdir;

    fn dummy_versions() -> LingXiaVersions {
        LingXiaVersions {
            sdk: "0.1.0".to_string(),
            rong: "0.1.0".to_string(),
            lingxia_crate: "0.1.0".to_string(),
        }
    }

    /// Creates a minimal mock lxapp-create template directory that exercises
    /// all framework-specific placeholders.
    fn make_mock_template_dir() -> tempfile::TempDir {
        let root = tempdir().unwrap();
        let lxapp = root.path().join("lxapp-create");
        let pages_home = lxapp.join("pages").join("home");
        fs::create_dir_all(&pages_home).unwrap();
        fs::create_dir_all(lxapp.join("shared")).unwrap();

        fs::write(
            lxapp.join("package.json"),
            r#"{"name":"{{APP_PACKAGE_NAME}}","dependencies":{"{{FRAMEWORK_PKG}}":"^{{LINGXIA_BRIDGE_VERSION}}","@lingxia/rong":"^{{RONG_VERSION}}","@lingxia/types":"^{{LINGXIA_TYPES_VERSION}}",{{FRAMEWORK_RUNTIME_DEPS}}},"devDependencies":{{{FRAMEWORK_DEV_DEPS_PREFIX}}{{FRAMEWORK_VITE_DEV_DEPS}}"typescript":"^5"}}"#,
        ).unwrap();
        fs::write(
            lxapp.join("lxapp.json"),
            r#"{"framework":"{{FRAMEWORK}}","pages":["pages/home/index.{{PAGE_EXT}}"]}"#,
        )
        .unwrap();
        fs::write(
            lxapp.join("tsconfig.json"),
            r#"{"compilerOptions":{"jsx":"{{JSX_MODE}}"},"include":[{{TSCONFIG_INCLUDE}}]}"#,
        )
        .unwrap();
        fs::write(
            lxapp.join("app.css"),
            "{{APP_ROOT_SELECTOR}} { min-height: 100%; }",
        )
        .unwrap();
        fs::write(lxapp.join("lxapp.ts"), "App({});").unwrap();
        fs::write(lxapp.join("lxapp.config.ts"), "export default {};").unwrap();
        fs::write(lxapp.join("gitignore"), "node_modules/").unwrap();
        fs::write(
            pages_home.join("index.json"),
            r#"{"navigationStyle":"custom"}"#,
        )
        .unwrap();
        fs::write(pages_home.join("index.ts"), "Page({});").unwrap();
        fs::write(
            pages_home.join("index.tsx"),
            "export default function Page() {}",
        )
        .unwrap();
        fs::write(pages_home.join("index.vue"), "<template></template>").unwrap();
        fs::write(lxapp.join("shared").join(".gitkeep"), "").unwrap();

        root
    }

    fn scaffold(framework: &str) -> (tempfile::TempDir, tempfile::TempDir) {
        let templates_root = make_mock_template_dir();
        let out = tempdir().unwrap();
        let target = out.path().join("myapp");
        fs::create_dir_all(&target).unwrap();

        let vars = build_framework_vars(
            framework,
            "my-app",
            "My App",
            &dummy_versions(),
            "0.4.0",
            "0.4.0",
        )
        .unwrap();

        let template_dir = templates_root.path().join("lxapp-create");
        process_template_dir(&template_dir, &target, &vars).unwrap();

        (templates_root, out)
    }

    // --- slugify ---

    #[test]
    fn slugify_lowercases_and_hyphenates() {
        assert_eq!(slugify("My App"), "my-app");
        assert_eq!(slugify("Hello World 123"), "hello-world-123");
    }

    #[test]
    fn slugify_strips_leading_trailing_dashes() {
        assert_eq!(slugify("  --abc--  "), "abc");
    }

    #[test]
    fn slugify_empty_returns_default() {
        assert_eq!(slugify(""), "lingxia-app");
        assert_eq!(slugify("---"), "lingxia-app");
    }

    // --- build_framework_vars ---

    #[test]
    fn react_vars_are_correct() {
        let vars = build_framework_vars(
            "react",
            "my-app",
            "My App",
            &dummy_versions(),
            "0.4.0",
            "0.3.0",
        )
        .unwrap();
        assert_eq!(vars["FRAMEWORK"], "react");
        assert_eq!(vars["FRAMEWORK_PKG"], "@lingxia/page-runtime");
        assert_eq!(vars["PAGE_EXT"], "tsx");
        assert_eq!(vars["JSX_MODE"], "react-jsx");
        assert_eq!(vars["APP_ROOT_SELECTOR"], "#root");
        assert!(vars["TSCONFIG_INCLUDE"].contains("**/*.tsx"));
        assert!(!vars["TSCONFIG_INCLUDE"].contains("**/*.vue"));
        assert!(vars["FRAMEWORK_RUNTIME_DEPS"].contains("react-dom"));
        assert!(vars["FRAMEWORK_DEV_DEPS_PREFIX"].contains("@types/react"));
        assert!(vars["FRAMEWORK_VITE_DEV_DEPS"].contains("@vitejs/plugin-react"));
        assert!(vars["FRAMEWORK_VITE_DEV_DEPS"].contains("\"esbuild\""));
        assert!(vars["FRAMEWORK_VITE_DEV_DEPS"].contains("\"vite\""));
    }

    #[test]
    fn vue_vars_are_correct() {
        let vars = build_framework_vars(
            "vue",
            "my-app",
            "My App",
            &dummy_versions(),
            "0.4.0",
            "0.3.0",
        )
        .unwrap();
        assert_eq!(vars["FRAMEWORK"], "vue");
        assert_eq!(vars["FRAMEWORK_PKG"], "@lingxia/page-runtime");
        assert_eq!(vars["PAGE_EXT"], "vue");
        assert_eq!(vars["JSX_MODE"], "preserve");
        assert_eq!(vars["APP_ROOT_SELECTOR"], "#app");
        assert!(vars["TSCONFIG_INCLUDE"].contains("**/*.vue"));
        assert!(vars["FRAMEWORK_RUNTIME_DEPS"].contains("\"vue\""));
        assert!(vars["FRAMEWORK_DEV_DEPS_PREFIX"].contains("vue-tsc"));
        assert!(vars["FRAMEWORK_VITE_DEV_DEPS"].contains("@vitejs/plugin-vue"));
        assert!(vars["FRAMEWORK_VITE_DEV_DEPS"].contains("\"esbuild\""));
        assert!(vars["FRAMEWORK_VITE_DEV_DEPS"].contains("\"vite\""));
    }

    #[test]
    fn unknown_framework_returns_error() {
        let result = build_framework_vars("svelte", "x", "X", &dummy_versions(), "0.1.0", "0.1.0");
        assert!(result.is_err());
    }

    #[test]
    fn version_vars_are_passed_through() {
        let vars = build_framework_vars("react", "app", "App", &dummy_versions(), "1.2.3", "4.5.6")
            .unwrap();
        assert_eq!(vars["LINGXIA_BRIDGE_VERSION"], "1.2.3");
        assert_eq!(vars["LINGXIA_TYPES_VERSION"], "4.5.6");
        assert_eq!(vars["RONG_VERSION"], "0.1.0");
    }

    // --- scaffold output: React ---

    #[test]
    fn react_scaffold_creates_tsx_not_vue() {
        let (_tmpl, out) = scaffold("react");
        let app = out.path().join("myapp");
        assert!(
            app.join("pages/home/index.tsx").exists(),
            "index.tsx must exist"
        );
        assert!(
            !app.join("pages/home/index.vue").exists(),
            "index.vue must not exist"
        );
    }

    #[test]
    fn react_scaffold_package_json_framework() {
        let (_tmpl, out) = scaffold("react");
        let s = fs::read_to_string(out.path().join("myapp/package.json")).unwrap();
        assert!(
            s.contains("@lingxia/page-runtime"),
            "must reference @lingxia/page-runtime"
        );
        assert!(!s.contains("@lingxia/react"), "must not reference @lingxia/react");
        assert!(!s.contains("@lingxia/vue"), "must not reference @lingxia/vue");
        assert!(s.contains("\"vite\""), "must include vite");
        assert!(s.contains("\"esbuild\""), "must include esbuild");
        assert!(
            s.contains("@vitejs/plugin-react"),
            "must include react vite plugin"
        );
    }

    #[test]
    fn react_scaffold_tsconfig_jsx_mode() {
        let (_tmpl, out) = scaffold("react");
        let s = fs::read_to_string(out.path().join("myapp/tsconfig.json")).unwrap();
        assert!(s.contains("react-jsx"), "jsx must be react-jsx");
        assert!(!s.contains("preserve"), "jsx must not be preserve");
    }

    #[test]
    fn react_scaffold_lxapp_json_page_ext() {
        let (_tmpl, out) = scaffold("react");
        let s = fs::read_to_string(out.path().join("myapp/lxapp.json")).unwrap();
        assert!(s.contains("index.tsx"), "page must be index.tsx");
        assert!(!s.contains("index.vue"), "page must not be index.vue");
    }

    #[test]
    fn react_scaffold_app_css_root_selector() {
        let (_tmpl, out) = scaffold("react");
        let s = fs::read_to_string(out.path().join("myapp/app.css")).unwrap();
        assert!(s.contains("#root"), "app.css must use #root");
        assert!(!s.contains("#app"), "app.css must not use #app");
    }

    // --- scaffold output: Vue ---

    #[test]
    fn vue_scaffold_creates_vue_not_tsx() {
        let (_tmpl, out) = scaffold("vue");
        let app = out.path().join("myapp");
        assert!(
            app.join("pages/home/index.vue").exists(),
            "index.vue must exist"
        );
        assert!(
            !app.join("pages/home/index.tsx").exists(),
            "index.tsx must not exist"
        );
    }

    #[test]
    fn vue_scaffold_package_json_framework() {
        let (_tmpl, out) = scaffold("vue");
        let s = fs::read_to_string(out.path().join("myapp/package.json")).unwrap();
        assert!(
            s.contains("@lingxia/page-runtime"),
            "must reference @lingxia/page-runtime"
        );
        assert!(!s.contains("@lingxia/react"), "must not reference @lingxia/react");
        assert!(!s.contains("@lingxia/vue"), "must not reference @lingxia/vue");
        assert!(s.contains("\"vite\""), "must include vite");
        assert!(s.contains("\"esbuild\""), "must include esbuild");
        assert!(
            s.contains("@vitejs/plugin-vue"),
            "must include vue vite plugin"
        );
    }

    #[test]
    fn vue_scaffold_tsconfig_jsx_mode() {
        let (_tmpl, out) = scaffold("vue");
        let s = fs::read_to_string(out.path().join("myapp/tsconfig.json")).unwrap();
        assert!(s.contains("preserve"), "jsx must be preserve");
        assert!(!s.contains("react-jsx"), "jsx must not be react-jsx");
    }

    #[test]
    fn vue_scaffold_lxapp_json_page_ext() {
        let (_tmpl, out) = scaffold("vue");
        let s = fs::read_to_string(out.path().join("myapp/lxapp.json")).unwrap();
        assert!(s.contains("index.vue"), "page must be index.vue");
        assert!(!s.contains("index.tsx"), "page must not be index.tsx");
    }

    #[test]
    fn vue_scaffold_app_css_root_selector() {
        let (_tmpl, out) = scaffold("vue");
        let s = fs::read_to_string(out.path().join("myapp/app.css")).unwrap();
        assert!(s.contains("#app"), "app.css must use #app");
        assert!(!s.contains("#root"), "app.css must not use #root");
    }

    // --- shared scaffold behaviour ---

    #[test]
    fn scaffold_renames_gitignore() {
        let (_tmpl, out) = scaffold("react");
        let app = out.path().join("myapp");
        assert!(
            app.join(".gitignore").exists(),
            ".gitignore must be created"
        );
        assert!(
            !app.join("gitignore").exists(),
            "bare gitignore must not exist"
        );
    }

    #[test]
    fn scaffold_slugifies_project_name_in_package_json() {
        let (_tmpl, out) = scaffold("react");
        let s = fs::read_to_string(out.path().join("myapp/package.json")).unwrap();
        assert!(s.contains("my-app"), "package name must be slugified");
    }

    #[test]
    fn scaffold_shared_files_always_present() {
        let (_tmpl, out) = scaffold("react");
        let app = out.path().join("myapp");
        assert!(app.join("lxapp.ts").exists());
        assert!(app.join("lxapp.config.ts").exists());
        assert!(app.join("pages/home/index.ts").exists());
        assert!(app.join("pages/home/index.json").exists());
    }
}
