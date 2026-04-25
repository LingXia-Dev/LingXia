use crate::lxapp::framework::{ProjectFramework, detect_project_framework, resolve_page_path};
use anyhow::{Context, Result, anyhow};
use semver::Version;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    LxApp,
    LxPlugin,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub kind: ProjectKind,
    pub framework: ProjectFramework,
    pub output_dir: PathBuf,
    pub pages: Vec<String>,
    pub logic_entry: Option<String>,
    pub plugin_id: Option<String>,
    pub package_name: Option<String>,
    pub version: String,
}

impl Project {
    pub fn discover(
        project_root: &Path,
        framework_override: Option<ProjectFramework>,
    ) -> Result<Self> {
        let lxapp_path = project_root.join("lxapp.json");
        let lxplugin_path = project_root.join("lxplugin.json");

        if lxapp_path.exists() {
            let manifest = read_json(&lxapp_path)?;
            validate_lxapp_manifest(&manifest)?;
            let framework = match framework_override {
                Some(framework) => framework,
                None => detect_project_framework(project_root)?,
            };
            let pages = resolve_lxapp_pages(project_root, &manifest, framework)?;
            let logic_entry = resolve_logic_entry(&manifest)?;
            let version = non_empty_str(manifest.get("version"), "version in lxapp.json")?;
            let package_name = read_package_name(project_root)?;
            return Ok(Self {
                root: project_root.to_path_buf(),
                kind: ProjectKind::LxApp,
                framework,
                output_dir: project_root.join("dist"),
                pages,
                logic_entry,
                plugin_id: None,
                package_name,
                version,
            });
        }

        if lxplugin_path.exists() {
            let manifest = read_json(&lxplugin_path)?;
            let framework = match framework_override {
                Some(framework) => framework,
                None => detect_project_framework(project_root)?,
            };
            let pages_obj = manifest
                .get("pages")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    anyhow!("lxplugin.json pages must be an array of objects with name/path")
                })?;
            let mut pages = Vec::with_capacity(pages_obj.len());
            let mut page_names = BTreeSet::new();
            for value in pages_obj {
                let entry = value.as_object().ok_or_else(|| {
                    anyhow!("lxplugin.json pages entries must be objects with name/path")
                })?;
                let name = entry
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("lxplugin.json pages entries must include name"))?;
                if !is_valid_page_name(name) {
                    return Err(anyhow!(
                        "lxplugin.json page name must use letters, numbers, '_' or '-': {name:?}"
                    ));
                }
                if !page_names.insert(name) {
                    return Err(anyhow!("lxplugin.json page name must be unique: {name:?}"));
                }
                let page = entry
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("lxplugin.json pages entries must include path"))?;
                validate_page_path(page, "lxplugin.json pages entry")?;
                let resolved = resolve_page_path(project_root, page, framework)
                    .ok_or_else(|| anyhow!("Page file not found for {page}"))?;
                pages.push(resolved);
            }
            let plugin_id =
                non_empty_str(manifest.get("lxPluginId"), "lxPluginId in lxplugin.json")?;
            let version = non_empty_str(manifest.get("version"), "version in lxplugin.json")?;
            let package_name = read_package_name(project_root)?.or_else(|| Some(plugin_id.clone()));
            return Ok(Self {
                root: project_root.to_path_buf(),
                kind: ProjectKind::LxPlugin,
                framework,
                output_dir: project_root.join("dist-plugin"),
                pages,
                logic_entry: Some("logic.js".to_string()),
                plugin_id: Some(plugin_id),
                package_name,
                version,
            });
        }

        Err(anyhow!(
            "No lxapp.json or lxplugin.json found in {}",
            project_root.display()
        ))
    }
}

fn validate_lxapp_manifest(manifest: &Value) -> Result<()> {
    non_empty_str(manifest.get("appId"), "appId in lxapp.json")?;
    let version = non_empty_str(manifest.get("version"), "version in lxapp.json")?;
    Version::parse(&version).map_err(|_| {
        anyhow!("version in lxapp.json must be a semantic version (major.minor.patch)")
    })?;
    if manifest.get("appService").is_some() {
        return Err(anyhow!(
            r#""appService" is no longer supported; use "logic" instead"#
        ));
    }
    validate_lxapp_pages(manifest.get("pages"))?;
    validate_lxapp_security(manifest.get("security"))?;
    Ok(())
}

