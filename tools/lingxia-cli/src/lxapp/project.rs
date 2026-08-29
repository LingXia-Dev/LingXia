use crate::lxapp::framework::{ProjectFramework, detect_project_framework, resolve_page_path};
use anyhow::{Context, Result, anyhow};
use semver::Version;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MIN_TAB_ITEMS: usize = 2;
const MAX_TAB_ITEMS: usize = 10;

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
            let framework = resolve_framework(project_root, &manifest, framework_override)?;
            let pages = resolve_lxapp_pages(project_root, &manifest, framework)?;
            validate_page_configs(project_root, &pages)?;
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
            let framework = resolve_framework(project_root, &manifest, framework_override)?;
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

fn resolve_framework(
    project_root: &Path,
    manifest: &Value,
    framework_override: Option<ProjectFramework>,
) -> Result<ProjectFramework> {
    // A manifest pin is part of the package contract. A host-level framework
    // selection chooses among ambiguous app sources; it must not reinterpret
    // an explicitly HTML/React/Vue resource bundled alongside that app.
    if manifest.get("framework").is_some() {
        return detect_project_framework(project_root);
    }
    framework_override.map_or_else(|| detect_project_framework(project_root), Ok)
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
    validate_page_chrome_manifest(manifest)?;
    validate_lxapp_security(manifest.get("security"))?;
    Ok(())
}

fn validate_page_configs(project_root: &Path, pages: &[String]) -> Result<()> {
    for page in pages {
        let relative = Path::new(page).with_extension("json");
        let path = project_root.join(&relative);
        if !path.exists() {
            continue;
        }
        let value = read_json(&path)?;
        let object = value
            .as_object()
            .ok_or_else(|| anyhow!("{}: page config must be an object", relative.display()))?;
        for (legacy, replacement) in [
            ("navigationBarTitleText", "navigationBar.title"),
            (
                "navigationBarBackgroundColor",
                "navigationBar.style.backgroundColor",
            ),
            (
                "navigationBarTextStyle",
                "navigationBar.style.foregroundColor",
            ),
        ] {
            if object.contains_key(legacy) {
                return Err(anyhow!(
                    "{} {legacy}: removed; use {replacement}",
                    relative.display()
                ));
            }
        }
        if object.contains_key("backgroundColor") {
            return Err(anyhow!(
                "{} backgroundColor: removed; page background is host-owned",
                relative.display()
            ));
        }
        for key in object.keys() {
            if ![
                "navigationStyle",
                "navigationBar",
                "enablePullDownRefresh",
                "pageOrientation",
            ]
            .contains(&key.as_str())
            {
                return Err(anyhow!(
                    "{} {key}: unknown page config field",
                    relative.display()
                ));
            }
        }
        if let Some(style) = object.get("navigationStyle")
            && !matches!(style.as_str(), Some("default" | "custom"))
        {
            return Err(anyhow!(
                "{} navigationStyle: expected default or custom",
                relative.display()
            ));
        }
        if let Some(navigation_bar) = object.get("navigationBar") {
            let navigation_bar = navigation_bar.as_object().ok_or_else(|| {
                anyhow!("{} navigationBar: expected an object", relative.display())
            })?;
            reject_unknown_fields(
                navigation_bar,
                &["title", "style"],
                &format!("{} navigationBar", relative.display()),
            )?;
            if let Some(title) = navigation_bar.get("title")
                && !title.is_string()
            {
                return Err(anyhow!(
                    "{} navigationBar.title: expected a string",
                    relative.display()
                ));
            }
            if let Some(style) = navigation_bar.get("style") {
                validate_style_object(
                    style,
                    &["backgroundColor", "foregroundColor", "dividerColor"],
                    &format!("{} navigationBar.style", relative.display()),
                    &["backgroundColor", "foregroundColor"],
                )?;
            }
        }
    }
    Ok(())
}

