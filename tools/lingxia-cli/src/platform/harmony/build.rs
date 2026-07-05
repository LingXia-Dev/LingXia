use super::{HarmonyPlatform, OHOS_TARGET, deploy::ensure_command};
use crate::commands::rust::run_cargo_build_for_target;
use crate::platform::{
    BuildArtifacts, BuildConfig, BuildProfile, lingxia_workspace_root,
    native_client_out_for_host_project, resolve_cargo_target_dir, resolve_lingxia_target_dir,
    set_native_client_codegen_env,
};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const HMOS_CMDLINE_TOOLS_URL: &str =
    "https://developer.huawei.com/consumer/en/download/command-line-tools-for-hmos";

impl HarmonyPlatform {
    fn detect_ohos_ndk() -> Result<PathBuf> {
        if let Ok(value) = env::var("OHOS_NDK_HOME") {
            let path = PathBuf::from(&value);
            if !path.exists() {
                return Err(anyhow!(
                    "OHOS_NDK_HOME is set to '{}' but path does not exist",
                    value
                ));
            }

            if path.join("native").exists() {
                return Ok(path);
            }

            return Err(anyhow!(
                "OHOS_NDK_HOME='{}' is not a valid Harmony SDK root (missing native/ directory)",
                value
            ));
        }

        Err(anyhow!(
            "Harmony SDK environment variable not set.\n\
             Set OHOS_NDK_HOME to Harmony command-line tools SDK root.\n\
             Download: {}\n\
             Example: export OHOS_NDK_HOME=$HOME/OpenHarmony/command-line-tools/sdk/default/openharmony",
            HMOS_CMDLINE_TOOLS_URL
        ))
    }

    pub(super) fn build_impl(
        &self,
        config: &BuildConfig,
        harmony_dir: &Path,
    ) -> Result<BuildArtifacts> {
        // Mirror the Harmony project into a per-env staging directory so the
        // user's source tree is never mutated. ohpm install, the env-version
        // bundleName rewrite, and hvigor all run inside the staging copy; a
        // SIGKILL or hard exit during build can no longer leave the source in
        // a partially-modified state.
        let staging = prepare_harmony_staging(harmony_dir, config)?;

        // External user projects don't have the Harmony SDK in their source
        // tree, so fetch the published HAR (verified, cached) and wire it into
        // the STAGED entry/oh-package.json5 as an absolute `file:` dependency.
        // We inject on the staging copy (after prepare_harmony_staging, before
        // ohpm install) because `rewrite_file_dependencies` only rewrites
        // RELATIVE `file:` deps — an absolute path injected here is left
        // untouched. Inside the workspace the committed oh-package.json5 already
        // references the local Harmony SDK source.
        match lingxia_workspace_root(&config.project_root) {
            // External user projects don't have the SDK in their tree: wire in
            // the published HAR, whose rawfile icons were generated at release.
            None => {
                let version = crate::sdk_cache::sdk_version();
                let har =
                    crate::sdk_cache::ensure_sdk(crate::sdk_cache::SdkPlatform::Harmony, &version)?;
                inject_harmony_har_dependency(&staging, &har)?;
            }
            // In-workspace builds compile the SDK source module in place. Its
            // `resources/rawfile/icons` are generated (gitignored), so without a
            // prior release-script run they can be stale or missing. Regenerate
            // the design icons into the source module before hvigor packages it.
            Some(workspace_root) => {
                stage_sdk_design_icons(&workspace_root)?;
            }
        }

        if config.build_native {
            let so_path = self.build_rust_library(&config.project_root, config)?;
            self.stage_native_library(&so_path, &staging)?;
        } else {
            println!(
                "  {} Skipping native compilation (using existing .so)",
                "⏭️".dimmed()
            );
        }

        if config.native_only {
            // CI build-verification: the ohos cross-compile is the goal. The
            // .hap needs `hvigor assembleHap`, which requires the gated API-21
            // HarmonyOS SDK, so stop at the staged native library.
            let so = staging.join("entry/libs/arm64-v8a/liblingxia.so");
            println!(
                "  {} --native-only: native library built; skipping ohpm + hvigor",
                "⏭️".dimmed()
            );
            return Ok(BuildArtifacts::Harmony { hap_path: so });
        }

        self.ohpm_install(&staging)?;
        let hap_path = self.build_hap(&staging, config)?;

        Ok(BuildArtifacts::Harmony { hap_path })
    }
    fn build_rust_library(&self, project_root: &Path, config: &BuildConfig) -> Result<PathBuf> {
        println!("{}", "Compiling native code (HarmonyOS)...".cyan());

        let ndk_path = Self::detect_ohos_ndk()?;
        let lingxia_config = config
            .lingxia_config
            .as_ref()
            .ok_or_else(|| anyhow!("lingxia.yaml is required to build native libraries"))?;

        let rust_lib_name = lingxia_config
            .get_rust_lib_name()
            .ok_or_else(|| anyhow!("app.projectName is required in lingxia.yaml"))?;
        let rust_lib_dir = project_root.join(&rust_lib_name);
        let rust_manifest = rust_lib_dir.join("Cargo.toml");
        if !rust_manifest.exists() {
            return Err(anyhow!(
                "Rust library manifest not found: {}",
                rust_manifest.display()
            ));
        }

        let (crate_name, lib_name) = parse_crate_and_lib_name(&rust_manifest)?;

        let llvm_bin = ndk_path.join("native/llvm/bin");
        let sysroot = ndk_path.join("native/sysroot");

        let linker = llvm_bin.join("aarch64-unknown-linux-ohos-clang");
        let ar = llvm_bin.join("llvm-ar");
        let cc = llvm_bin.join("aarch64-unknown-linux-ohos-clang");
        let cxx = llvm_bin.join("aarch64-unknown-linux-ohos-clang++");

        let cpath = format!(
            "{}:{}",
            sysroot.join("usr/include").display(),
            sysroot.join("usr/include/aarch64-linux-ohos").display()
        );
        let bindgen_args = format!(
            "--sysroot={} -I{} -I{}",
            sysroot.display(),
            sysroot.join("usr/include").display(),
            sysroot.join("usr/include/aarch64-linux-ohos").display()
        );

        let target_dir = resolve_cargo_target_dir(project_root);
        let native_client_out =
            native_client_out_for_host_project(project_root, lingxia_config, config.framework)?;
        run_cargo_build_for_target(
            &rust_manifest,
            &rust_lib_dir,
            &target_dir,
            OHOS_TARGET,
            Some(&crate_name),
            config.profile,
            |cmd| {
                if !config.native_default_features {
                    cmd.arg("--no-default-features");
                }
                set_native_client_codegen_env(cmd, native_client_out.as_deref());
                if !config.native_features.is_empty() {
                    cmd.arg("--features").arg(config.native_features.join(","));
                }

                let target_env = OHOS_TARGET.replace('-', "_");
                let target_upper = OHOS_TARGET.to_uppercase().replace('-', "_");
                cmd.env(format!("CARGO_TARGET_{}_LINKER", target_upper), &linker);
                cmd.env(format!("AR_{}", target_env), &ar);
                cmd.env(format!("CC_{}", target_env), &cc);
                cmd.env(format!("CXX_{}", target_env), &cxx);
                cmd.env("CPATH", &cpath);
                cmd.env("BINDGEN_EXTRA_CLANG_ARGS", &bindgen_args);

                cmd.env_remove("SDKROOT");
                cmd.env_remove("MACOSX_DEPLOYMENT_TARGET");
            },
        )?;

        let profile_dir = config.profile.as_str();
        let so_file_name = format!("lib{lib_name}.so");
        let so_path = target_dir
            .join(OHOS_TARGET)
            .join(profile_dir)
            .join(&so_file_name);
        if !so_path.exists() {
            return Err(anyhow!("Built .so not found at: {}", so_path.display()));
        }

        println!("  {} Rust build complete", "✓".green());
        Ok(so_path)
    }