fn validate_lxapp_pages(pages: Option<&Value>) -> Result<()> {
    match pages {
        Some(Value::Array(entries)) => {
            if entries.is_empty() {
                return Err(anyhow!("lxapp.json pages must not be empty"));
            }
            let mut page_names = BTreeSet::new();
            for value in entries {
                let entry = value.as_object().ok_or_else(|| {
                    anyhow!("lxapp.json pages entries must be objects with name/path")
                })?;
                let name = entry
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("lxapp.json pages entries must include name"))?;
                if !is_valid_page_name(name) {
                    return Err(anyhow!(
                        "lxapp.json page name must use letters, numbers, '_' or '-': {name:?}"
                    ));
                }
                if !page_names.insert(name) {
                    return Err(anyhow!("lxapp.json page name must be unique: {name:?}"));
                }
                let page = entry
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("lxapp.json pages entries must include path"))?;
                validate_page_path(page, "lxapp.json pages entry")?;
            }
            Ok(())
        }
        _ => Err(anyhow!(
            "lxapp.json pages must be an array of objects with name/path"
        )),
    }
}

fn is_valid_page_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn validate_page_path(path: &str, field: &str) -> Result<()> {
    let path = path.trim();
    if path.is_empty() {
        return Err(anyhow!("{field} must not be empty"));
    }
    if path.contains('\\') || Path::new(path).is_absolute() {
        return Err(anyhow!("{field} must be a relative package path: {path:?}"));
    }
    if !Path::new(path)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(anyhow!(
            "{field} must stay inside the lxapp package: {path:?}"
        ));
    }
    Ok(())
}

fn validate_lxapp_security(security: Option<&Value>) -> Result<()> {
    let security = security.ok_or_else(|| anyhow!("lxapp.json security must be declared"))?;
    let security = security
        .as_object()
        .ok_or_else(|| anyhow!("lxapp.json security must be an object"))?;

    let network = security
        .get("network")
        .ok_or_else(|| anyhow!("lxapp.json security.network must be declared"))?
        .as_object()
        .ok_or_else(|| anyhow!("lxapp.json security.network must be an object"))?;
    let domains = network
        .get("trustedDomains")
        .ok_or_else(|| anyhow!("lxapp.json security.network.trustedDomains must be declared"))?
        .as_array()
        .ok_or_else(|| anyhow!("lxapp.json security.network.trustedDomains must be an array"))?;
    let mut normalized_domains = BTreeSet::new();
    for domain in domains {
        let domain = domain.as_str().ok_or_else(|| {
            anyhow!("lxapp.json security.network.trustedDomains entries must be strings")
        })?;
        validate_trusted_domain(domain)?;
        normalized_domains.insert(domain.trim().trim_end_matches('.').to_ascii_lowercase());
    }
    if normalized_domains.len() > 1 && normalized_domains.contains("*") {
        return Err(anyhow!(
            "lxapp.json security.network.trustedDomains wildcard \"*\" cannot be combined with other hosts"
        ));
    }

    let privileges = security
        .get("privileges")
        .ok_or_else(|| anyhow!("lxapp.json security.privileges must be declared"))?
        .as_array()
        .ok_or_else(|| anyhow!("lxapp.json security.privileges must be an array"))?;
    for privilege in privileges {
        let privilege = privilege
            .as_str()
            .ok_or_else(|| anyhow!("lxapp.json security.privileges entries must be strings"))?;
        validate_security_privilege_id(privilege)?;
    }

    Ok(())
}

fn validate_security_privilege_id(privilege: &str) -> Result<()> {
    let trimmed = privilege.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(char::is_whitespace)
        || !trimmed.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_')
        })
    {
        return Err(anyhow!(
            "lxapp.json security.privileges entries must be lowercase identifiers: {privilege:?}"
        ));
    }
    Ok(())
}