fn validate_page_chrome_manifest(manifest: &Value) -> Result<()> {
    if let Some(value) = manifest.get("appearance") {
        match value.as_str() {
            Some("auto" | "light" | "dark") => {}
            _ => return Err(anyhow!("appearance: expected auto, light, or dark")),
        }
    }
    let Some(tabbar) = manifest.get("tabBar") else {
        return Ok(());
    };
    let tabbar = tabbar
        .as_object()
        .ok_or_else(|| anyhow!("tabBar: expected an object"))?;
    for (legacy, replacement) in [
        ("list", "tabBar.items"),
        ("color", "tabBar.style.foregroundColor"),
        ("selectedColor", "tabBar.style.selectedForegroundColor"),
        ("backgroundColor", "tabBar.style.backgroundColor"),
        ("borderStyle", "tabBar.style.dividerColor"),
        ("position", "tabBar.presentation"),
        ("dimension", "host-owned layout"),
    ] {
        if tabbar.contains_key(legacy) {
            return Err(anyhow!("tabBar.{legacy}: removed; use {replacement}"));
        }
    }
    if !tabbar.get("items").is_some_and(Value::is_array) {
        return Err(anyhow!("tabBar.items: expected an array"));
    }
    reject_unknown_fields(tabbar, &["presentation", "style", "items"], "tabBar")?;
    let presentation = tabbar
        .get("presentation")
        .map(|value| {
            value
                .as_str()
                .filter(|value| matches!(*value, "standard" | "immersive"))
                .ok_or_else(|| anyhow!("tabBar.presentation: expected standard or immersive"))
        })
        .transpose()?
        .unwrap_or("standard");
    if let Some(style) = tabbar.get("style") {
        let style = validate_style_object(
            style,
            &[
                "foregroundColor",
                "selectedForegroundColor",
                "backgroundColor",
                "dividerColor",
            ],
            "tabBar.style",
            &[
                "foregroundColor",
                "selectedForegroundColor",
                "backgroundColor",
            ],
        )?;
        if presentation == "immersive" {
            for field in ["backgroundColor", "dividerColor"] {
                if style.contains_key(field) {
                    return Err(anyhow!(
                        "tabBar.style.{field}: must be omitted when tabBar.presentation is immersive"
                    ));
                }
            }
        }
    }
    let items = tabbar["items"].as_array().expect("items checked above");
    // Mirrors TabBar::MIN_ITEMS/MAX_ITEMS in lingxia-lxapp; the runtime crate is
    // too heavy a dependency for the CLI, so the bound is restated here.
    if !(MIN_TAB_ITEMS..=MAX_TAB_ITEMS).contains(&items.len()) {
        return Err(anyhow!(
            "tabBar.items: expected {MIN_TAB_ITEMS} to {MAX_TAB_ITEMS} items"
        ));
    }
    let page_paths: BTreeSet<&str> = manifest
        .get("pages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|page| page.get("path").and_then(Value::as_str))
        .collect();
    for (index, item) in items.iter().enumerate() {
        let item = item
            .as_object()
            .ok_or_else(|| anyhow!("tabBar.items[{index}]: expected an object"))?;
        if item.contains_key("selected") {
            return Err(anyhow!("tabBar.items[{index}].selected: removed"));
        }
        reject_unknown_fields(
            item,
            &["pagePath", "text", "iconPath", "showOn"],
            &format!("tabBar.items[{index}]"),
        )?;
        let page_path = item
            .get("pagePath")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("tabBar.items[{index}].pagePath: expected a string"))?;
        if !page_paths.contains(page_path) {
            return Err(anyhow!(
                "tabBar.items[{index}].pagePath: '{page_path}' is not a registered page"
            ));
        }
        for field in ["text", "iconPath"] {
            if let Some(value) = item.get(field)
                && !value.is_string()
            {
                return Err(anyhow!("tabBar.items[{index}].{field}: expected a string"));
            }
        }
    }
    Ok(())
}

fn reject_unknown_fields(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<()> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(anyhow!("{path}.{field}: unknown field"));
    }
    Ok(())
}