    fn stage_native_library(&self, so_path: &Path, harmony_dir: &Path) -> Result<()> {
        let dest_dir = harmony_dir.join("entry/libs/arm64-v8a");
        std::fs::create_dir_all(&dest_dir)
            .with_context(|| format!("Failed to create {}", dest_dir.display()))?;

        let dest = dest_dir.join("liblingxia.so");
        std::fs::copy(so_path, &dest)
            .with_context(|| format!("Failed to copy .so to {}", dest.display()))?;

        println!(
            "  {} Native library staged: {}",
            "✓".green(),
            dest.display()
        );
        Ok(())
    }

    fn ohpm_install(&self, harmony_dir: &Path) -> Result<()> {
        println!("{}", "Installing ohpm dependencies...".cyan());
        let ohpm = ensure_command("ohpm")?;

        let status = Command::new(&ohpm)
            .arg("install")
            .current_dir(harmony_dir.join("entry"))
            .status()
            .context("Failed to execute ohpm install")?;

        if !status.success() {
            return Err(anyhow!("ohpm install failed"));
        }

        println!("  {} ohpm install complete", "✓".green());
        Ok(())
    }

    fn build_hap(&self, harmony_dir: &Path, config: &BuildConfig) -> Result<PathBuf> {
        println!("{}", "Building HAP...".cyan());
        let hvigorw = ensure_command("hvigorw")?;

        let status = Command::new(&hvigorw)
            .arg("assembleHap")
            .arg("--no-daemon")
            .current_dir(harmony_dir)
            .status()
            .context("Failed to execute hvigorw assembleHap")?;

        if !status.success() {
            return Err(anyhow!("hvigorw assembleHap failed"));
        }

        let unsigned =
            harmony_dir.join("entry/build/default/outputs/default/entry-default-unsigned.hap");
        if unsigned.exists() {
            println!("  {} HAP built (unsigned)", "✓".green());
            return self.sign_hap_after_build(unsigned, &config.project_root, config.profile);
        }

        let signed =
            harmony_dir.join("entry/build/default/outputs/default/entry-default-signed.hap");
        if signed.exists() {
            println!("  {} HAP built (pre-signed by build tool)", "✓".green());
            return Ok(signed);
        }

        Err(anyhow!(
            "HAP not found after build. Expected at: {}",
            unsigned.display()
        ))
    }

