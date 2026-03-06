use anyhow::{Context, Result, anyhow};
use clap::Args;
use inflector::Inflector;
use jsonschema::{Draft, Validator};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct I18nConfig {
    /// Path to the i18n source root directory
    /// (`ui/`, `permission/cli/`, `permission/runtime/`, `error/`, `schema/`)
    #[arg(short, long, default_value = "i18n")]
    pub input: PathBuf,

    /// Path to output the Rust generated code (Optional)
    #[arg(long)]
    pub rust_out: Option<PathBuf>,

    /// Path to output Android resources (res directory) (Optional)
    #[arg(long)]
    pub android_out: Option<PathBuf>,

    /// Path to output iOS resources (Resources directory) (Optional)
    #[arg(long)]
    pub ios_out: Option<PathBuf>,

    /// Path to output HarmonyOS resources (resources directory) (Optional)
    #[arg(long)]
    pub harmony_out: Option<PathBuf>,

    /// Path to output generated TypeScript files (directory) (Optional)
    #[arg(long)]
    pub ts_out: Option<PathBuf>,

    /// Path to JSON Schema directory (Optional arg, defaults to <input>/schema)
    #[arg(long)]
    pub schema_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
struct TranslationValue {
    default: String,
    android: Option<String>,
    apple: Option<String>,
    ios: Option<String>,
    harmony: Option<String>,
    rust: Option<String>,
}

impl TranslationValue {
    fn from_string(s: String) -> Self {
        Self {
            default: s,
            android: None,
            apple: None,
            ios: None,
            harmony: None,
            rust: None,
        }
    }

    fn get_for_android(&self) -> &String {
        self.android.as_ref().unwrap_or(&self.default)
    }

    fn get_for_apple(&self) -> &String {
        // Prefer explicit 'ios', then 'apple', then default
        self.ios
            .as_ref()
            .or(self.apple.as_ref())
            .unwrap_or(&self.default)
    }

    fn get_explicit_apple(&self) -> Option<&String> {
        self.ios.as_ref().or(self.apple.as_ref())
    }

    fn get_for_harmony(&self) -> &String {
        self.harmony.as_ref().unwrap_or(&self.default)
    }
}

// Use BTreeMap to ensure keys are sorted for deterministic output
type Translations = BTreeMap<String, TranslationValue>;
type I18nMap = BTreeMap<String, Translations>;

const UI_LOCALE_DIR: &str = "ui";
const PERMISSION_RUNTIME_DIR: &str = "permission/runtime";
const PERMISSION_CLI_DIR: &str = "permission/cli";
const ERROR_DIR: &str = "error";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocaleSourceKind {
    Ui,
    PermissionRuntime,
    ErrorLocale,
}

pub fn run(config: I18nConfig) -> Result<()> {
    println!("Scanning for i18n files in: {:?}", config.input);

    let mut i18n_map: I18nMap = BTreeMap::new();
    let mut all_keys: BTreeMap<String, ()> = BTreeMap::new();
    let mut locale_documents: Vec<(String, PathBuf, serde_yaml_ng::Value)> = Vec::new();
    let locale_sources = [
        (config.input.join(UI_LOCALE_DIR), LocaleSourceKind::Ui),
        (
            config.input.join(PERMISSION_RUNTIME_DIR),
            LocaleSourceKind::PermissionRuntime,
        ),
        (config.input.join(ERROR_DIR), LocaleSourceKind::ErrorLocale),
    ];
    for (source_dir, _) in &locale_sources {
        if !source_dir.exists() {
            return Err(anyhow!(
                "Missing locale i18n directory: {}",
                source_dir.display()
            ));
        }
    }

    // 1. Read and parse YAML files from UI + runtime permission + error locale fragments.
    for (source_dir, source_kind) in &locale_sources {
        for path in collect_yaml_files(source_dir)? {
            let lang_code = path
                .file_stem()
                .context("No file stem")?
                .to_string_lossy()
                .to_string();
            println!("Found locale fragment: {} ({})", lang_code, path.display());

            let content = fs::read_to_string(&path)?;
            let yaml_value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            match source_kind {
                LocaleSourceKind::Ui => validate_ui_locale_boundary(&path, &yaml_value)?,
                LocaleSourceKind::ErrorLocale => {
                    validate_error_locale_boundary(&path, &yaml_value)?
                }
                LocaleSourceKind::PermissionRuntime => {}
            }
            let flat_map = flatten_yaml(&yaml_value, None);

            let locale_entry = i18n_map.entry(lang_code.clone()).or_default();
            for (key, value) in flat_map {
                if locale_entry.insert(key.clone(), value).is_some() {
                    return Err(anyhow!(
                        "Duplicate i18n key `{}` found in locale `{}` while loading `{}`",
                        key,
                        lang_code,
                        path.display()
                    ));
                }
                all_keys.insert(key, ());
            }

            locale_documents.push((lang_code, path, yaml_value));
        }
    }

    if i18n_map.is_empty() {
        return Err(anyhow!(
            "No i18n locale YAML files found in {}, {}, or {}",
            locale_sources[0].0.display(),
            locale_sources[1].0.display(),
            locale_sources[2].0.display()
        ));
    }

    let schema_dir = config
        .schema_dir
        .clone()
        .unwrap_or_else(|| config.input.join("schema"));
    let ui_schema_path = schema_dir.join("ui.schema.json");
    let permission_schema_path = schema_dir.join("permission.schema.json");
    validate_schemas_exist(&[&ui_schema_path, &permission_schema_path])?;

    validate_locale_documents_schema(&ui_schema_path, &locale_documents)?;
    let permission_cli_dir = config.input.join(PERMISSION_CLI_DIR);
    validate_permission_documents_schema(&permission_schema_path, &permission_cli_dir)?;
    validate_permission_key_sets(&permission_cli_dir)?;

    // 2. Validate Consistency
    validate_keys(&i18n_map, &all_keys)?;
    let err_code_keys = collect_err_code_keys(&all_keys)?;
    if err_code_keys.is_empty() {
        return Err(anyhow!(
            "No `err_code_*` keys found in locale fragments. At least one business error code is required."
        ));
    }

    // 3. Generate Outputs
    if let Some(path) = &config.rust_out {
        generate_rust(path, &i18n_map, &all_keys, &err_code_keys)?;
    }
    if let Some(path) = &config.android_out {
        generate_android(path, &i18n_map)?;
    }
    if let Some(path) = &config.ios_out {
        generate_ios(path, &i18n_map)?;
    }
    if let Some(path) = &config.harmony_out {
        generate_harmony(path, &i18n_map)?;
    }
    if let Some(path) = &config.ts_out {
        generate_typescript(path, &all_keys, &err_code_keys)?;
    }

    Ok(())
}

fn collect_yaml_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "yaml") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn top_level_mapping_keys(
    path: &Path,
    yaml_value: &serde_yaml_ng::Value,
) -> Result<BTreeSet<String>> {
    let mapping = yaml_value
        .as_mapping()
        .ok_or_else(|| anyhow!("Locale file `{}` root must be a mapping", path.display()))?;
    let mut keys = BTreeSet::new();
    for key in mapping.keys() {
        let key_str = key.as_str().ok_or_else(|| {
            anyhow!(
                "Locale file `{}` contains non-string top-level key",
                path.display()
            )
        })?;
        keys.insert(key_str.to_string());
    }
    Ok(keys)
}

