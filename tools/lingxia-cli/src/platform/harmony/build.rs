use super::{HarmonyPlatform, OHOS_TARGET, deploy::ensure_command};
use crate::commands::rust::run_cargo_build_for_target;
use crate::platform::{BuildArtifacts, BuildConfig, BuildProfile};
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
        if config.build_native {
            let so_path = self.build_rust_library(&config.project_root, config)?;
            self.stage_native_library(&so_path, harmony_dir)?;
        } else {
            println!(
                "  {} Skipping native compilation (using existing .so)",
                "⏭️".dimmed()
            );
        }

        self.ohpm_install(harmony_dir)?;
        let hap_path = self.build_hap(harmony_dir, config)?;

        Ok(BuildArtifacts::Harmony { hap_path })
    }

    fn build_rust_library(&self, project_root: &Path, config: &BuildConfig) -> Result<PathBuf> {
        println!("{}", "Compiling native code (HarmonyOS)...".cyan());

        let ndk_path = Self::detect_ohos_ndk()?;
        let lingxia_config = config
            .lingxia_config
            .as_ref()
            .ok_or_else(|| anyhow!("lingxia.config.json is required to build native libraries"))?;

        let rust_lib_name = lingxia_config
            .get_rust_lib_name()
            .ok_or_else(|| anyhow!("app.projectName is required in lingxia.config.json"))?;
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

        let target_dir = project_root.join("target");
        run_cargo_build_for_target(
            &rust_manifest,
            &rust_lib_dir,
            &target_dir,
            OHOS_TARGET,
            Some(&crate_name),
            config.profile,
            &config.features,
            |cmd| {
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