    fn sign_hap_after_build(
        &self,
        unsigned_hap: PathBuf,
        project_root: &Path,
        build_profile: BuildProfile,
    ) -> Result<PathBuf> {
        self.sign_hap_with_project_config(&unsigned_hap, project_root, build_profile)
    }
}

/// Mirror the Harmony source project into a per-env staging directory and
/// rewrite `AppScope/app.json5`'s `bundleName` with the env-version suffix.
///
/// Harmony's hvigor toolchain has no build-time injection point for
/// `bundleName` — it reads `app.json5` directly. Earlier versions wrote the
/// effective name into the source file and tried to restore it on Drop, which
/// left the source tree dirty during the build (visible in `git status`) and
/// could leak the suffix on SIGKILL. We now mirror the whole project into
/// `target/lingxia/harmony/build/<env>/` and operate exclusively on the copy, so the source
/// tree is never mutated regardless of how the build terminates.
///
/// Excludes from the mirror: `.lingxia/` (generated dev state),
/// `oh_modules/` (re-installed inside staging), and `build/` (regenerated).
///
/// After mirroring, `oh-package.json5` `file:` dependencies are rewritten so
/// the staging-relative path still resolves to the original target. Without
/// this step, `file:../../../foo` written from `<source>/entry/` would point
/// inside the staging tree once read from
/// `<target>/lingxia/harmony/build/<env>/entry/`.
fn prepare_harmony_staging(source: &Path, config: &BuildConfig) -> Result<PathBuf> {
    let staging = resolve_lingxia_target_dir(&config.project_root)
        .join("harmony")
        .join("build")
        .join(config.resolved_env.version.as_str());
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .with_context(|| format!("Failed to clean {}", staging.display()))?;
    }
    const SKIP: &[&str] = &[".lingxia", "oh_modules", "build"];
    copy_dir_recursive_excluding(source, &staging, SKIP)
        .with_context(|| format!("Failed to mirror Harmony project to {}", staging.display()))?;
    rewrite_staged_source_paths(&staging, source)?;
    rewrite_app_bundle_name(&staging, config)?;
    Ok(staging)
}

/// Walk staging and rewrite every `oh-package.json5` so relative `file:`
/// dependencies still resolve to the original source-tree target.
fn rewrite_staged_source_paths(staging: &Path, source: &Path) -> Result<()> {
    walk_and_rewrite_oh_packages(staging, staging, source)
}

fn walk_and_rewrite_oh_packages(dir: &Path, staging: &Path, source: &Path) -> Result<()> {
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            walk_and_rewrite_oh_packages(&path, staging, source)?;
        } else if ft.is_file() {
            let name = entry.file_name();
            let name_os = name.as_os_str();
            let rel_parent = path
                .parent()
                .unwrap_or(staging)
                .strip_prefix(staging)
                .unwrap_or_else(|_| Path::new(""));
            let source_parent = source.join(rel_parent);
            if name_os == std::ffi::OsStr::new("oh-package.json5") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let rewritten = rewrite_file_dependencies(&content, &source_parent);
                if rewritten != content {
                    std::fs::write(&path, rewritten)
                        .with_context(|| format!("Failed to write {}", path.display()))?;
                }
            } else if name_os == std::ffi::OsStr::new("build-profile.json5") {
                // build-profile.json5 declares hvigor `modules[].srcPath`; when a
                // module's source lives outside the project root (e.g. SDK source
                // dep via `../..` segments) the path needs the same staging
                // prefix as `file:` deps in oh-package.json5.
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let rewritten = rewrite_build_profile_src_paths(&content, &source_parent);
                if rewritten != content {
                    std::fs::write(&path, rewritten)
                        .with_context(|| format!("Failed to write {}", path.display()))?;
                }
            }
        }
    }
    Ok(())
}