fn validate_ui_locale_boundary(path: &Path, yaml_value: &serde_yaml_ng::Value) -> Result<()> {
    let keys = top_level_mapping_keys(path, yaml_value)?;
    let mut forbidden = Vec::new();
    if keys.contains("error") {
        forbidden.push("error");
    }
    if keys.contains("err_code") {
        forbidden.push("err_code");
    }
    if forbidden.is_empty() {
        return Ok(());
    }
    Err(anyhow!(
        "UI locale file `{}` must not define [{}]. Move them to `error/<locale>.yaml`.",
        path.display(),
        forbidden.join(", ")
    ))
}

fn validate_error_locale_boundary(path: &Path, yaml_value: &serde_yaml_ng::Value) -> Result<()> {
    let keys = top_level_mapping_keys(path, yaml_value)?;
    let allowed = ["error", "err_code"]
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let invalid = keys.difference(&allowed).cloned().collect::<Vec<_>>();
    if !invalid.is_empty() {
        return Err(anyhow!(
            "Error locale file `{}` contains non-error sections [{}]. Only `error` and `err_code` are allowed.",
            path.display(),
            invalid.join(", ")
        ));
    }
    if !keys.contains("error") && !keys.contains("err_code") {
        return Err(anyhow!(
            "Error locale file `{}` must define at least one of `error` or `err_code`.",
            path.display()
        ));
    }
    Ok(())
}

