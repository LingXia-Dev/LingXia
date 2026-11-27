use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use inflector::Inflector;
use jsonschema::{Draft, Validator};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug, Clone, Default)]
pub struct I18nConfig {
    /// Path to the i18n source root directory (holds the scope dirs
    /// `shared/`, `error/`, `permission/runtime/`, `permission/cli/`,
    /// `logic/`, `desktop/`, `android/`, `apple/`, `harmony/` and `schema/`)
    #[arg(short, long, default_value = "i18n")]
    pub input: PathBuf,

    /// Verify-only mode: regenerate every enabled target into a tempdir and
    /// diff against the checked-in copy. Writes nothing on success and
    /// exits non-zero on drift. Run this in pre-commit / pre-push hooks to
    /// catch "forgot to regenerate after editing yaml".
    #[arg(long)]
    pub check: bool,

    /// Override the Rust generated-source path. Defaults to the workspace
    /// path used by `lingxia-logic`. Pass `--no-rust` to skip entirely.
    #[arg(long)]
    pub rust_out: Option<PathBuf>,

    /// Skip Rust generation.
    #[arg(long, conflicts_with = "rust_out")]
    pub no_rust: bool,

    /// Override Android `res/` output. Defaults to the workspace path used
    /// by the Android SDK. Pass `--no-android` to skip.
    #[arg(long)]
    pub android_out: Option<PathBuf>,

    #[arg(long, conflicts_with = "android_out")]
    pub no_android: bool,

    /// Override iOS / macOS `Resources/` output. Pass `--no-ios` to skip.
    #[arg(long)]
    pub ios_out: Option<PathBuf>,

    #[arg(long, conflicts_with = "ios_out")]
    pub no_ios: bool,

    /// Override Harmony `resources/` output. Pass `--no-harmony` to skip.
    #[arg(long)]
    pub harmony_out: Option<PathBuf>,

    #[arg(long, conflicts_with = "harmony_out")]
    pub no_harmony: bool,

    /// Override the View words module generated from `<input>/view/`. Pass
    /// `--no-view` to skip.
    #[arg(long)]
    pub view_out: Option<PathBuf>,

    #[arg(long, conflicts_with = "view_out")]
    pub no_view: bool,

    /// Override the Liuli words module generated from `<input>/liuli/`.
    #[arg(long)]
    pub liuli_out: Option<PathBuf>,

    #[arg(long, conflicts_with = "liuli_out")]
    pub no_liuli: bool,

    /// Path to JSON Schema directory (defaults to <input>/schema).
    #[arg(long)]
    pub schema_dir: Option<PathBuf>,

    /// Generate a component package's words instead of LingXia's: read
    /// `<dir>/messages/<language>.json` (`en` is the source, every key under one
    /// `@namespace.`) and write `<dir>/src/i18n/words.ts`. With `--check`, fail on drift.
    #[arg(long, conflicts_with_all = ["rust_out", "android_out", "ios_out", "harmony_out", "view_out", "liuli_out"])]
    pub package: Option<PathBuf>,
}

/// Workspace-relative defaults so the bare command
/// `cargo run -p lingxia-cli -- gen i18n` regenerates every target in place
/// without 6 separate `--*-out` flags.
const RUST_DEFAULT_OUT: &str = "crates/lingxia-logic/src/i18n_generated.rs";
const ANDROID_DEFAULT_OUT: &str = "lingxia-sdk/android/lingxia/src/main/res";
const APPLE_DEFAULT_OUT: &str = "lingxia-sdk/apple/Sources/Resources";
const HARMONY_DEFAULT_OUT: &str = "lingxia-sdk/harmony/lingxia/src/main/resources";
const VIEW_DEFAULT_OUT: &str = "packages/lingxia-view/src/generated/words.js";
const LIULI_DEFAULT_OUT: &str = "packages/liuli/core/src/i18n/words.ts";
/// The source catalogue each package ships and names in `lingxia.messages`, so an app's
/// overrides are checked against the version it installed.
const VIEW_MESSAGES_OUT: &str = "packages/lingxia-view/messages/en.json";
const LIULI_MESSAGES_OUT: &str = "packages/liuli/core/messages/en.json";
/// The View's and Liuli's own words, in the lxapp catalogue format; `en` is the source.
const VIEW_DIR: &str = "view";
const LIULI_DIR: &str = "liuli";
const PACKAGE_SOURCE: &str = "en";

/// Resolved generation plan for a single target.
struct OutputPlan {
    /// Where this run physically writes. In `--check` mode this is inside a
    /// tempdir; otherwise it's `expected`.
    write_to: PathBuf,
    /// Where the checked-in artifact normally lives. `--check` compares
    /// `write_to` against this path tree.
    expected: PathBuf,
    /// Whether `--check` should byte-compare the generated result against
    /// `expected`. Only the Rust and TS outputs are committed; the native
    /// resource bundles (Android `strings.xml`, Apple `Localizable.strings`,
    /// Harmony `string.json`) are gitignored and regenerated at build time,
    /// so for those targets `--check` only validates that generation
    /// succeeds — it must not demand a checked-in copy that never exists.
    compare_in_check: bool,
}

fn resolve_output_plan(
    user_path: Option<&Path>,
    no_flag: bool,
    workspace_default: &str,
    temp_subpath: &str,
    temp_root: Option<&Path>,
    compare_in_check: bool,
) -> Option<OutputPlan> {
    if no_flag {
        return None;
    }
    let expected = user_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(workspace_default));
    let write_to = match temp_root {
        Some(td) => td.join(temp_subpath),
        None => expected.clone(),
    };
    Some(OutputPlan {
        write_to,
        expected,
        compare_in_check,
    })
}