fn validate_trusted_domain(domain: &str) -> Result<()> {
    let trimmed = domain.trim().trim_end_matches('.');
    if trimmed == "*" {
        return Ok(());
    }
    if !is_valid_trusted_host(trimmed)
        || trimmed.contains("://")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || trimmed.chars().any(char::is_whitespace)
    {
        return Err(anyhow!(
            "lxapp.json security.network.trustedDomains entries must be host names without scheme/path: {domain:?}"
        ));
    }
    Ok(())
}

fn is_valid_trusted_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }

    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

fn read_json(path: &Path) -> Result<Value> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

fn read_package_name(project_root: &Path) -> Result<Option<String>> {
    let package_json = project_root.join("package.json");
    if !package_json.exists() {
        return Ok(None);
    }
    let value = read_json(&package_json)?;
    Ok(value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned))
}

fn resolve_lxapp_pages(
    project_root: &Path,
    manifest: &Value,
    framework: ProjectFramework,
) -> Result<Vec<String>> {
    match manifest.get("pages") {
        Some(Value::Array(raw_pages)) => {
            let mut pages = Vec::with_capacity(raw_pages.len());
            for value in raw_pages {
                let entry = value.as_object().ok_or_else(|| {
                    anyhow!("lxapp.json pages entries must be objects with name/path")
                })?;
                let page = entry
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("lxapp.json pages entries must include path"))?;
                let resolved = resolve_page_path(project_root, page, framework)
                    .ok_or_else(|| anyhow!("Page file not found for {page}"))?;
                pages.push(resolved);
            }
            Ok(pages)
        }
        _ => Err(anyhow!(
            "lxapp.json pages must be an array of objects with name/path"
        )),
    }
}

fn resolve_logic_entry(manifest: &Value) -> Result<Option<String>> {
    if manifest.get("appService").is_some() {
        return Err(anyhow!(
            "\"appService\" is no longer supported; use \"logic\" instead"
        ));
    }
    let logic = manifest.get("logic");
    match logic {
        None | Some(Value::Null) | Some(Value::Bool(true)) => Ok(Some("logic.js".to_string())),
        Some(Value::Bool(false)) => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                return Err(anyhow!("\"logic\" entry must not be empty"));
            }
            if !is_safe_logic_entry(value) {
                return Err(anyhow!(
                    "\"logic\" entry must stay within the lxapp package: {value:?}"
                ));
            }
            Ok(Some(value.to_string()))
        }
        Some(_) => Err(anyhow!(
            "\"logic\" must be false, true, a string entry path, or omitted"
        )),
    }
}

fn is_safe_logic_entry(entry: &str) -> bool {
    if entry.is_empty() || entry.contains('\\') {
        return false;
    }
    let normalized = Path::new(entry).components().collect::<Vec<_>>();
    if normalized.is_empty() {
        return false;
    }
    !Path::new(entry).is_absolute()
        && !entry.starts_with("../")
        && !entry.contains("/../")
        && entry != "."
}