fn validate_schemas_exist(schema_paths: &[&Path]) -> Result<()> {
    for path in schema_paths {
        if !path.exists() {
            return Err(anyhow!("Missing schema file: {}", path.display()));
        }
    }
    Ok(())
}

fn load_json_schema(schema_path: &Path) -> Result<Validator> {
    let content = fs::read_to_string(schema_path)
        .with_context(|| format!("Failed to read schema file {}", schema_path.display()))?;
    let schema: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Invalid JSON schema {}", schema_path.display()))?;
    jsonschema::options()
        .with_draft(Draft::Draft7)
        .build(&schema)
        .map_err(|error| {
            anyhow!(
                "Failed to compile schema {}: {}",
                schema_path.display(),
                error
            )
        })
}

fn yaml_to_json(value: &serde_yaml_ng::Value) -> Result<serde_json::Value> {
    serde_json::to_value(value).map_err(|error| anyhow!("Failed to convert YAML to JSON: {error}"))
}

fn validate_instance(
    schema: &Validator,
    schema_path: &Path,
    instance: &serde_json::Value,
) -> Result<()> {
    if !schema.is_valid(instance) {
        let details = schema
            .iter_errors(instance)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        return Err(anyhow!(
            "Schema validation failed for {}:\n{}",
            schema_path.display(),
            details.join("\n")
        ));
    }
    Ok(())
}

fn validate_locale_documents_schema(
    schema_path: &Path,
    locale_documents: &[(String, PathBuf, serde_yaml_ng::Value)],
) -> Result<()> {
    let schema = load_json_schema(schema_path)?;
    for (locale, path, yaml_value) in locale_documents {
        let json_value = yaml_to_json(yaml_value)?;
        validate_instance(&schema, schema_path, &json_value).with_context(|| {
            format!(
                "Locale file `{}` failed UI schema validation",
                path.display()
            )
        })?;
        if locale.trim().is_empty() {
            return Err(anyhow!(
                "Locale file `{}` has empty locale name",
                path.display()
            ));
        }
    }
    Ok(())
}

fn validate_permission_documents_schema(
    schema_path: &Path,
    permission_cli_dir: &Path,
) -> Result<()> {
    if !permission_cli_dir.exists() {
        return Err(anyhow!(
            "Missing CLI permission i18n directory: {}",
            permission_cli_dir.display()
        ));
    }

    let schema = load_json_schema(schema_path)?;
    let mut files = Vec::new();
    for entry in fs::read_dir(permission_cli_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "yaml") {
            files.push(path);
        }
    }

    if files.is_empty() {
        return Err(anyhow!(
            "No CLI permission locale YAML files found in {}",
            permission_cli_dir.display()
        ));
    }

    for path in files {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read permission file {}", path.display()))?;
        let yaml_value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)
            .with_context(|| format!("Invalid YAML in {}", path.display()))?;
        let json_value = yaml_to_json(&yaml_value)?;
        validate_instance(&schema, schema_path, &json_value).with_context(|| {
            format!(
                "Permission file `{}` failed permission schema validation",
                path.display()
            )
        })?;
    }

    Ok(())
}