/// Rewrite relative `srcPath` values that point above the staged project root
/// to absolute paths under the original source tree. In-tree paths (`./entry`,
/// `entry`) are left untouched.
fn rewrite_build_profile_src_paths(content: &str, source_dir: &Path) -> String {
    const MARKER: &str = "\"srcPath\"";
    let mut out = String::with_capacity(content.len() + content.len() / 16);
    let mut rest = content;
    while let Some(idx) = rest.find(MARKER) {
        out.push_str(&rest[..idx]);
        out.push_str(MARKER);
        let after = &rest[idx + MARKER.len()..];
        // Skip over `: "` (allow whitespace).
        let Some(quote_idx) = after.find('"') else {
            out.push_str(after);
            return out;
        };
        let between = &after[..quote_idx];
        // Bail if there's anything but whitespace and a colon between key and value.
        if !between.chars().all(|c| c.is_whitespace() || c == ':') {
            out.push_str(after);
            return out;
        }
        out.push_str(between);
        out.push('"');
        let value_start = quote_idx + 1;
        let value_rest = &after[value_start..];
        let Some(end) = value_rest.find('"') else {
            out.push_str(value_rest);
            return out;
        };
        let path = &value_rest[..end];
        // Only rewrite paths that escape the project root. Absolute paths and
        // in-tree paths stay as-is.
        if path.starts_with("..") {
            out.push_str(&source_path_string(source_dir.join(path)));
        } else {
            out.push_str(path);
        }
        out.push('"');
        rest = &value_rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Rewrite relative `file:` paths inside a JSON5 document to absolute source
/// paths. Absolute paths and `file://` URLs are left alone.
fn rewrite_file_dependencies(content: &str, source_dir: &Path) -> String {
    const MARKER: &str = "\"file:";
    let mut out = String::with_capacity(content.len() + content.len() / 16);
    let mut rest = content;
    while let Some(idx) = rest.find(MARKER) {
        out.push_str(&rest[..idx]);
        let after = &rest[idx + MARKER.len()..];
        let Some(end) = after.find('"') else {
            // Unterminated string literal; bail out and leave the rest intact.
            out.push_str(&rest[idx..]);
            return out;
        };
        let path = &after[..end];
        out.push_str(MARKER);
        // Leave absolute paths and URL-form refs untouched.
        if path.starts_with("//") || Path::new(path).is_absolute() {
            out.push_str(path);
        } else {
            out.push_str(&source_path_string(source_dir.join(path)));
        }
        out.push('"');
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn source_path_string(path: PathBuf) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Idempotently inject (or refresh) the LingXia SDK HAR as an absolute `file:`
/// dependency in the STAGED `entry/oh-package.json5`.
///
/// Adding `"lingxia": "file:/abs/.../lingxia.har"` to the `dependencies` object.
/// Absolute `file:` paths are left untouched by `rewrite_file_dependencies`, so
/// injecting here (on the staging copy) survives staging-path rewrites. On
/// repeat builds the existing `"lingxia"` line is replaced so version/path
/// drift converges.
fn inject_harmony_har_dependency(staging: &Path, har: &Path) -> Result<()> {
    let pkg_path = staging.join("entry").join("oh-package.json5");
    let original = std::fs::read_to_string(&pkg_path)
        .with_context(|| format!("Failed to read {}", pkg_path.display()))?;

    let abs = har.canonicalize().unwrap_or_else(|_| har.to_path_buf());
    let abs_str = abs.to_string_lossy().replace('\\', "/");
    let dep_value = format!("file:{abs_str}");

    let rewritten = upsert_lingxia_har_dep(&original, &dep_value).ok_or_else(|| {
        anyhow!(
            "Could not locate a `dependencies` object in {}",
            pkg_path.display()
        )
    })?;

    if rewritten != original {
        std::fs::write(&pkg_path, rewritten)
            .with_context(|| format!("Failed to write {}", pkg_path.display()))?;
    }
    Ok(())
}

/// Regenerate the SDK source module's HarmonyOS design icons from
/// `design/icons/svg` into `lingxia-sdk/harmony/lingxia/src/main/resources/
/// rawfile/icons`. That rawfile tree is gitignored and generated (the release
/// script does this via `gen icons --harmony-out`), so an in-workspace build
/// that hasn't run the release script would otherwise package stale/missing
/// icons. The SDK's `.ets` reference `$rawfile('icons/...')` resolves against
/// this module's own rawfile, so this is where the shipped icons must live.
fn stage_sdk_design_icons(workspace_root: &Path) -> Result<()> {
    let svg_dir = workspace_root.join("design/icons/svg");
    if !svg_dir.is_dir() {
        // No design source in this checkout: leave whatever icons are present.
        return Ok(());
    }
    let harmony_icons_dir =
        workspace_root.join("lingxia-sdk/harmony/lingxia/src/main/resources/rawfile/icons");
    println!(
        "  {} Staging SDK design icons → {}",
        "🎨".dimmed(),
        harmony_icons_dir.display()
    );
    crate::r#gen::icons::run(crate::r#gen::icons::IconsConfig {
        input: svg_dir,
        ios_out: None,
        android_out: None,
        harmony_out: Some(harmony_icons_dir),
        windows_out: None,
        windows_png_size: 64,
    })
    .context("Failed to stage SDK HarmonyOS design icons")
}

/// Insert or replace the `"lingxia"` entry in the `dependencies` object of a
/// JSON5 `oh-package.json5` document. Returns `None` if no `dependencies` key
/// is present. String matching keeps us off a full JSON5 parser (deps already
/// avoid adding crates) while staying idempotent via the unique `"lingxia":` key.
fn upsert_lingxia_har_dep(content: &str, dep_value: &str) -> Option<String> {
    const DEPS_KEY: &str = "\"dependencies\"";
    let deps_idx = content.find(DEPS_KEY)?;
    // Find the opening brace of the dependencies object.
    let after_key = &content[deps_idx + DEPS_KEY.len()..];
    let brace_rel = after_key.find('{')?;
    let brace_idx = deps_idx + DEPS_KEY.len() + brace_rel;
    let indent = "    ";
    let new_line = format!("\n{indent}\"lingxia\": \"{dep_value}\",");

    // If a "lingxia" key already exists, replace its value in place.
    if let Some(key_rel) = content[brace_idx..].find("\"lingxia\"") {
        let key_idx = brace_idx + key_rel;
        // Locate the value string that follows `"lingxia"` : `"<...>"`.
        let after_lingxia = &content[key_idx + "\"lingxia\"".len()..];
        let colon_rel = after_lingxia.find(':')?;
        let value_region = &after_lingxia[colon_rel + 1..];
        let first_quote_rel = value_region.find('"')?;
        let value_start = colon_rel + 1 + first_quote_rel + 1;
        let value_rest = &after_lingxia[value_start..];
        let close_quote_rel = value_rest.find('"')?;
        let abs_value_start = key_idx + "\"lingxia\"".len() + value_start;
        let abs_value_end = abs_value_start + close_quote_rel;
        let mut out = String::with_capacity(content.len() + dep_value.len());
        out.push_str(&content[..abs_value_start]);
        out.push_str(dep_value);
        out.push_str(&content[abs_value_end..]);
        return Some(out);
    }

    // Otherwise insert a new entry right after the opening brace.
    let mut out = String::with_capacity(content.len() + new_line.len());
    out.push_str(&content[..=brace_idx]);
    out.push_str(&new_line);
    out.push_str(&content[brace_idx + 1..]);
    Some(out)
}

fn copy_dir_recursive_excluding(src: &Path, dst: &Path, skip: &[&str]) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("Failed to create {}", dst.display()))?;
    for entry in
        std::fs::read_dir(src).with_context(|| format!("Failed to read {}", src.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        if skip
            .iter()
            .any(|s| name.as_os_str() == std::ffi::OsStr::new(s))
        {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_dir_recursive_excluding(&src_path, &dst_path, skip)?;
        } else if ft.is_file() {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} -> {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
        // Intentionally skip symlinks and other file types — Harmony source
        // trees don't use them, and following them risks pulling in unwanted
        // targets.
    }
    Ok(())
}

fn rewrite_app_bundle_name(staging: &Path, config: &BuildConfig) -> Result<()> {
    let app_json_path = staging.join("AppScope/app.json5");
    let base_bundle_name = config
        .lingxia_config
        .as_ref()
        .and_then(|c| c.harmony.as_ref())
        .map(|h| h.bundle_name.as_str())
        .ok_or_else(|| {
            anyhow!("lingxia.yaml is missing `harmony.bundleName`; required to build a HAP")
        })?;
    let suffix = config
        .resolved_env
        .effective_package_id_suffix()
        .unwrap_or("");
    let effective_bundle_name = format!("{base_bundle_name}{suffix}");

    let content = std::fs::read_to_string(&app_json_path)
        .with_context(|| format!("Failed to read {}", app_json_path.display()))?;
    let updated = replace_json5_string_field_value(&content, "bundleName", &effective_bundle_name)?;
    std::fs::write(&app_json_path, updated)
        .with_context(|| format!("Failed to write {}", app_json_path.display()))?;
    Ok(())
}

/// Replace the string value of a single `"<field>"` (or `'<field>'`) key in a
/// JSON5 document while preserving the surrounding text — comments, quoting
/// style, indentation, and trailing punctuation are all left untouched.
///
/// Scans token-aware so the matcher cannot be fooled by:
/// - the field name appearing inside another string literal,
/// - the field name appearing inside `//` or `/* */` comments,
/// - compact or inline-nested layout where the key isn't at the start of a
///   line (e.g. `{"app":{"bundleName":"x"}}`).
///
/// Only the first occurrence is replaced; further matches at the same
/// "key position" return an error so we never silently overwrite multiple
/// fields. Unquoted JSON5 keys (`bundleName: "x"`) are *not* supported — the
/// caller controls the source format and Harmony's `app.json5` always quotes.
fn replace_json5_string_field_value(content: &str, field: &str, value: &str) -> Result<String> {
    let bytes = content.as_bytes();
    let mut out = String::with_capacity(content.len() + value.len());
    let mut i = 0;
    let mut replaced = false;

    while i < bytes.len() {
        let rest = &content[i..];

        // Line comment — copy through to the next newline (or EOF).
        if rest.starts_with("//") {
            let end = rest.find('\n').map(|p| i + p).unwrap_or(bytes.len());
            out.push_str(&content[i..end]);
            i = end;
            continue;
        }

        // Block comment — copy through the closing `*/`.
        if rest.starts_with("/*") {
            let after = &content[i + 2..];
            let end = after
                .find("*/")
                .map(|p| i + 2 + p + 2)
                .unwrap_or(bytes.len());
            out.push_str(&content[i..end]);
            i = end;
            continue;
        }

        let c = bytes[i];
        if c == b'"' || c == b'\'' {
            let quote = c;
            let key_end = scan_json5_string(bytes, i, quote)
                .ok_or_else(|| anyhow!("unterminated string literal near byte {i}"))?;
            let literal = &content[i..key_end];

            // Is this string literal the field key we're looking for? It only
            // counts as a key when followed (after trivia) by `:`.
            let inner = &literal[1..literal.len() - 1];
            let candidate_field_match = inner == field;
            if candidate_field_match
                && let Some((colon_end, value_start)) = find_value_after_colon(bytes, key_end)
            {
                if replaced {
                    return Err(anyhow!(
                        "field '{field}' appears more than once; refusing to overwrite"
                    ));
                }
                let value_quote = bytes[value_start];
                if value_quote != b'"' && value_quote != b'\'' {
                    return Err(anyhow!(
                        "field '{field}' has a non-string value; refusing to overwrite"
                    ));
                }
                let value_end = scan_json5_string(bytes, value_start, value_quote)
                    .ok_or_else(|| anyhow!("unterminated value string for field '{field}'"))?;

                out.push_str(literal); // key as-is, original quoting preserved
                out.push_str(&content[key_end..colon_end]); // `:` + any trivia
                out.push_str(&content[colon_end..value_start]); // pre-value trivia
                out.push(value_quote as char);
                // Escape only the chars that would break the surrounding quote.
                for vc in value.chars() {
                    if vc == value_quote as char || vc == '\\' {
                        out.push('\\');
                    }
                    out.push(vc);
                }
                out.push(value_quote as char);
                i = value_end;
                replaced = true;
                continue;
            }

            // Either not our key, or already replaced — copy verbatim.
            out.push_str(literal);
            i = key_end;
            continue;
        }

        // Default: copy the full UTF-8 scalar. Copying byte-by-byte corrupts
        // non-ASCII app.json5 content such as localized labels or comments.
        let ch = rest
            .chars()
            .next()
            .ok_or_else(|| anyhow!("invalid UTF-8 boundary near byte {i}"))?;
        out.push(ch);
        i += ch.len_utf8();
    }

    if !replaced {
        return Err(anyhow!("field '{field}' not found"));
    }
    Ok(out)
}

/// Return the byte index just past the closing quote of a JSON5 string literal
/// that begins at `start` with quote character `quote`. Handles `\"` / `\'`
/// escapes by skipping the next byte. Returns `None` on EOF before close.
fn scan_json5_string(bytes: &[u8], start: usize, quote: u8) -> Option<usize> {
    debug_assert_eq!(bytes[start], quote);
    let mut j = start + 1;
    while j < bytes.len() {
        let b = bytes[j];
        if b == b'\\' && j + 1 < bytes.len() {
            j += 2;
            continue;
        }
        if b == quote {
            return Some(j + 1);
        }
        j += 1;
    }
    None
}

/// After a candidate key literal, skip trivia (whitespace + comments) and
/// confirm the next non-trivia byte is `:`. Returns `(end_of_colon,
/// start_of_value)` where `start_of_value` already has post-colon trivia
/// skipped past.
fn find_value_after_colon(bytes: &[u8], from: usize) -> Option<(usize, usize)> {
    let after_key = skip_trivia(bytes, from);
    if after_key >= bytes.len() || bytes[after_key] != b':' {
        return None;
    }
    let after_colon = after_key + 1;
    let value_start = skip_trivia(bytes, after_colon);
    if value_start >= bytes.len() {
        return None;
    }
    Some((after_colon, value_start))
}

/// Skip JSON5 inter-token trivia: whitespace, line comments, block comments.
fn skip_trivia(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'/' {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                continue;
            }
        }
        break;
    }
    i
}