fn validate_style_object<'a>(
    value: &'a Value,
    allowed: &[&str],
    path: &str,
    opaque: &[&str],
) -> Result<&'a serde_json::Map<String, Value>> {
    let style = value
        .as_object()
        .ok_or_else(|| anyhow!("{path}: expected an object"))?;
    reject_unknown_fields(style, allowed, path)?;
    for (field, value) in style {
        let color = value
            .as_str()
            .ok_or_else(|| anyhow!("{path}.{field}: expected a CSS hex color"))?;
        let hex = color
            .strip_prefix('#')
            .ok_or_else(|| anyhow!("{path}.{field}: expected #RRGGBB or #RRGGBBAA"))?;
        let valid_length = hex.len() == 6 || (!opaque.contains(&field.as_str()) && hex.len() == 8);
        if !valid_length || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(anyhow!(
                "{path}.{field}: expected {}",
                if opaque.contains(&field.as_str()) {
                    "opaque #RRGGBB"
                } else {
                    "#RRGGBB or #RRGGBBAA"
                }
            ));
        }
    }
    Ok(style)
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

    fn manifest_with_tab_items(count: usize) -> Value {
        let pages: Vec<Value> = (0..count)
            .map(|index| {
                serde_json::json!({
                    "name": format!("p{index}"),
                    "path": format!("pages/p{index}/index"),
                })
            })
            .collect();
        let items: Vec<Value> = (0..count)
            .map(|index| serde_json::json!({ "pagePath": format!("pages/p{index}/index") }))
            .collect();
        serde_json::json!({ "pages": pages, "tabBar": { "items": items } })
    }

    /// The bound is restated here because the CLI cannot depend on the runtime
    /// crate; this pins it so the two copies cannot drift apart unnoticed.
    #[test]
    fn tab_bar_accepts_two_to_ten_items() {
        for count in [2, 5, 6, 10] {
            validate_page_chrome_manifest(&manifest_with_tab_items(count))
                .unwrap_or_else(|error| panic!("{count} items rejected: {error}"));
        }
        for count in [0, 1, 11, 12] {
            let error = validate_page_chrome_manifest(&manifest_with_tab_items(count))
                .expect_err(&format!("{count} items accepted"));
            assert_eq!(error.to_string(), "tabBar.items: expected 2 to 10 items");
        }
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
                "privileges": ["downloads", "vendor_devtools"]
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
        // Only the frameworks actually present are offered, and the manifest
        // pin is suggested as the permanent fix.
        assert!(error.contains("Pass --framework react|vue,"), "{error}");
        assert!(error.contains("\"framework\""), "{error}");
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
    fn manifest_framework_pin_beats_host_override() {
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
              "pages": [{"name":"home","path":"pages/home/index.html"}]
            }"#,
        );
        write_file(temp.path(), "pages/home/index.html", "<!doctype html>");

        let project = Project::discover(temp.path(), Some(ProjectFramework::React)).unwrap();

        assert_eq!(project.framework, ProjectFramework::Html);
        assert_eq!(project.pages, vec!["pages/home/index.html".to_string()]);
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

    #[test]
    fn rejects_removed_tabbar_fields_with_replacement() {
        let manifest = serde_json::json!({
            "appId": "demo",
            "version": "1.0.0",
            "security": {"network":{"trustedDomains":[]},"privileges":[]},
            "pages": [
                {"name":"home","path":"pages/home/index"},
                {"name":"profile","path":"pages/profile/index"}
            ],
            "tabBar": {"list": []}
        });

        let error = validate_lxapp_manifest(&manifest).unwrap_err().to_string();
        assert!(error.contains("tabBar.list: removed; use tabBar.items"));
    }

    #[test]
    fn rejects_removed_page_background_with_actionable_message() {
        let temp = tempdir().unwrap();
        write_file(
            temp.path(),
            "pages/home/index.json",
            r##"{"backgroundColor":"#FFFFFF"}"##,
        );

        let error = validate_page_configs(temp.path(), &["pages/home/index".to_string()])
            .unwrap_err()
            .to_string();

        assert!(error.contains("backgroundColor: removed; page background is host-owned"));
    }

    #[test]
    fn rejects_removed_tab_item_selected_with_actionable_message() {
        let manifest = serde_json::json!({
            "appId": "demo",
            "version": "1.0.0",
            "security": {"network":{"trustedDomains":[]},"privileges":[]},
            "pages": [
                {"name":"home","path":"pages/home/index"},
                {"name":"profile","path":"pages/profile/index"}
            ],
            "tabBar": {
                "items": [
                    {"pagePath":"pages/home/index", "selected": true},
                    {"pagePath":"pages/profile/index"}
                ]
            }
        });

        let error = validate_lxapp_manifest(&manifest).unwrap_err().to_string();
        assert!(error.contains("tabBar.items[0].selected: removed"));
    }

    #[test]
    fn validates_immersive_tabbar_contract() {
        let manifest = serde_json::json!({
            "appId": "demo",
            "version": "1.0.0",
            "appearance": "dark",
            "security": {"network":{"trustedDomains":[]},"privileges":[]},
            "pages": [
                {"name":"home","path":"pages/home/index"},
                {"name":"profile","path":"pages/profile/index"}
            ],
            "tabBar": {
                "presentation": "immersive",
                "style": {"foregroundColor":"#FFFFFF"},
                "items": [
                    {"pagePath":"pages/home/index"},
                    {"pagePath":"pages/profile/index"}
                ]
            }
        });

        validate_lxapp_manifest(&manifest).unwrap();
    }
}
