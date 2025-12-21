use anyhow::{Context, Result};
use clap::Args;
use inflector::Inflector;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone)]
pub struct I18nConfig {
    /// Path to the i18n source directory containing YAML files
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

pub fn run(config: I18nConfig) -> Result<()> {
    println!("Scanning for i18n files in: {:?}", config.input);

    let mut i18n_map: I18nMap = BTreeMap::new();
    let mut all_keys: BTreeMap<String, ()> = BTreeMap::new();

    // 1. Read and Parse YAML files
    for entry in fs::read_dir(&config.input)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "yaml") {
            let lang_code = path
                .file_stem()
                .context("No file stem")?
                .to_string_lossy()
                .to_string();
            println!("Found language: {}", lang_code);

            let content = fs::read_to_string(&path)?;
            let yaml_value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
            let flat_map = flatten_yaml(&yaml_value, None);

            for key in flat_map.keys() {
                all_keys.insert(key.clone(), ());
            }
            i18n_map.insert(lang_code, flat_map);
        }
    }

    // 2. Validate Consistency
    validate_keys(&i18n_map, &all_keys)?;

    // 3. Generate Outputs
    if let Some(path) = &config.rust_out {
        generate_rust(path, &i18n_map, &all_keys)?;
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
    for (lang, translations) in i18n_map {
        for key in all_keys.keys() {
            if !translations.contains_key(key) {
                println!("WARNING: Key '{}' missing in language '{}'", key, lang);
            }
        }
    }
    Ok(())
}

fn escape_rust_string(val: &str) -> String {
    val.replace("\\", "\\\\").replace("\"", "\\\"")
}

// --- Generators ---

fn generate_rust(
    out_path: &PathBuf,
    i18n_map: &I18nMap,
    all_keys: &BTreeMap<String, ()>,
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
    content.push_str("        let lang = locale.split('-').next().unwrap_or(\"en\");\n");
    content.push_str("        match (self, lang) {\n");

    let fallback_lang = "en-US";
    let fallback_map = i18n_map
        .get(fallback_lang)
        .or_else(|| i18n_map.values().next())
        .unwrap();

    for (lang, translations) in i18n_map {
        let match_lang = if lang.starts_with("zh") { "zh" } else { "en" };

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

    // Fallback for missing keys
    content.push_str("            (key, _) => match key {\n");
    for key in all_keys.keys() {
        let variant_name = key.to_pascal_case();
        let val = fallback_map
            .get(key)
            .cloned()
            .unwrap_or_else(|| TranslationValue::from_string("MISSING".to_string()));

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
            "                I18nKey::{} => {},\n",
            variant_name,
            branches.trim()
        ));
    }
    content.push_str("            }\n");

    content.push_str("        }\n");
    content.push_str("    }\n");
    content.push_str("}\n");

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out_path, content)?;
    println!("Generated Rust: {:?}", out_path);
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