/// Byte-compare a single generated file against the checked-in copy and
/// push human-readable drift entries on mismatch.
fn compare_file(generated: &Path, expected: &Path, drift: &mut Vec<String>) -> Result<()> {
    let gen_bytes =
        fs::read(generated).with_context(|| format!("read generated {}", generated.display()))?;
    match fs::read(expected) {
        Ok(exp_bytes) if exp_bytes == gen_bytes => Ok(()),
        Ok(_) => {
            drift.push(format!("differs: {}", expected.display()));
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            drift.push(format!("missing on disk: {}", expected.display()));
            Ok(())
        }
        Err(e) => Err(anyhow!("read expected {}: {}", expected.display(), e)),
    }
}

/// Walk what the generator just produced and confirm every file matches
/// the checked-in copy. Files that exist on disk but were not produced
/// (e.g. unrelated Android `drawable/` resources sharing `res/`) are not
/// flagged — drift is one-directional: "did we produce content the repo
/// doesn't have yet?".
fn diff_against_expected(plan: &OutputPlan, drift: &mut Vec<String>) -> Result<()> {
    let generated = &plan.write_to;
    let expected = &plan.expected;
    if generated.is_file() || !generated.exists() {
        if generated.is_file() {
            compare_file(generated, expected, drift)?;
        }
        return Ok(());
    }
    let mut stack = vec![generated.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path.strip_prefix(generated).unwrap();
            compare_file(&path, &expected.join(rel), drift)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TranslationValue {
    pub(crate) default: String,
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
impl TranslationValue {
    fn map(&self, f: impl Fn(&str) -> Result<String>) -> Result<Self> {
        let opt = |v: &Option<String>| v.as_deref().map(&f).transpose();
        Ok(Self {
            default: f(&self.default)?,
            android: opt(&self.android)?,
            apple: opt(&self.apple)?,
            ios: opt(&self.ios)?,
            harmony: opt(&self.harmony)?,
            rust: opt(&self.rust)?,
        })
    }
}

/// How a platform writes an ICU argument: Rust keeps `{name}`, the native resources take positional
/// printf (`%1$s`, Apple `%1$@`), numbered in the source message's order so a translation may move them.
#[derive(Clone, Copy)]
pub(crate) enum ArgForm {
    Icu,
    Printf(&'static str),
}

/// Every value in the platform's form, each translation checked to keep its source's arguments.
fn platform_form(map: &I18nMap, form: ArgForm) -> Result<I18nMap> {
    let source = map.get(SOURCE_LOCALE);
    let mut out = I18nMap::new();
    let mut problems = Vec::new();
    for (locale, translations) in map {
        let mut converted = Translations::new();
        for (key, value) in translations {
            let order = source
                .and_then(|source| source.get(key))
                .map(|source| argument_order(&source.default))
                .transpose()
                .with_context(|| format!("{SOURCE_LOCALE} {key}"))?
                .unwrap_or_default();
            if let Some(source) = source.and_then(|source| source.get(key))
                && locale != SOURCE_LOCALE
            {
                let signature = |text: &str| {
                    lingxia_i18n::parse(text)
                        .map_err(|e| e.to_string())
                        .and_then(|m| m.signature())
                };
                match (signature(&source.default), signature(&value.default)) {
                    (Ok(want), Ok(got)) => {
                        for drift in lingxia_i18n::signature_drift(&want, &got) {
                            problems.push(format!("{locale} {key}: {drift}"));
                        }
                    }
                    (_, Err(error)) | (Err(error), _) => {
                        problems.push(format!("{locale} {key}: {error}"))
                    }
                }
            }
            let value = value
                .map(|text| render(text, &order, form))
                .with_context(|| format!("{locale} {key}"))?;
            converted.insert(key.clone(), value);
        }
        out.insert(locale.clone(), converted);
    }
    if !problems.is_empty() {
        bail!(
            "SDK strings do not keep their source's arguments:
  {}",
            problems.join(
                "
  "
            )
        );
    }
    Ok(out)
}

pub(crate) const SOURCE_LOCALE: &str = "en-US";

pub(crate) fn argument_order(text: &str) -> Result<Vec<String>> {
    let message = lingxia_i18n::parse(text).map_err(|e| anyhow!("{e}"))?;
    let mut order = Vec::new();
    for part in &message.parts {
        if let lingxia_i18n::Part::Arg(lingxia_i18n::Arg::Text(name)) = part
            && !order.contains(name)
        {
            order.push(name.clone());
        }
    }
    Ok(order)
}

pub(crate) fn render(text: &str, order: &[String], form: ArgForm) -> Result<String> {
    use lingxia_i18n::{Arg, Part};
    let message = lingxia_i18n::parse(text).map_err(|e| anyhow!("{e}"))?;
    let mut out = String::new();
    for part in &message.parts {
        match part {
            Part::Text(text) => match form {
                // A literal `%` beside printf arguments would read as one.
                ArgForm::Printf(_) if !order.is_empty() => out.push_str(&text.replace('%', "%%")),
                _ => out.push_str(text),
            },
            Part::Arg(Arg::Text(name)) => match form {
                ArgForm::Icu => out.push_str(&format!("{{{name}}}")),
                ArgForm::Printf(conversion) => {
                    let n = order.iter().position(|o| o == name).ok_or_else(|| {
                        anyhow!("`{{{name}}}` is not an argument of the source message")
                    })? + 1;
                    out.push_str(&format!("%{n}${conversion}"));
                }
            },
            _ => bail!("SDK strings take plain `{{name}}` arguments only"),
        }
    }
    Ok(out)
}

pub(crate) type Translations = BTreeMap<String, TranslationValue>;
type I18nMap = BTreeMap<String, Translations>;
// Scope -> locale -> flat translations.
type ScopedI18nMap = BTreeMap<Scope, I18nMap>;

/// Banner placed at the top of every auto-generated artifact. Points at
/// the actual invocation so grep-driven debugging lands on the right
/// command.
const GEN_HEADER_RUST: &str =
    "// Auto-generated by `cargo run -p lingxia-cli -- gen i18n`. DO NOT EDIT.\n\n";
const GEN_HEADER_C_STYLE: &str =
    "/* Auto-generated by `cargo run -p lingxia-cli -- gen i18n`. DO NOT EDIT. */\n\n";

const SHARED_LOCALE_DIR: &str = "shared";
const PERMISSION_RUNTIME_DIR: &str = "permission/runtime";
const PERMISSION_CLI_DIR: &str = "permission/cli";
const ERROR_DIR: &str = "error";
const LOGIC_DIR: &str = "logic";
const DESKTOP_DIR: &str = "desktop";
const ANDROID_DIR: &str = "android";
const APPLE_DIR: &str = "apple";
const HARMONY_DIR: &str = "harmony";

/// A YAML source bucket. The directory the file lives in determines who
/// consumes its strings — the generators only union the scopes that emit
/// to their respective target. Splitting the input by scope keeps native
/// resource files (Android XML / Apple lproj / Harmony JSON) and the
/// `i18n_generated.rs` baked into `lingxia-logic` from collecting strings
/// nobody on that platform reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    Shared,
    PermissionRuntime,
    Error,
    Logic,
    Desktop,
    Android,
    Apple,
    Harmony,
}

/// Output target. Each generator wires up to one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Rust,
    Ts,
    Android,
    Apple,
    Harmony,
}

impl std::fmt::Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/", self.dir_name())
    }
}