fn non_empty_str(value: Option<&Value>, field: &str) -> Result<String> {
    let value = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("Missing {field}"))?;
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_file(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discovers_lxapp_with_logic_disabled() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(
            temp.path(),
            "package.json",
            r#"{
              "name": "@demo/home"
            }"#,
        );
        write_file(temp.path(), "pages/home/index.vue", "<template />");

        let project = Project::discover(temp.path(), None).unwrap();

        assert_eq!(project.kind, ProjectKind::LxApp);
        assert_eq!(project.framework, ProjectFramework::Vue);
        assert_eq!(project.pages, vec!["pages/home/index.vue".to_string()]);
        assert_eq!(project.logic_entry, None);
        assert_eq!(project.output_dir, temp.path().join("dist"));
        assert_eq!(project.package_name.as_deref(), Some("@demo/home"));
    }

    #[test]
    fn discovers_lxapp_with_named_pages() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [
                { "name": "newtab", "path": "pages/newtab/index.html" },
                { "name": "settings", "path": "pages/settings/index.html" }
              ]
            }"#,
        );
        write_file(
            temp.path(),
            "package.json",
            r#"{
              "name": "@demo/browser"
            }"#,
        );
        write_file(temp.path(), "pages/newtab/index.html", "<!doctype html>");
        write_file(temp.path(), "pages/settings/index.html", "<!doctype html>");

        let project = Project::discover(temp.path(), None).unwrap();

        assert_eq!(project.kind, ProjectKind::LxApp);
        assert_eq!(project.framework, ProjectFramework::Html);
        assert_eq!(
            project.pages,
            vec![
                "pages/newtab/index.html".to_string(),
                "pages/settings/index.html".to_string()
            ]
        );
        assert_eq!(project.logic_entry, None);
    }

    #[test]
    fn discovers_named_html_pages_without_framework_metadata() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [
                { "name": "newtab", "path": "pages/newtab/index.html" },
                { "name": "settings", "path": "pages/settings/index.html" }
              ]
            }"#,
        );
        write_file(
            temp.path(),
            "package.json",
            r#"{
              "name": "@demo/browser"
            }"#,
        );
        write_file(temp.path(), "pages/newtab/index.html", "<!doctype html>");
        write_file(temp.path(), "pages/settings/index.html", "<!doctype html>");

        let project = Project::discover(temp.path(), None).unwrap();

        assert_eq!(project.kind, ProjectKind::LxApp);
        assert_eq!(project.framework, ProjectFramework::Html);
        assert_eq!(
            project.pages,
            vec![
                "pages/newtab/index.html".to_string(),
                "pages/settings/index.html".to_string()
            ]
        );
        assert_eq!(project.logic_entry, None);
        assert_eq!(project.package_name.as_deref(), Some("@demo/browser"));
    }

    #[test]
    fn rejects_legacy_appservice_field() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "appService": false,
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(
            temp.path(),
            "pages/home/index.tsx",
            "export default function Page() {}",
        );

        let error = Project::discover(temp.path(), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("\"appService\" is no longer supported"));
    }

    #[test]
    fn rejects_lxapp_without_appid() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let error = Project::discover(temp.path(), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("Missing appId in lxapp.json"));
    }

    #[test]
    fn rejects_empty_lxapp_pages() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": []
            }"#,
        );

        let error = Project::discover(temp.path(), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("lxapp.json pages must not be empty"));
    }

    #[test]
    fn rejects_invalid_named_page_key() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [
                { "name": "home page", "path": "pages/home/index" }
              ]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let error = Project::discover(temp.path(), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("lxapp.json page name must use"));
    }

    #[test]
    fn rejects_duplicate_lxapp_page_names() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [
                { "name": "home", "path": "pages/home/index" },
                { "name": "home", "path": "pages/other/index" }
              ]
            }"#,
        );

        let error = Project::discover(temp.path(), Some(ProjectFramework::Html))
            .unwrap_err()
            .to_string();
        assert!(error.contains("page name must be unique"));
    }

    #[test]
    fn rejects_unsafe_lxapp_page_path() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"../outside"}]
            }"#,
        );

        let error = Project::discover(temp.path(), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("lxapp.json pages entry must stay inside"));
    }

    #[test]
    fn accepts_lxapp_security_config() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {
                "network": {
                  "trustedDomains": ["api.example.com", "LOCALHOST"]
                },
                "privileges": ["agent.automation", "vendor_devtools"]
              },
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let project = Project::discover(temp.path(), Some(ProjectFramework::Html)).unwrap();
        assert_eq!(project.kind, ProjectKind::LxApp);
    }

    #[test]
    fn rejects_missing_lxapp_security_config() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );

        let error = Project::discover(temp.path(), Some(ProjectFramework::Html))
            .unwrap_err()
            .to_string();
        assert!(error.contains("lxapp.json security must be declared"));
    }

    #[test]
    fn rejects_invalid_lxapp_security_privilege_id() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {
                "network": {
                  "trustedDomains": []
                },
                "privileges": ["Agent Automation"]
              },
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );

        let error = Project::discover(temp.path(), Some(ProjectFramework::Html))
            .unwrap_err()
            .to_string();
        assert!(error.contains("security.privileges entries must be lowercase identifiers"));
    }

    #[test]
    fn rejects_lxapp_security_domain_with_scheme() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {
                "network": {
                  "trustedDomains": ["https://api.example.com"]
                },
                "privileges": []
              },
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );

        let error = Project::discover(temp.path(), Some(ProjectFramework::Html))
            .unwrap_err()
            .to_string();
        assert!(error.contains("trustedDomains entries must be host names"));
    }

    #[test]
    fn rejects_lxapp_security_wildcard_mixed_with_domains() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {
                "network": {
                  "trustedDomains": ["api.example.com", "*"]
                },
                "privileges": []
              },
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );

        let error = Project::discover(temp.path(), Some(ProjectFramework::Html))
            .unwrap_err()
            .to_string();
        assert!(error.contains("wildcard"));
    }

    #[test]
    fn discovers_lxplugin_and_falls_back_to_plugin_id_for_package_name() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxplugin.json",
            r#"{
              "version": "2.0.0",
              "lxPluginId": "plugin.demo",
              "pages": [
                { "name": "home", "path": "pages/home/index" }
              ]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let project = Project::discover(temp.path(), Some(ProjectFramework::Html)).unwrap();

        assert_eq!(project.kind, ProjectKind::LxPlugin);
        assert_eq!(project.framework, ProjectFramework::Html);
        assert_eq!(project.pages, vec!["pages/home/index.html".to_string()]);
        assert_eq!(project.logic_entry.as_deref(), Some("logic.js"));
        assert_eq!(project.plugin_id.as_deref(), Some("plugin.demo"));
        assert_eq!(project.package_name.as_deref(), Some("plugin.demo"));
        assert_eq!(project.output_dir, temp.path().join("dist-plugin"));
    }

    #[test]
    fn rejects_unsafe_logic_entry() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": "../logic.js",
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let error = Project::discover(temp.path(), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("\"logic\" entry must stay within the lxapp package"));
    }

    #[test]
    fn rejects_ambiguous_extensionless_page_without_framework_override() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.vue", "<template />");
        write_file(
            temp.path(),
            "pages/home/index.tsx",
            "export default function Page() {}",
        );

        let error = Project::discover(temp.path(), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("Pass --framework react|vue|html"));
    }

    #[test]
    fn allows_framework_override_for_ambiguous_extensionless_page() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.vue", "<template />");
        write_file(
            temp.path(),
            "pages/home/index.tsx",
            "export default function Page() {}",
        );

        let project = Project::discover(temp.path(), Some(ProjectFramework::Vue)).unwrap();

        assert_eq!(project.framework, ProjectFramework::Vue);
        assert_eq!(project.pages, vec!["pages/home/index.vue".to_string()]);
    }

    #[test]
    fn prefers_framework_declared_in_lxapp_manifest() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "framework": "react",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.vue", "<template />");
        write_file(
            temp.path(),
            "pages/home/index.tsx",
            "export default function Page() {}",
        );

        let project = Project::discover(temp.path(), None).unwrap();

        assert_eq!(project.framework, ProjectFramework::React);
        assert_eq!(project.pages, vec!["pages/home/index.tsx".to_string()]);
    }

    #[test]
    fn discovers_logic_disabled_without_logic_entry() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "framework": "html",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let project = Project::discover(temp.path(), None).unwrap();

        assert_eq!(project.framework, ProjectFramework::Html);
        assert_eq!(project.logic_entry, None);
        assert_eq!(project.pages, vec!["pages/home/index.html".to_string()]);
    }

    #[test]
    fn rejects_false_logic_with_string_entry_conflict() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "lxapp.json",
            r#"{
              "appId": "demo",
              "version": "1.0.0",
              "framework": "html",
              "logic": false,
              "security": {"network":{"trustedDomains":[]},"privileges":[]},
              "pages": [{"name":"home","path":"pages/home/index"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let project = Project::discover(temp.path(), None).unwrap();
        assert_eq!(project.logic_entry, None);
    }
}