fn validate_permission_key_sets(permission_cli_dir: &Path) -> Result<()> {
    let mut by_locale = BTreeMap::<String, BTreeSet<String>>::new();

    if !permission_cli_dir.exists() {
        return Err(anyhow!(
            "Missing CLI permission i18n directory: {}",
            permission_cli_dir.display()
        ));
    }

    for entry in fs::read_dir(permission_cli_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "yaml") {
            continue;
        }
        let locale = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| anyhow!("Invalid permission locale filename: {}", path.display()))?
            .to_string();
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read permission file {}", path.display()))?;
        let parsed: BTreeMap<String, String> = serde_yaml_ng::from_str(&content)
            .with_context(|| format!("Invalid permission YAML in {}", path.display()))?;
        by_locale.insert(locale, parsed.keys().cloned().collect::<BTreeSet<_>>());
    }

    let mut iter = by_locale.iter();
    let Some((base_locale, base_keys)) = iter.next() else {
        return Err(anyhow!(
            "No CLI permission locale YAML files found in {}",
            permission_cli_dir.display()
        ));
    };

    for (locale, keys) in iter {
        let missing_in_locale = base_keys.difference(keys).cloned().collect::<Vec<_>>();
        let missing_in_base = keys.difference(base_keys).cloned().collect::<Vec<_>>();
        if !missing_in_locale.is_empty() || !missing_in_base.is_empty() {
            return Err(anyhow!(
                "Permission key mismatch between `{}` and `{}`. Missing in `{}`: [{}]; missing in `{}`: [{}]",
                base_locale,
                locale,
                locale,
                missing_in_locale.join(", "),
                base_locale,
                missing_in_base.join(", "),
            ));
        }
    }

    Ok(())
}

fn flatten_yaml(value: &serde_yaml_ng::Value, prefix: Option<String>) -> Translations {
    let mut map = Translations::new();

    match value {
        serde_yaml_ng::Value::Mapping(m) => {
            // Check if this is a "Leaf with Overrides" (contains "default" key)
            if m.contains_key(serde_yaml_ng::Value::String("default".to_string())) {
                if let Some(p) = prefix {
                    let default = m
                        .get("default")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let android = m
                        .get("android")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let apple = m
                        .get("apple")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let ios = m.get("ios").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let harmony = m
                        .get("harmony")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let rust = m
                        .get("rust")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    map.insert(
                        p,
                        TranslationValue {
                            default,
                            android,
                            apple,
                            ios,
                            harmony,
                            rust,
                        },
                    );
                }
            } else {
                // Regular nesting
                for (k, v) in m {
                    // Handle both string and number keys
                    let key_str = match k {
                        serde_yaml_ng::Value::String(s) => s.clone(),
                        serde_yaml_ng::Value::Number(n) => n.to_string(),
                        _ => continue, // Skip unsupported key types
                    };
                    let new_prefix = match &prefix {
                        Some(p) => format!("{}_{}", p, key_str),
                        None => key_str,
                    };
                    map.extend(flatten_yaml(v, Some(new_prefix)));
                }
            }
        }
        serde_yaml_ng::Value::String(s) => {
            if let Some(p) = prefix {
                map.insert(p, TranslationValue::from_string(s.clone()));
            }
        }
        _ => {}
    }
    map
}

fn validate_keys(i18n_map: &I18nMap, all_keys: &BTreeMap<String, ()>) -> Result<()> {
    let mut mismatches = Vec::new();
    for (lang, translations) in i18n_map {
        for key in all_keys.keys() {
            if !translations.contains_key(key) {
                mismatches.push(format!("locale `{lang}` missing key `{key}`"));
            }
        }
    }
    if !mismatches.is_empty() {
        return Err(anyhow!(
            "i18n key mismatch detected:\n{}",
            mismatches.join("\n")
        ));
    }
    Ok(())
}

fn collect_err_code_keys(all_keys: &BTreeMap<String, ()>) -> Result<BTreeMap<u32, String>> {
    let mut out = BTreeMap::new();
    for key in all_keys.keys() {
        let Some(code_str) = key.strip_prefix("err_code_") else {
            continue;
        };
        if !code_str.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(anyhow!(
                "Invalid err_code key `{key}`: suffix must be digits"
            ));
        }
        let code = code_str
            .parse::<u32>()
            .map_err(|_| anyhow!("Invalid err_code key `{key}`: parse failed"))?;
        out.insert(code, key.clone());
    }
    Ok(out)
}