fn parse_crate_and_lib_name(manifest_path: &Path) -> Result<(String, String)> {
    let content = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let mut section = "";
    let mut package_name: Option<String> = None;
    let mut lib_name: Option<String> = None;

    for raw_line in content.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "name" {
            continue;
        }

        let name = value.trim().trim_matches('"').trim_matches('\'').trim();
        if name.is_empty() {
            continue;
        }

        match section {
            "package" if package_name.is_none() => package_name = Some(name.to_string()),
            "lib" if lib_name.is_none() => lib_name = Some(name.to_string()),
            _ => {}
        }
    }

    let package_name = package_name.ok_or_else(|| {
        anyhow!(
            "Could not find [package].name in {}",
            manifest_path.display()
        )
    })?;
    let lib_name = lib_name.unwrap_or_else(|| package_name.replace('-', "_"));

    Ok((package_name, lib_name))
}

#[cfg(test)]
mod tests {
    use super::{
        copy_dir_recursive_excluding, replace_json5_string_field_value, rewrite_file_dependencies,
        source_path_string, upsert_lingxia_har_dep,
    };
    use std::{fs, path::Path};
    use tempfile::TempDir;

    #[test]
    fn upsert_har_dep_inserts_into_empty_dependencies() {
        let content = r#"{
  "name": "entry",
  "dependencies": {
    // Add the LingXia dependency via ohpm before building.
  }
}"#;
        let out = upsert_lingxia_har_dep(content, "file:/abs/lingxia.har").unwrap();
        assert!(out.contains(r#""lingxia": "file:/abs/lingxia.har""#));
        // Absolute file: path must survive the relative-only rewrite untouched.
        assert_eq!(rewrite_file_dependencies(&out, Path::new("/source")), out);
    }

    #[test]
    fn upsert_har_dep_replaces_existing_value_idempotently() {
        let content = r#"{
  "dependencies": {
    "lingxia": "file:/old/path/lingxia.har",
    "other": "1.0.0"
  }
}"#;
        let once = upsert_lingxia_har_dep(content, "file:/new/path/lingxia.har").unwrap();
        assert!(once.contains(r#""lingxia": "file:/new/path/lingxia.har""#));
        assert!(!once.contains("/old/path"));
        assert!(once.contains(r#""other": "1.0.0""#));
        // Running again with the same value is a no-op (converges).
        let twice = upsert_lingxia_har_dep(&once, "file:/new/path/lingxia.har").unwrap();
        assert_eq!(twice, once);
    }

    #[test]
    fn upsert_har_dep_returns_none_without_dependencies_object() {
        assert!(upsert_lingxia_har_dep(r#"{"name":"entry"}"#, "file:/x.har").is_none());
    }

    #[test]
    fn rewrite_file_deps_resolves_relative_paths_to_source() {
        let source = TempDir::new().unwrap();
        let content = r#"{
  "dependencies": {
    "lingxia": "file:../../../lingxia-sdk/harmony/lingxia/build/default/outputs/default/lingxia.har",
    "other": "1.0.0"
  }
}"#;
        let expected = source_path_string(
            source
                .path()
                .join("entry")
                .join("../../../lingxia-sdk/harmony/lingxia/build/default/outputs/default/lingxia.har"),
        );
        let rewritten = rewrite_file_dependencies(content, &source.path().join("entry"));
        assert!(rewritten.contains(&format!(r#""file:{expected}""#)));
        // Non-file deps untouched.
        assert!(rewritten.contains(r#""other": "1.0.0""#));
    }

    #[test]
    fn rewrite_file_deps_leaves_absolute_paths_alone() {
        let content = r#"{"dependencies":{"x":"file:/abs/path/lib.har"}}"#;
        let rewritten = rewrite_file_dependencies(content, Path::new("/source"));
        assert!(rewritten.contains(r#""file:/abs/path/lib.har""#));
        assert!(!rewritten.contains("../"));
    }

    #[test]
    fn rewrite_file_deps_is_noop_when_no_file_refs() {
        let content = r#"{"dependencies":{"x":"^1.2.3"}}"#;
        assert_eq!(rewrite_file_dependencies(content, Path::new("/source")), content);
    }

    #[test]
    fn staging_mirror_excludes_build_artifacts_and_self() {
        let source = TempDir::new().unwrap();
        let source_root = source.path();
        // Files that must show up in staging.
        fs::create_dir_all(source_root.join("AppScope")).unwrap();
        fs::write(
            source_root.join("AppScope/app.json5"),
            r#"{"app":{"bundleName":"com.example.demo"}}"#,
        )
        .unwrap();
        fs::create_dir_all(source_root.join("entry/src")).unwrap();
        fs::write(source_root.join("entry/src/main.ets"), "// src").unwrap();
        fs::write(source_root.join("hvigorfile.ts"), "// hvigor").unwrap();
        // Directories that must be excluded from staging.
        fs::create_dir_all(source_root.join("entry/oh_modules/foo")).unwrap();
        fs::write(source_root.join("entry/oh_modules/foo/index.ets"), "// dep").unwrap();
        fs::create_dir_all(source_root.join("entry/build/outputs")).unwrap();
        fs::write(source_root.join("entry/build/outputs/stale.hap"), "stale").unwrap();
        fs::create_dir_all(source_root.join(".lingxia/build/release")).unwrap();
        fs::write(source_root.join(".lingxia/build/release/prev.bin"), "prev").unwrap();

        let staging = TempDir::new().unwrap();
        copy_dir_recursive_excluding(
            source_root,
            staging.path(),
            &[".lingxia", "oh_modules", "build"],
        )
        .unwrap();

        // Included
        assert!(staging.path().join("AppScope/app.json5").exists());
        assert!(staging.path().join("entry/src/main.ets").exists());
        assert!(staging.path().join("hvigorfile.ts").exists());
        // Excluded
        assert!(!staging.path().join("entry/oh_modules").exists());
        assert!(!staging.path().join("entry/build").exists());
        assert!(!staging.path().join(".lingxia").exists());
    }

    #[test]
    fn env_overlay_writes_bundle_name() {
        let content = r#"{
  "app": {
    "bundleName": "com.example.demo",
    "vendor": "example"
  }
}"#;

        let updated =
            replace_json5_string_field_value(content, "bundleName", "com.example.demo.dev")
                .unwrap();
        assert!(updated.contains(r#""bundleName": "com.example.demo.dev""#));
        assert!(updated.contains(r#""vendor": "example""#));
    }

    #[test]
    fn env_overlay_overwrites_stale_suffix() {
        // Simulates a previous developer build whose Drop didn't run, leaving
        // `.dev` baked into app.json5. A subsequent release build must produce
        // the clean base value, not append onto the corrupted state.
        let content = r#"{
  "app": {
    "bundleName": "com.example.demo.dev",
    "vendor": "example"
  }
}"#;

        let updated =
            replace_json5_string_field_value(content, "bundleName", "com.example.demo").unwrap();
        assert!(updated.contains(r#""bundleName": "com.example.demo""#));
        assert!(!updated.contains(".dev"));
    }

    #[test]
    fn env_overlay_handles_compact_single_line_layout() {
        // The line-based matcher would miss this entirely because no line
        // starts with `"bundleName"` — the key sits mid-line.
        let content = r#"{"app":{"bundleName":"com.example.demo","vendor":"example"}}"#;

        let updated =
            replace_json5_string_field_value(content, "bundleName", "com.example.demo.preview")
                .unwrap();

        assert!(updated.contains(r#""bundleName":"com.example.demo.preview""#));
        assert!(updated.contains(r#""vendor":"example""#));
    }

    #[test]
    fn env_overlay_ignores_field_name_inside_comments() {
        let content = r#"{
  "app": {
    // Note: do not rename "bundleName"; downstream tools assume this exact key.
    /* legacy: "bundleName": "ignored.com" */
    "bundleName": "com.example.demo",
    "vendor": "example"
  }
}"#;

        let updated =
            replace_json5_string_field_value(content, "bundleName", "com.example.demo.dev")
                .unwrap();

        assert!(updated.contains(r#""bundleName": "com.example.demo.dev""#));
        // Comments preserved verbatim.
        assert!(updated.contains(r#"// Note: do not rename "bundleName""#));
        assert!(updated.contains(r#"/* legacy: "bundleName": "ignored.com" */"#));
    }

    #[test]
    fn env_overlay_ignores_field_name_inside_string_values() {
        // A *value* string that mentions "bundleName" must not be treated as
        // the key — only the actual key match is rewritten.
        let content = r#"{
  "doc": "the bundleName field is set below",
  "app": {
    "bundleName": "com.example.demo"
  }
}"#;

        let updated =
            replace_json5_string_field_value(content, "bundleName", "com.example.demo.dev")
                .unwrap();

        assert!(updated.contains(r#""doc": "the bundleName field is set below""#));
        assert!(updated.contains(r#""bundleName": "com.example.demo.dev""#));
    }

    #[test]
    fn env_overlay_preserves_non_ascii_content() {
        let content = r#"{
  "app": {
    "bundleName": "com.example.demo",
    "vendor": "凌霞",
    "label": "演示应用"
  }
}"#;

        let updated =
            replace_json5_string_field_value(content, "bundleName", "com.example.demo.dev")
                .unwrap();

        assert!(updated.contains(r#""vendor": "凌霞""#));
        assert!(updated.contains(r#""label": "演示应用""#));
        assert!(updated.contains(r#""bundleName": "com.example.demo.dev""#));
    }

    #[test]
    fn env_overlay_errors_on_duplicate_field() {
        let content = r#"{
  "app": {
    "bundleName": "com.example.demo",
    "module": { "bundleName": "com.example.other" }
  }
}"#;

        let err = replace_json5_string_field_value(content, "bundleName", "com.example.demo.dev")
            .unwrap_err()
            .to_string();

        assert!(err.contains("more than once"));
    }

    #[test]
    fn env_overlay_errors_when_field_absent() {
        let content = r#"{"app":{"vendor":"example"}}"#;
        let err = replace_json5_string_field_value(content, "bundleName", "x").unwrap_err();
        assert!(err.to_string().contains("bundleName"));
    }
}