impl Scope {
    fn dir_name(self) -> &'static str {
        match self {
            Scope::Shared => SHARED_LOCALE_DIR,
            Scope::PermissionRuntime => PERMISSION_RUNTIME_DIR,
            Scope::Error => ERROR_DIR,
            Scope::Logic => LOGIC_DIR,
            Scope::Desktop => DESKTOP_DIR,
            Scope::Android => ANDROID_DIR,
            Scope::Apple => APPLE_DIR,
            Scope::Harmony => HARMONY_DIR,
        }
    }

    /// Whether this scope is mandatory at the input root. `shared/`, `error/`,
    /// and `permission/runtime/` define the baseline catalog; the rest are
    /// optional refinements.
    fn is_required(self) -> bool {
        matches!(
            self,
            Scope::Shared | Scope::PermissionRuntime | Scope::Error
        )
    }

    /// Schema files that documents in this scope must validate against.
    /// `None` means no JSON schema check (caller still runs structural
    /// validators).
    fn schema_basename(self) -> Option<&'static str> {
        match self {
            Scope::Shared | Scope::PermissionRuntime => Some("shared.schema.json"),
            // The error scope reuses the shared-leaf schema; its boundary check
            // (only `error` / `err_code` top-level keys) runs separately.
            Scope::Error => Some("shared.schema.json"),
            Scope::Logic | Scope::Desktop | Scope::Android | Scope::Apple | Scope::Harmony => {
                Some("native.schema.json")
            }
        }
    }

    /// Targets this scope feeds. Generators reverse-look this up via
    /// `scopes_feeding(target)`.
    fn targets(self) -> &'static [Target] {
        use Target::*;
        match self {
            // Cross-platform baseline: anything in shared/, permission/runtime/,
            // or error/ feeds every target. Native SDK code currently reads
            // `lx_permission_*_reason` and `lx_err_code_*` / `lx_error_*`
            // strings directly via R.string / Localizable, so these stay
            // emitted everywhere until the consumers migrate.
            Scope::Shared | Scope::PermissionRuntime | Scope::Error => {
                &[Rust, Ts, Android, Apple, Harmony]
            }
            // Rust + TS only: keys used by the logic crate (and surfaced to
            // JS via the bridge) but never referenced from native SDK code.
            Scope::Logic => &[Rust, Ts],
            // `desktop/` holds desktop-shell strings (terminal, browser
            // chrome) read by the Windows SDK (Rust/TS) and the macOS Swift
            // shell (Localizable.strings) — but not the mobile Android /
            // Harmony resource bundles.
            Scope::Desktop => &[Rust, Ts, Apple],
            // Single-platform native strings — nothing else needs them.
            Scope::Android => &[Android],
            Scope::Apple => &[Apple],
            Scope::Harmony => &[Harmony],
        }
    }
}

const ALL_SCOPES: &[Scope] = &[
    Scope::Shared,
    Scope::PermissionRuntime,
    Scope::Error,
    Scope::Logic,
    Scope::Desktop,
    Scope::Android,
    Scope::Apple,
    Scope::Harmony,
];

fn scopes_feeding(target: Target) -> Vec<Scope> {
    ALL_SCOPES
        .iter()
        .copied()
        .filter(|scope| scope.targets().contains(&target))
        .collect()
}