fn escape_rust_string(val: &str) -> String {
    val.replace("\\", "\\\\").replace("\"", "\\\"")
}

// --- Generators ---

fn generate_rust(
    out_path: &PathBuf,
    i18n_map: &I18nMap,
    all_keys: &BTreeMap<String, ()>,
    err_code_keys: &BTreeMap<u32, String>,
) -> Result<()> {
    let mut content = String::from("// Auto-generated by tools/i18n-gen. DO NOT EDIT.\n\n");

    content.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n");
    content.push_str("pub enum I18nKey {\n");

    for key in all_keys.keys() {
        let variant_name = key.to_pascal_case();
        content.push_str(&format!(
            "    {},
",
            variant_name
        ));
    }
    content.push_str("}\n\n");

    content.push_str("impl I18nKey {\n");
    content.push_str("    pub fn get(&self, locale: &str) -> &'static str {\n");
    content.push_str(
        "        let lang = locale.split('-').next().unwrap_or(locale).to_ascii_lowercase();\n",
    );
    content.push_str("        match (self, lang.as_str()) {\n");

    let mut supported_langs: BTreeSet<String> = BTreeSet::new();

    for (lang, translations) in i18n_map {
        let match_lang = lang
            .split('-')
            .next()
            .unwrap_or(lang.as_str())
            .to_ascii_lowercase();
        supported_langs.insert(match_lang.clone());

        for (key, val) in translations {
            let variant_name = key.to_pascal_case();

            let mut branches = String::new();

            if let Some(v) = &val.android {
                branches.push_str(&format!(
                    "if cfg!(target_os = \"android\") {{ \"{}\" }} else ",
                    escape_rust_string(v)
                ));
            }

            if let Some(v) = &val.harmony {
                branches.push_str(&format!(
                    "if cfg!(target_env = \"ohos\") {{ \"{}\" }} else ",
                    escape_rust_string(v)
                ));
            }

            if let Some(v) = val.get_explicit_apple() {
                branches.push_str(&format!(
                    "if cfg!(any(target_os = \"ios\", target_os = \"macos\")) {{ \"{}\" }} else ",
                    escape_rust_string(v)
                ));
            }

            if let Some(v) = &val.rust {
                branches.push_str(&format!("if cfg!(not(any(target_os = \"android\", target_env = \"ohos\", target_os = \"ios\", target_os = \"macos\"))) {{ \"{}\" }} else ", escape_rust_string(v)));
            }

            // If there are platform-specific branches, wrap final value in braces for else clause
            if branches.is_empty() {
                branches.push_str(&format!("\"{}\" ", escape_rust_string(&val.default)));
            } else {
                branches.push_str(&format!("{{ \"{}\" }}", escape_rust_string(&val.default)));
            }

            content.push_str(&format!(
                "            (I18nKey::{}, \"{}\") => {},\n",
                variant_name,
                match_lang,
                branches.trim()
            ));
        }
    }

    let supported_lang_list = supported_langs.into_iter().collect::<Vec<_>>().join(", ");
    content.push_str(&format!(
        "            _ => panic!(\"Unsupported locale language `{{}}`. Supported languages: {}\", lang),\n",
        escape_rust_string(&supported_lang_list)
    ));

    content.push_str("        }\n");
    content.push_str("    }\n\n");
    content.push_str("}\n");

    content.push_str("\npub fn err_code_key(code: u32) -> Option<I18nKey> {\n");
    content.push_str("    match code {\n");
    for (code, key) in err_code_keys {
        content.push_str(&format!(
            "        {} => Some(I18nKey::{}),\n",
            code,
            key.to_pascal_case()
        ));
    }
    content.push_str("        _ => None,\n");
    content.push_str("    }\n");
    content.push_str("}\n");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out_path, content)?;
    println!("Generated Rust: {:?}", out_path);
    Ok(())
}