/// Build a flat (locale -> key -> value) view across the scopes that
/// emit to `target`. Cross-scope key collisions were already rejected at
/// load time, so this is a straightforward extend.
fn translations_for_target(scoped: &ScopedI18nMap, target: Target) -> I18nMap {
    let mut out: I18nMap = BTreeMap::new();
    for scope in scopes_feeding(target) {
        let Some(by_lang) = scoped.get(&scope) else {
            continue;
        };
        for (lang, translations) in by_lang {
            let bucket = out.entry(lang.clone()).or_default();
            for (key, value) in translations {
                bucket.insert(key.clone(), value.clone());
            }
        }
    }
    out
}

pub fn run(config: I18nConfig) -> Result<()> {
    if let Some(dir) = &config.package {
        return generate_package_words(dir, config.check);
    }
    println!("Scanning for i18n files in: {:?}", config.input);

    let schema_dir = config
        .schema_dir
        .clone()
        .unwrap_or_else(|| config.input.join("schema"));
    let shared_schema_path = schema_dir.join("shared.schema.json");
    let permission_schema_path = schema_dir.join("permission.schema.json");
    let native_schema_path = schema_dir.join("native.schema.json");
    validate_schemas_exist(&[
        &shared_schema_path,
        &permission_schema_path,
        &native_schema_path,
    ])?;

    // 1. Read + per-scope validate every locale fragment.
    let mut scoped: ScopedI18nMap = BTreeMap::new();
    // Where each flattened key first appeared. Used to fail loudly when a
    // key collides across scopes (e.g. someone copies a `shared/` key into
    // `android/` instead of moving it).
    let mut key_origin: BTreeMap<String, (Scope, PathBuf)> = BTreeMap::new();

    for &scope in ALL_SCOPES {
        let source_dir = config.input.join(scope.dir_name());
        if !source_dir.exists() {
            if scope.is_required() {
                return Err(anyhow!(
                    "Missing required locale i18n directory: {}",
                    source_dir.display()
                ));
            }
            continue;
        }

        let mut docs_in_scope: Vec<(String, PathBuf, serde_yaml_ng::Value)> = Vec::new();
        for path in collect_yaml_files(&source_dir)? {
            let lang_code = path
                .file_stem()
                .context("No file stem")?
                .to_string_lossy()
                .to_string();
            println!(
                "Found locale fragment: {} ({}) [{}]",
                lang_code,
                path.display(),
                scope
            );

            let content = fs::read_to_string(&path)?;
            let yaml_value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;

            // Structural boundary checks (top-level key set) per scope.
            match scope {
                Scope::Shared => validate_shared_locale_boundary(&path, &yaml_value)?,
                Scope::Error => validate_error_locale_boundary(&path, &yaml_value)?,
                Scope::PermissionRuntime
                | Scope::Logic
                | Scope::Desktop
                | Scope::Android
                | Scope::Apple
                | Scope::Harmony => {}
            }

            let flat_map = flatten_yaml(&yaml_value, None);

            let bucket = scoped
                .entry(scope)
                .or_default()
                .entry(lang_code.clone())
                .or_default();
            for (key, value) in flat_map {
                if bucket.insert(key.clone(), value).is_some() {
                    return Err(anyhow!(
                        "Duplicate i18n key `{}` in locale `{}` while loading `{}`",
                        key,
                        lang_code,
                        path.display()
                    ));
                }
                if let Some((other_scope, other_path)) = key_origin.get(&key) {
                    if *other_scope != scope {
                        return Err(anyhow!(
                            "Key `{}` declared in scope `{}` (`{}`) collides with scope `{}` (`{}`). Each key may live in exactly one scope directory.",
                            key,
                            other_scope,
                            other_path.display(),
                            scope,
                            path.display(),
                        ));
                    }
                } else {
                    key_origin.insert(key.clone(), (scope, path.clone()));
                }
            }

            docs_in_scope.push((lang_code, path, yaml_value));
        }

        // JSON-schema validation, per scope.
        if let Some(schema_basename) = scope.schema_basename() {
            let schema_path = schema_dir.join(schema_basename);
            validate_locale_documents_schema(&schema_path, &docs_in_scope)?;
        }
    }

    let permission_cli_dir = config.input.join(PERMISSION_CLI_DIR);
    validate_permission_documents_schema(&permission_schema_path, &permission_cli_dir)?;
    validate_permission_key_sets(&permission_cli_dir)?;

    // 2. Per-scope locale-key set consistency: within a scope, each locale
    // file must define the exact same key set. Cross-scope consistency
    // (key uniqueness) was already enforced at load time via key_origin.
    for (scope, by_lang) in &scoped {
        let mut all_keys_in_scope: BTreeMap<String, ()> = BTreeMap::new();
        for translations in by_lang.values() {
            for key in translations.keys() {
                all_keys_in_scope.insert(key.clone(), ());
            }
        }
        validate_keys_in_scope(*scope, by_lang, &all_keys_in_scope)?;
    }

    // 3. `err_code_*` keys must live in the error scope and nowhere else.
    let error_keys = scoped
        .get(&Scope::Error)
        .map(|by_lang| {
            let mut keys = BTreeMap::<String, ()>::new();
            for translations in by_lang.values() {
                for key in translations.keys() {
                    keys.insert(key.clone(), ());
                }
            }
            keys
        })
        .unwrap_or_default();
    for (key, (origin, path)) in &key_origin {
        if key.starts_with("err_code_") && *origin != Scope::Error {
            return Err(anyhow!(
                "`{}` is reserved for the error scope but was declared under scope `{}` (`{}`). Move it to `i18n/error/`.",
                key,
                origin,
                path.display(),
            ));
        }
    }
    let err_code_keys = collect_err_code_keys(&error_keys)?;
    if err_code_keys.is_empty() {
        return Err(anyhow!(
            "No `err_code_*` keys found in `i18n/error/`. At least one business error code is required."
        ));
    }

    if scoped.is_empty() {
        return Err(anyhow!(
            "No i18n locale YAML files found under {}",
            config.input.display()
        ));
    }

    // 4. Resolve output plans. In --check mode each plan's write_to is in a
    // tempdir; afterwards we diff write_to vs expected.
    let temp_root = if config.check {
        Some(tempfile::tempdir().context("create tempdir for --check")?)
    } else {
        None
    };
    let temp_path = temp_root.as_ref().map(|d| d.path());
    let rust_plan = resolve_output_plan(
        config.rust_out.as_deref(),
        config.no_rust,
        RUST_DEFAULT_OUT,
        "rust/i18n_generated.rs",
        temp_path,
        true,
    );
    let android_plan = resolve_output_plan(
        config.android_out.as_deref(),
        config.no_android,
        ANDROID_DEFAULT_OUT,
        "android",
        temp_path,
        false,
    );
    let ios_plan = resolve_output_plan(
        config.ios_out.as_deref(),
        config.no_ios,
        APPLE_DEFAULT_OUT,
        "apple",
        temp_path,
        false,
    );
    let harmony_plan = resolve_output_plan(
        config.harmony_out.as_deref(),
        config.no_harmony,
        HARMONY_DEFAULT_OUT,
        "harmony",
        temp_path,
        false,
    );

    let view_plan = resolve_output_plan(
        config.view_out.as_deref(),
        config.no_view || !config.input.join(VIEW_DIR).is_dir(),
        VIEW_DEFAULT_OUT,
        "view/words.js",
        temp_path,
        true,
    );
    let liuli_plan = resolve_output_plan(
        config.liuli_out.as_deref(),
        config.no_liuli || !config.input.join(LIULI_DIR).is_dir(),
        LIULI_DEFAULT_OUT,
        "liuli/words.ts",
        temp_path,
        true,
    );
    let view_messages_plan = resolve_output_plan(
        None,
        config.no_view || config.view_out.is_some() || !config.input.join(VIEW_DIR).is_dir(),
        VIEW_MESSAGES_OUT,
        "view/messages/en.json",
        temp_path,
        true,
    );
    let liuli_messages_plan = resolve_output_plan(
        None,
        config.no_liuli || config.liuli_out.is_some() || !config.input.join(LIULI_DIR).is_dir(),
        LIULI_MESSAGES_OUT,
        "liuli/messages/en.json",
        temp_path,
        true,
    );

    // 5. The Rust target's combined translations and key set.
    let rust_combined = rust_plan
        .is_some()
        .then(|| {
            platform_form(
                &translations_for_target(&scoped, Target::Rust),
                ArgForm::Icu,
            )
        })
        .transpose()?;
    let rust_keys = rust_combined.as_ref().map(|c| {
        c.values()
            .flat_map(|t| t.keys().cloned().map(|k| (k, ())))
            .collect::<BTreeMap<String, ()>>()
    });

    // 6. Run generators.
    if let Some(plan) = &rust_plan {
        generate_rust(
            &plan.write_to,
            rust_combined.as_ref().expect("rust_combined"),
            rust_keys.as_ref().expect("rust_keys"),
            &err_code_keys,
        )?;
        let view = config.input.join(VIEW_DIR);
        if view.is_dir() {
            let mut generated = fs::read_to_string(&plan.write_to)?;
            generated.push_str(&view_error_words_rust(&view)?);
            fs::write(&plan.write_to, generated)?;
        }
    }
    if let Some(plan) = &android_plan {
        let combined = platform_form(
            &translations_for_target(&scoped, Target::Android),
            ArgForm::Printf("s"),
        )?;
        generate_android(&plan.write_to, &combined)?;
    }
    if let Some(plan) = &ios_plan {
        let combined = platform_form(
            &translations_for_target(&scoped, Target::Apple),
            ArgForm::Printf("@"),
        )?;
        generate_ios(&plan.write_to, &combined)?;
    }
    if let Some(plan) = &harmony_plan {
        let combined = platform_form(
            &translations_for_target(&scoped, Target::Harmony),
            ArgForm::Printf("s"),
        )?;
        generate_harmony(&plan.write_to, &combined)?;
    }

    if let Some(plan) = &view_plan {
        generate_view_words(&config.input.join(VIEW_DIR), &plan.write_to)?;
    }
    if let Some(plan) = &liuli_plan {
        generate_liuli_words(&config.input.join(LIULI_DIR), &plan.write_to)?;
    }
    for (plan, dir) in [
        (&view_messages_plan, VIEW_DIR),
        (&liuli_messages_plan, LIULI_DIR),
    ] {
        if let Some(plan) = plan {
            let source = config
                .input
                .join(dir)
                .join(format!("{PACKAGE_SOURCE}.json"));
            write_module(&plan.write_to, fs::read_to_string(&source)?)?;
        }
    }

    // 7. Verify mode: diff generated tempdir against checked-in expected.
    if config.check {
        let mut drift = Vec::new();
        for plan in [
            &rust_plan,
            &android_plan,
            &ios_plan,
            &harmony_plan,
            &view_plan,
            &liuli_plan,
            &view_messages_plan,
            &liuli_messages_plan,
        ]
        .into_iter()
        .flatten()
        {
            if !plan.compare_in_check {
                continue;
            }
            diff_against_expected(plan, &mut drift)?;
        }
        if drift.is_empty() {
            println!("i18n outputs are in sync.");
        } else {
            eprintln!("i18n drift detected ({} file(s)):", drift.len());
            for entry in &drift {
                eprintln!("  {entry}");
            }
            return Err(anyhow!(
                "Checked-in i18n outputs are stale. Run `cargo run -p lingxia-cli -- gen i18n` to regenerate."
            ));
        }
    }

    Ok(())
}

/// `<dir>/<language>.json`, every key under `prefix`, each translation true to the `en` source.
/// A missing word falls back to English at run time.
fn package_catalogues(
    dir: &Path,
    prefix: &str,
) -> Result<BTreeMap<String, lingxia_i18n::Catalogue>> {
    let problems = |problems: Vec<lingxia_i18n::Problem>| {
        problems
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ")
    };
    let mut languages = BTreeMap::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(tag) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        let catalogue =
            lingxia_i18n::Catalogue::parse(&fs::read_to_string(&path)?, tag == PACKAGE_SOURCE)
                .map_err(|found| anyhow!("{}: {}", path.display(), problems(found)))?;
        languages.insert(tag.to_string(), catalogue);
    }
    let source = languages
        .get(PACKAGE_SOURCE)
        .ok_or_else(|| anyhow!("{}: no {PACKAGE_SOURCE}.json source", dir.display()))?;
    for (tag, catalogue) in &languages {
        if let Some(key) = catalogue
            .entries
            .keys()
            .find(|key| !key.starts_with(prefix))
        {
            bail!(
                "{}/{tag}.json: '{key}' is not a {prefix} key",
                dir.display()
            );
        }
        let (found, missing) = lingxia_i18n::check(source, catalogue);
        if !found.is_empty() {
            bail!("{}/{tag}.json: {}", dir.display(), problems(found));
        }
        // A language LingXia ships is complete; an app fills a gap of its own in its catalogue.
        if !missing.is_empty() {
            bail!(
                "{}/{tag}.json: untranslated: {}",
                dir.display(),
                missing.join(", ")
            );
        }
    }
    Ok(languages)
}