fn escape_ts_string(val: &str) -> String {
    val.replace('\\', "\\\\").replace('"', "\\\"")
}

fn generate_typescript(
    out_dir: &PathBuf,
    all_keys: &BTreeMap<String, ()>,
    err_code_keys: &BTreeMap<u32, String>,
) -> Result<()> {
    fs::create_dir_all(out_dir).with_context(|| {
        format!(
            "Failed to create TypeScript output dir {}",
            out_dir.display()
        )
    })?;

    let mut i18n_content = String::from("// Auto-generated by lingxia-gen. DO NOT EDIT.\n\n");
    i18n_content.push_str("export const I18N_KEYS = [\n");
    for key in all_keys.keys() {
        i18n_content.push_str(&format!("  \"{}\",\n", escape_ts_string(key)));
    }
    i18n_content.push_str("] as const;\n\n");
    i18n_content.push_str("export type I18nKey = (typeof I18N_KEYS)[number];\n");

    let i18n_file = out_dir.join("i18n.ts");
    fs::write(&i18n_file, i18n_content)
        .with_context(|| format!("Failed to write {}", i18n_file.display()))?;
    println!("Generated TypeScript: {:?}", i18n_file);

    let mut error_content = String::from("// Auto-generated by lingxia-gen. DO NOT EDIT.\n\n");
    error_content.push_str("import type { I18nKey } from \"./i18n\";\n\n");
    error_content.push_str("export const ERR_CODE_INFO_BY_CODE = {\n");
    for (code, key) in err_code_keys {
        error_content.push_str(&format!(
            "  {}: {{ code: {}, key: \"{}\" }},\n",
            code,
            code,
            escape_ts_string(key)
        ));
    }
    error_content.push_str("} as const;\n\n");
    error_content.push_str(
        "export type LxErrorCode = (typeof ERR_CODE_INFO_BY_CODE)[keyof typeof ERR_CODE_INFO_BY_CODE][\"code\"];\n",
    );
    error_content.push_str("export interface LxErrorCodeInfo {\n");
    error_content.push_str("  readonly code: LxErrorCode;\n");
    error_content.push_str("  readonly key: I18nKey;\n");
    error_content.push_str("}\n\n");

    let error_file = out_dir.join("error.ts");
    fs::write(&error_file, error_content)
        .with_context(|| format!("Failed to write {}", error_file.display()))?;
    println!("Generated TypeScript: {:?}", error_file);

    Ok(())
}

fn generate_android(base_res_dir: &Path, i18n_map: &I18nMap) -> Result<()> {
    for (lang, translations) in i18n_map {
        let dirname = if lang == "en-US" {
            "values".to_string()
        } else if lang == "zh-CN" {
            "values-zh-rCN".to_string()
        } else {
            format!("values-{}", lang)
        };

        let dir_path = base_res_dir.join(dirname);
        fs::create_dir_all(&dir_path)?;
        let file_path = dir_path.join("strings.xml");

        let mut content = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n");

        for (key, val) in translations {
            let res_name = format!("lx_{}", key);
            let target_val = val.get_for_android();

            let escaped_val = target_val
                .replace("'", "'\'")
                .replace("\"", "\\\"")
                .replace("&", "&amp;")
                .replace("<", "&lt;")
                .replace(">", "&gt;");

            content.push_str(&format!(
                "    <string name=\"{}\">{}</string>\n",
                res_name, escaped_val
            ));
        }
        content.push_str("</resources>");

        fs::write(&file_path, content)?;
        println!("Generated Android: {:?}", file_path);
    }
    Ok(())
}