fn language_objects(languages: &BTreeMap<String, lingxia_i18n::Catalogue>) -> String {
    let mut out = String::new();
    for (tag, catalogue) in languages {
        let object = lingxia_i18n::js::module(catalogue);
        let object = object
            .strip_prefix("export default ")
            .and_then(|rest| rest.strip_suffix(";\n"))
            .expect("js::module is one default export");
        out.push_str(&format!("{tag:?}: {object},\n"));
    }
    out
}

fn write_module(out: &Path, module: String) -> Result<()> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, module)?;
    Ok(())
}

/// The View runtime's words (`@view.*`); they take no arguments, so no formatter.
fn generate_view_words(dir: &Path, out: &Path) -> Result<()> {
    let languages = package_catalogues(dir, "@view.")?;
    write_module(
        out,
        format!(
            "// Generated by `lingxia gen i18n` from i18n/view/. Do not edit.\n\n\
             export const source = {PACKAGE_SOURCE:?};\nexport const words = {{\n{}}};\n",
            language_objects(&languages)
        ),
    )
}

/// `view_error_word`: the View's `@view.error.*` words for Logic's `lx.ui.toast(e)` and `lx.ui.alert({ error })`, so both
/// layers say a failure the same way from one catalogue.
fn view_error_words_rust(dir: &Path) -> Result<String> {
    let languages = package_catalogues(dir, "@view.")?;
    let mut out = String::from(
        "\n#[rustfmt::skip]\n\
         /// A `@view.error.*` word (`network`, `failed`, …) in `language`'s base tag; `None` for a\n\
         /// language LingXia does not translate.\n\
         pub fn view_error_word(word: &str, language: &str) -> Option<&'static str> {\n    \
         let lang = language.split(['-', '_']).next().unwrap_or(language).to_ascii_lowercase();\n    \
         Some(match (word, lang.as_str()) {\n",
    );
    for (tag, catalogue) in &languages {
        for (key, entry) in &catalogue.entries {
            let Some(word) = key.strip_prefix("@view.error.") else {
                continue;
            };
            let text = lingxia_i18n::format::format(&entry.message, tag, &serde_json::Value::Null);
            out.push_str(&format!(
                "        ({word:?}, {tag:?}) => \"{}\",\n",
                escape_rust_string(&text)
            ));
        }
    }
    out.push_str("        _ => return None,\n    })\n}\n");
    Ok(out)
}

/// A package's `PackageWords` module, which `packageText` reads; `from` names its input.
fn package_words_module(
    languages: &BTreeMap<String, lingxia_i18n::Catalogue>,
    from: &str,
) -> String {
    format!(
        "// Generated by `lingxia gen i18n` from {from}. Do not edit.\n// @ts-nocheck\n\n\
         const languages = {{\n{}}};\n{}\n\
         export default {{ source: {PACKAGE_SOURCE:?}, languages, formatter }};\n",
        language_objects(languages),
        lingxia_i18n::js::formatter(&languages.keys().map(String::as_str).collect::<Vec<_>>())
            .replacen("export function formatter", "function formatter", 1)
    )
}

/// Liuli's words (`@liuli.*`).
fn generate_liuli_words(dir: &Path, out: &Path) -> Result<()> {
    let languages = package_catalogues(dir, "@liuli.")?;
    write_module(out, package_words_module(&languages, "i18n/liuli/"))
}

/// `--package <dir>`: `<dir>/messages/` to `<dir>/src/i18n/words.ts`, under the source's namespace.
fn generate_package_words(dir: &Path, check: bool) -> Result<()> {
    let messages = dir.join("messages");
    let source = messages.join(format!("{PACKAGE_SOURCE}.json"));
    let keys: serde_json::Map<String, serde_json::Value> = serde_json::from_str(
        &fs::read_to_string(&source).with_context(|| format!("read {}", source.display()))?,
    )
    .with_context(|| format!("{} is not a JSON object", source.display()))?;
    let prefix = keys
        .keys()
        .next()
        .and_then(|key| key.strip_prefix('@'))
        .and_then(|key| key.split_once('.'))
        .map(|(namespace, _)| format!("@{namespace}."))
        .ok_or_else(|| {
            anyhow!(
                "{}: a package's keys start with its `@namespace.`",
                source.display()
            )
        })?;
    let module = package_words_module(&package_catalogues(&messages, &prefix)?, "messages/");
    let out = dir.join("src/i18n/words.ts");
    if !check {
        write_module(&out, module)?;
        println!("Wrote {}", out.display());
        return Ok(());
    }
    if fs::read_to_string(&out).ok().as_deref() != Some(module.as_str()) {
        bail!(
            "{} is stale; run `lingxia gen i18n --package {}`",
            out.display(),
            dir.display()
        );
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

fn validate_shared_locale_boundary(path: &Path, yaml_value: &serde_yaml_ng::Value) -> Result<()> {
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
        "Shared locale file `{}` must not define [{}]. Move them to `error/<locale>.yaml`.",
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
                "Locale file `{}` failed shared-leaf schema validation",
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
        if path.extension().is_none_or(|ext| ext != "yaml") {
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

pub(crate) fn flatten_yaml(value: &serde_yaml_ng::Value, prefix: Option<String>) -> Translations {
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

fn validate_keys_in_scope(
    scope: Scope,
    i18n_map: &I18nMap,
    all_keys: &BTreeMap<String, ()>,
) -> Result<()> {
    let mut mismatches = Vec::new();
    for (lang, translations) in i18n_map {
        for key in all_keys.keys() {
            if !translations.contains_key(key) {
                mismatches.push(format!(
                    "scope `{scope}` / locale `{lang}` missing key `{key}`"
                ));
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
    let mut content = String::from(GEN_HEADER_RUST);

    // Every top-level item carries `#[rustfmt::skip]`: this file is emitted
    // raw (unformatted long match arms), so `cargo fmt --check` would flag it
    // and `gen i18n --check` would report spurious drift against a fmt'd copy.
    // Skipping keeps the generator self-sufficient — no `rustfmt` binary
    // required at generation time (CI check/test toolchains lack the
    // component), and output is byte-stable across rustfmt versions.
    content.push_str("#[rustfmt::skip]\n");
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

    content.push_str("#[rustfmt::skip]\n");
    content.push_str("impl I18nKey {\n");
    content.push_str("    /// The key as `i18n/` names it, flat: `file_chooser_filter_label`.\n");
    content.push_str("    pub fn name(&self) -> &'static str {\n        match self {\n");
    for key in all_keys.keys() {
        content.push_str(&format!(
            "            I18nKey::{} => \"{key}\",\n",
            key.to_pascal_case()
        ));
    }
    content.push_str("        }\n    }\n}\n\n");

    content.push_str("#[rustfmt::skip]\n");
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

    content.push_str("            _ => self.get(\"en\"),\n");

    content.push_str("        }\n");
    content.push_str("    }\n\n");
    content.push_str("}\n");

    content.push_str("\n#[rustfmt::skip]\npub fn err_code_key(code: u32) -> Option<I18nKey> {\n");
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

            // aapt string escaping: apostrophes and quotes take a backslash;
            // XML entities cover the markup characters.
            let escaped_val = target_val
                .replace('\\', "\\\\")
                .replace('\'', "\\'")
                .replace('"', "\\\"")
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");

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
        let lproj_name =
            crate::product_i18n::apple_lproj_dir(lang).unwrap_or_else(|| format!("{lang}.lproj"));

        let dir_path = base_dir.join(lproj_name);
        fs::create_dir_all(&dir_path)?;
        let file_path = dir_path.join("Localizable.strings");

        let mut content = String::from(GEN_HEADER_C_STYLE);

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

    #[test]
    fn printf_arguments_are_numbered_in_the_source_order_and_must_survive_translation() {
        let map = |zh: &str| -> I18nMap {
            let one = |text: &str| {
                Translations::from([("k".to_string(), TranslationValue::from_string(text.into()))])
            };
            I18nMap::from([
                ("en-US".to_string(), one("{app} {version} 100%")),
                ("zh-CN".to_string(), one(zh)),
            ])
        };
        let apple = platform_form(&map("{version} 的 {app}"), ArgForm::Printf("@")).unwrap();
        assert_eq!(apple["en-US"]["k"].default, "%1$@ %2$@ 100%%");
        assert_eq!(apple["zh-CN"]["k"].default, "%2$@ 的 %1$@");
        let error = platform_form(&map("{app}"), ArgForm::Printf("s")).unwrap_err();
        assert!(format!("{error:#}").contains("zh-CN k"), "{error:#}");
    }

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
            // Skip every other target so tests don't touch the workspace.
            no_android: true,
            no_ios: true,
            no_harmony: true,
            ..I18nConfig::default()
        }
    }

    fn write_test_schemas(root: &Path) {
        write_file(
            root,
            "schema/shared.schema.json",
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
        write_file(
            root,
            "schema/native.schema.json",
            r##"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "minProperties": 1,
  "additionalProperties": {
    "$ref": "#/definitions/node"
  },
  "definitions": {
    "node": {
      "anyOf": [
        { "type": "string", "minLength": 1 },
        {
          "type": "object",
          "minProperties": 1,
          "additionalProperties": { "$ref": "#/definitions/node" }
        }
      ]
    }
  }
}"##,
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
    fn a_package_generates_its_words_under_its_namespace_and_checks_for_drift() {
        let temp = TempDir::new().expect("temp dir");
        let dir = temp.path();
        write_file(
            dir,
            "messages/en.json",
            r#"{"@demo.hello":{"defaultMessage":"Hello {name}","description":"A greeting."}}"#,
        );
        write_file(
            dir,
            "messages/zh.json",
            r#"{"@demo.hello":{"defaultMessage":"你好{name}"}}"#,
        );
        let out = dir.join("src/i18n/words.ts");
        assert!(
            generate_package_words(dir, true).is_err(),
            "nothing generated yet"
        );
        generate_package_words(dir, false).expect("generate");
        let words = fs::read_to_string(&out).unwrap();
        assert!(words.starts_with("// Generated by `lingxia gen i18n` from messages/."));
        assert!(words.contains("\"@demo.hello\"") && words.contains("\"zh\""));
        generate_package_words(dir, true).expect("in step");
        fs::write(&out, format!("{words}// edited\n")).unwrap();
        let stale = generate_package_words(dir, true).expect_err("drift");
        assert!(stale.to_string().contains("is stale"), "{stale}");
        write_file(
            dir,
            "messages/zh.json",
            r#"{"@other.hello":{"defaultMessage":"你好"}}"#,
        );
        let foreign = generate_package_words(dir, false).expect_err("foreign key");
        assert!(
            foreign.to_string().contains("is not a @demo. key"),
            "{foreign}"
        );
    }

    #[test]
    fn fails_when_locale_keys_mismatch() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(
            root,
            "shared/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\n",
        );
        write_file(root, "shared/zh-CN.yaml", "common:\n  cancel: \"取消\"\n");
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
        write_file(
            root,
            "shared/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\n",
        );
        write_file(root, "shared/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_test_error_locale(root, "  1000: \"Unknown\"\n", "  1000: \"未知\"\n");

        run(base_config(root)).expect("generation should work");
    }

    #[test]
    fn fails_when_no_err_code_keys() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(
            root,
            "shared/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\n",
        );
        write_file(root, "shared/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_file(root, "error/en-US.yaml", "error:\n  unknown: \"Unknown\"\n");
        write_file(root, "error/zh-CN.yaml", "error:\n  unknown: \"未知\"\n");

        let error = run(base_config(root)).expect_err("expected missing err_code failure");
        assert!(error.to_string().contains("No `err_code_*` keys found"));
    }

    #[test]
    fn fails_when_schema_file_missing() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_file(
            root,
            "shared/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\n",
        );
        write_file(root, "shared/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
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
        write_file(
            root,
            "shared/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\n",
        );
        write_file(root, "shared/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
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
            "shared/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\nerr_code:\n  1000: \"Unknown\"\n",
        );
        write_file(root, "shared/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
        write_test_error_locale(root, "  1000: \"Unknown\"\n", "  1000: \"未知\"\n");

        let error = run(base_config(root)).expect_err("expected shared boundary failure");
        assert!(error.to_string().contains("must not define [err_code]"));
    }

    /// The repository's own `i18n/` generates for every target and matches the committed Rust
    /// output: a merge that puts one key in two scopes fails here, not on a
    /// fresh checkout's first host build.
    #[test]
    fn the_repository_strings_generate_for_every_target() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        run(I18nConfig {
            input: root.join("i18n"),
            check: true,
            rust_out: Some(root.join(RUST_DEFAULT_OUT)),
            android_out: Some(root.join(ANDROID_DEFAULT_OUT)),
            ios_out: Some(root.join(APPLE_DEFAULT_OUT)),
            harmony_out: Some(root.join(HARMONY_DEFAULT_OUT)),
            view_out: Some(root.join(VIEW_DEFAULT_OUT)),
            liuli_out: Some(root.join(LIULI_DEFAULT_OUT)),
            ..I18nConfig::default()
        })
        .expect("i18n/ must generate cleanly; run `lingxia gen i18n` and read its error");
    }

    #[test]
    fn fails_when_error_locale_contains_ui_sections() {
        let temp = TempDir::new().expect("temp dir");
        let root = temp.path();
        write_test_schemas(root);
        write_test_permissions(root);
        write_file(
            root,
            "shared/en-US.yaml",
            "common:\n  confirm: \"Confirm\"\n",
        );
        write_file(root, "shared/zh-CN.yaml", "common:\n  confirm: \"确定\"\n");
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