fn generate_ios(base_dir: &Path, i18n_map: &I18nMap) -> Result<()> {
    for (lang, translations) in i18n_map {
        let lproj_name = if lang == "en-US" {
            "en.lproj".to_string()
        } else if lang == "zh-CN" {
            "zh-Hans.lproj".to_string()
        } else {
            format!("{}.lproj", lang)
        };

        let dir_path = base_dir.join(lproj_name);
        fs::create_dir_all(&dir_path)?;
        let file_path = dir_path.join("Localizable.strings");

        let mut content = String::from("/* Auto-generated by tools/i18n-gen */\n\n");

        for (key, val) in translations {
            let res_key = format!("lx_{}", key);
            let target_val = val.get_for_apple();

            let escaped_val = escape_rust_string(target_val);
            content.push_str(&format!("\"{}\" = \"{}\";\n", res_key, escaped_val));
        }

        fs::write(&file_path, content)?;
        println!("Generated iOS: {:?}", file_path);
    }
    Ok(())
}

fn generate_harmony(base_dir: &Path, i18n_map: &I18nMap) -> Result<()> {
    for (lang, translations) in i18n_map {
        let dir_path = if lang == "en-US" {
            base_dir.join("base/element")
        } else if lang == "zh-CN" {
            base_dir.join("zh_CN/element")
        } else {
            base_dir.join(format!("{}/element", lang.replace("-", "_")))
        };

        fs::create_dir_all(&dir_path)?;
        let file_path = dir_path.join("string.json");

        let mut strings_array = Vec::new();
        for (key, val) in translations {
            let res_name = format!("lx_{}", key);
            let target_val = val.get_for_harmony();

            let mut obj = serde_json::Map::new();
            obj.insert("name".to_string(), serde_json::Value::String(res_name));
            obj.insert(
                "value".to_string(),
                serde_json::Value::String(target_val.clone()),
            );
            strings_array.push(serde_json::Value::Object(obj));
        }

        let mut root = serde_json::Map::new();
        root.insert(
            "string".to_string(),
            serde_json::Value::Array(strings_array),
        );

        let content = serde_json::to_string_pretty(&root)?;
        fs::write(&file_path, content)?;
        println!("Generated Harmony: {:?}", file_path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_file(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, content).expect("write test file");
    }

    fn base_config(input: &Path) -> I18nConfig {
        I18nConfig {
            input: input.to_path_buf(),
            rust_out: Some(input.join("out.rs")),
            android_out: None,
            ios_out: None,
            harmony_out: None,
            ts_out: None,
            schema_dir: None,
        }
    }

    fn write_test_schemas(root: &Path) {
        write_file(
            root,
            "schema/ui.schema.json",
            r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "minProperties": 1,
  "additionalProperties": {
    "anyOf": [
      { "type": "string" },
      {
        "type": "object",
        "properties": {
          "default": { "type": "string" },
          "android": { "type": "string" },
          "apple": { "type": "string" },
          "ios": { "type": "string" },
          "harmony": { "type": "string" },
          "rust": { "type": "string" }
        },
        "required": ["default"],
        "additionalProperties": false
      },
      {
        "type": "object",
        "minProperties": 1,
        "additionalProperties": {
          "anyOf": [
            { "type": "string" },
            {
              "type": "object",
              "properties": {
                "default": { "type": "string" },
                "android": { "type": "string" },
                "apple": { "type": "string" },
                "ios": { "type": "string" },
                "harmony": { "type": "string" },
                "rust": { "type": "string" }
              },
              "required": ["default"],
              "additionalProperties": false
            }
          ]
        }
      }
    ]
  }
}"#,
        );
        write_file(
            root,
            "schema/permission.schema.json",
            r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "minProperties": 1,
  "patternProperties": {
    "^apple\\.info_plist\\.[A-Za-z0-9_]+$": {
      "type": "string",
      "minLength": 1
    }
  },
  "additionalProperties": false
}"#,
        );
    }

    fn write_test_permissions(root: &Path) {
        write_file(
            root,
            "permission/cli/en-US.yaml",
            "apple.info_plist.NSCameraUsageDescription: \"camera\"\n",
        );
        write_file(
            root,
            "permission/cli/zh-CN.yaml",
            "apple.info_plist.NSCameraUsageDescription: \"相机\"\n",
        );
        write_file(
            root,
            "permission/runtime/en-US.yaml",
            "permission:\n  location_reason: \"Location permission required\"\n",
        );
        write_file(
            root,
            "permission/runtime/zh-CN.yaml",
            "permission:\n  location_reason: \"需要定位权限\"\n",
        );
    }

    fn write_test_error_locale(root: &Path, en_err_code: &str, zh_err_code: &str) {
        write_file(
            root,
            "error/en-US.yaml",
            &format!("error:\n  unknown: \"Unknown\"\nerr_code:\n{en_err_code}"),
        );
        write_file(
            root,
            "error/zh-CN.yaml",
            &format!("error:\n  unknown: \"未知\"\nerr_code:\n{zh_err_code}"),
        );
    }

    #[test]
    fn fails_when_locale_keys_mismatch() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(root, "ui/en-US.yaml", "common:\n  confirm: \"Confirm\"\n");
        write_file(root, "ui/zh-CN.yaml", "common:\n  cancel: \"取消\"\n");
        write_test_error_locale(root, "  1000: \"Unknown\"\n", "  1000: \"未知\"\n");

        let error = run(base_config(root)).expect_err("expected key mismatch");
        assert!(error.to_string().contains("i18n key mismatch"));
    }

    #[test]
    fn succeeds_with_err_code_only() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(root, "ui/en-US.yaml", "common:\n  confirm: \"Confirm\"\n");
        write_file(root, "ui/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_test_error_locale(root, "  1000: \"Unknown\"\n", "  1000: \"未知\"\n");

        run(base_config(root)).expect("generation should work");
    }

    #[test]
    fn fails_when_no_err_code_keys() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(root, "ui/en-US.yaml", "common:\n  confirm: \"Confirm\"\n");
        write_file(root, "ui/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_file(root, "error/en-US.yaml", "error:\n  unknown: \"Unknown\"\n");
        write_file(root, "error/zh-CN.yaml", "error:\n  unknown: \"未知\"\n");

        let error = run(base_config(root)).expect_err("expected missing err_code failure");
        assert!(error.to_string().contains("No `err_code_*` keys found"));
    }

    #[test]
    fn fails_when_schema_file_missing() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_file(root, "ui/en-US.yaml", "common:\n  confirm: \"Confirm\"\n");
        write_file(root, "ui/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_test_error_locale(root, "  1000: \"Unknown\"\n", "  1000: \"未知\"\n");
        write_test_permissions(root);

        let error = run(base_config(root)).expect_err("expected missing schema failure");
        assert!(error.to_string().contains("Missing schema file"));
    }

    #[test]
    fn fails_when_permission_file_violates_schema() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_file(root, "ui/en-US.yaml", "common:\n  confirm: \"Confirm\"\n");
        write_file(root, "ui/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_test_error_locale(root, "  1000: \"Unknown\"\n", "  1000: \"未知\"\n");
        write_test_permissions(root);
        write_file(root, "permission/cli/zh-CN.yaml", "invalid_key: \"相机\"\n");

        let error = run(base_config(root)).expect_err("expected permission schema failure");
        assert!(error.to_string().contains("permission schema validation"));
    }

    #[test]
    fn fails_when_ui_file_contains_error_sections() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(
            root,
            "ui/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\nerr_code:\n  1000: \"Unknown\"\n",
        );
        write_file(root, "ui/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_test_error_locale(root, "  1000: \"Unknown\"\n", "  1000: \"未知\"\n");

        let error = run(base_config(root)).expect_err("expected ui boundary failure");
        assert!(error.to_string().contains("must not define [err_code]"));
    }

    #[test]
    fn fails_when_error_locale_contains_ui_sections() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(root, "ui/en-US.yaml", "common:\n  confirm: \"Confirm\"\n");
        write_file(root, "ui/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_file(
            root,
            "error/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\nerr_code:\n  1000: \"Unknown\"\n",
        );
        write_file(
            root,
            "error/zh-CN.yaml",
            "error:\n  unknown: \"未知\"\nerr_code:\n  1000: \"未知\"\n",
        );

        let error = run(base_config(root)).expect_err("expected error locale boundary failure");
        assert!(error.to_string().contains("contains non-error sections"));
    }
}
