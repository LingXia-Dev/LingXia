use super::{
    DEFAULT_ABILITY_NAME, HarmonyPlatform, HarmonySigner, ProvisioningManager, SigningConfig,
    SigningMode, project::resolve_harmony_dir, read_bundle_name, resolve_effective_acl_permissions,
};
use crate::platform::{BuildProfile, Device, DeviceType, InstallConfig, RunConfig};
use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

impl HarmonyPlatform {
    pub(super) fn install_impl(&self, config: &InstallConfig) -> Result<()> {
        let hdc = ensure_command("hdc")?;

        let explicit_artifact = config.artifact_path.is_some();
        let hap_path = if let Some(ref path) = config.artifact_path {
            path.clone()
        } else {
            let harmony_dir = resolve_harmony_dir(&config.project_root, None)?;
            auto_detect_hap(&harmony_dir)?
        };

        if !hap_path.exists() {
            return Err(anyhow!("HAP not found at: {}", hap_path.display()));
        }

        let target_udids = ensure_device_connected(config.device_id.as_deref())?;

        // Install path is strict: always sign first, then install.
        let hap_path = self.sign_before_install(&hap_path, &config.project_root, &target_udids)?;

        if config.reinstall {
            let package_id = infer_harmony_bundle_for_uninstall(&config.project_root);
            if let Some(package_id) = package_id {
                if let Err(err) = self.uninstall_impl(&package_id, config.device_id.as_deref()) {
                    eprintln!(
                        "  {} failed to uninstall {} before install: {}",
                        "Warning:".yellow(),
                        package_id,
                        err
                    );
                }
            } else {
                eprintln!(
                    "  {} could not resolve Harmony bundle name for --reinstall; continuing install",
                    "Warning:".yellow()
                );
            }
        }

        let mut cmd = Command::new(&hdc);
        if let Some(ref device_id) = config.device_id {
            cmd.arg("-t").arg(device_id);
        }
        cmd.arg("install").arg("-r").arg(&hap_path);

        let install_progress = harmony_install_progress(&hap_path, config.quiet)?;
        let output = cmd.output().context("Failed to execute hdc install")?;
        finish_harmony_install_progress(install_progress);

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || hdc_install_failed(&stdout, &stderr) {
            return Err(anyhow!(
                "hdc install failed:\n{}\n{}",
                stdout.trim(),
                stderr.trim()
            ));
        }

        if let Some(package_id) =
            install_verification_bundle(&hap_path, explicit_artifact, &config.project_root)?
        {
            verify_bundle_installed(&hdc, config.device_id.as_deref(), &package_id)?;
        }

        println!("{}", "  ✓ Installed".green());
        Ok(())
    }

    pub(super) fn uninstall_impl(&self, package_id: &str, device_id: Option<&str>) -> Result<()> {
        let hdc = ensure_command("hdc")?;

        let mut cmd = Command::new(&hdc);
        if let Some(id) = device_id {
            cmd.arg("-t").arg(id);
        }
        cmd.arg("uninstall").arg(package_id);

        let status = cmd.status().context("Failed to execute hdc uninstall")?;
        if !status.success() {
            return Err(anyhow!("Failed to uninstall {}", package_id));
        }

        println!("  {} Uninstalled {}", "✓".green(), package_id);
        Ok(())
    }

    pub(super) fn run_impl(&self, config: &RunConfig) -> Result<()> {
        if config.restart {
            bail!(
                "Restart is not supported for HarmonyOS yet. Use 'lingxia uninstall' + 'lingxia launch', or plain 'lingxia launch'."
            );
        }

        let hdc = ensure_command("hdc")?;

        let ability = config
            .main_activity
            .as_deref()
            .unwrap_or(DEFAULT_ABILITY_NAME);

        let mut cmd = Command::new(&hdc);
        if let Some(ref device_id) = config.device_id {
            cmd.arg("-t").arg(device_id);
        }
        // TODO: when Harmony enables env suffix (.dev / .preview) on bundleName,
        // auto-detect the installed variant via `hdc shell bm dump -a` — mirror
        // android.rs::resolve_installed_app_id / devicectl::resolve_installed_bundle_id.
        // Today harmony deploys are release-only so the canonical id always
        // matches what's on device.
        cmd.arg("shell")
            .arg("aa")
            .arg("start")
            .arg("-a")
            .arg(ability)
            .arg("-b")
            .arg(&config.package_id);

        let output = cmd
            .output()
            .context("Failed to execute hdc shell aa start")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() || stdout.contains("error:") || stderr.contains("error:") {
            let msg = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                stdout.trim().to_string()
            };
            return Err(anyhow!(
                "Failed to launch app (bundle={}, ability={}): {}",
                config.package_id,
                ability,
                msg
            ));
        }

        println!("  {} App launched", "✓".green());
        Ok(())
    }

    pub(super) fn list_devices_impl(&self) -> Result<Vec<Device>> {
        let hdc = ensure_command("hdc")?;

        let output = Command::new(&hdc)
            .arg("list")
            .arg("targets")
            .output()
            .context("Failed to execute hdc list targets")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let devices: Vec<Device> = stdout
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && *line != "[Empty]")
            .map(|line| Device {
                id: line.to_string(),
                name: None,
                device_type: if line.contains("emulator") || line.starts_with("127.0.0.1") {
                    DeviceType::Emulator
                } else {
                    DeviceType::Physical
                },
                online: true,
            })
            .collect();

        Ok(devices)
    }

    /// Sign input HAP before install regardless of current filename.
    fn sign_before_install(
        &self,
        hap_path: &Path,
        project_root: &Path,
        target_udids: &[String],
    ) -> Result<PathBuf> {
        let source = preferred_resign_source(hap_path);
        let mut output_path = install_signed_output_path(&source);

        if output_path == source {
            output_path = source.with_file_name(format!(
                "{}-resigned.hap",
                source.file_stem().unwrap_or_default().to_string_lossy()
            ));
        }

        self.sign_hap_with_project_config_at(
            &source,
            project_root,
            output_path,
            BuildProfile::Debug,
            target_udids,
        )
    }

    pub(super) fn sign_hap_with_project_config(
        &self,
        input_hap: &Path,
        project_root: &Path,
        build_profile: BuildProfile,
    ) -> Result<PathBuf> {
        let output_path = signed_output_path(input_hap);
        self.sign_hap_with_project_config_at(
            input_hap,
            project_root,
            output_path,
            build_profile,
            &[],
        )
    }

    fn sign_hap_with_project_config_at(
        &self,
        input_hap: &Path,
        project_root: &Path,
        output_path: PathBuf,
        build_profile: BuildProfile,
        target_udids: &[String],
    ) -> Result<PathBuf> {
        let signing = load_signing_config(project_root, build_profile, target_udids)?;
        let signer = HarmonySigner::new_native();
        let aligned_hap = align_unsigned_hap_for_mmap(input_hap)?;
        let signing_input = aligned_hap.as_deref().unwrap_or(input_hap);
        println!("  {} Signing HAP (Rust native signer)...", "→".dimmed());
        signer
            .sign_hap(&signing, signing_input, &output_path)
            .context("HAP signing failed")?;
        signer
            .verify_hap(&output_path)
            .context("Signed HAP verification failed")?;

        println!(
            "  {} Signed HAP created: {}",
            "✓".green(),
            output_path.display()
        );
        Ok(output_path)
    }
}

fn load_signing_config(
    project_root: &Path,
    build_profile: BuildProfile,
    target_udids: &[String],
) -> Result<SigningConfig> {
    let harmony_dir = resolve_harmony_dir(project_root, None)?;
    let bundle_name = read_bundle_name(&harmony_dir)?;

    let mode = match build_profile {
        BuildProfile::Debug => SigningMode::Debug,
        BuildProfile::Release => SigningMode::Release,
    };
    let resolution = resolve_effective_acl_permissions(&bundle_name);
    if !resolution.missing_permissions.is_empty() {
        eprintln!(
            "{} Harmony restricted ACL permissions not granted for `{}`: {}",
            "Warning:".yellow(),
            bundle_name,
            resolution.missing_permissions.join(", ")
        );
    }
    if !resolution.can_sync_managed_permissions {
        eprintln!(
            "{} Harmony ACL approvals are not verified; signing will proceed without requesting managed restricted ACL permissions.",
            "Warning:".yellow()
        );
    }
    let effective_acl_permissions = resolution.effective_permissions;

    let mut provisioning = ProvisioningManager::from_storage()?;
    provisioning.prepare_signing_config(
        &bundle_name,
        mode,
        target_udids,
        &effective_acl_permissions,
    )
}

pub(super) fn ensure_command(name: &str) -> Result<PathBuf> {
    resolve_command_path(name).ok_or_else(|| {
        anyhow!(
            "'{}' not found. Install Harmony command-line tools and set OHOS_NDK_HOME.\n\
             Also ensure '{}' is available in PATH or under the OHOS_NDK_HOME tool directories.",
            name,
            name
        )
    })
}

pub(super) fn resolve_command_path(name: &str) -> Option<PathBuf> {
    if let Ok(path) = which::which(name) {
        return Some(path);
    }
    resolve_command_from_ohos_ndk_home(name)
}

fn resolve_command_from_ohos_ndk_home(name: &str) -> Option<PathBuf> {
    let ndk_home = env::var("OHOS_NDK_HOME").ok()?;
    let ndk_home = PathBuf::from(ndk_home);
    if !ndk_home.exists() {
        return None;
    }

    let mut candidates = Vec::new();
    let toolchains = ndk_home.join("toolchains");
    let root = ndk_home
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());

    if name == "hdc" {
        candidates.push(toolchains.join("hdc"));
        candidates.push(toolchains.join("hdc.exe"));
        candidates.push(toolchains.join("hdc.cmd"));
        candidates.push(toolchains.join("hdc.bat"));
    } else {
        candidates.push(toolchains.join(name));
        candidates.push(toolchains.join(format!("{name}.exe")));
        candidates.push(toolchains.join(format!("{name}.cmd")));
        candidates.push(toolchains.join(format!("{name}.bat")));
        candidates.push(toolchains.join(name).join("bin").join(name));
        candidates.push(
            toolchains
                .join(name)
                .join("bin")
                .join(format!("{name}.exe")),
        );
        candidates.push(
            toolchains
                .join(name)
                .join("bin")
                .join(format!("{name}.cmd")),
        );
        candidates.push(
            toolchains
                .join(name)
                .join("bin")
                .join(format!("{name}.bat")),
        );
    }

    if let Some(root) = root {
        candidates.push(root.join("bin").join(name));
        candidates.push(root.join("bin").join(format!("{name}.exe")));
        candidates.push(root.join("bin").join(format!("{name}.cmd")));
        candidates.push(root.join("bin").join(format!("{name}.bat")));
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn ensure_device_connected(device_id: Option<&str>) -> Result<Vec<String>> {
    let targets = connected_devices()?;

    if targets.is_empty() {
        return Err(anyhow!(
            "No HarmonyOS device connected. Connect a device via USB or start an emulator."
        ));
    }

    if let Some(id) = device_id {
        if let Some(exact) = targets.iter().find(|candidate| candidate.as_str() == id) {
            return Ok(vec![fetch_harmony_udid(exact)?]);
        }
        if let Some(partial) = targets.iter().find(|candidate| candidate.contains(id)) {
            return Ok(vec![fetch_harmony_udid(partial)?]);
        }

        return Err(anyhow!(
            "Device '{}' not found. Available devices: {}",
            id,
            targets.join(", ")
        ));
    }

    targets
        .iter()
        .map(|target| fetch_harmony_udid(target))
        .collect()
}

fn infer_harmony_bundle_for_uninstall(project_root: &Path) -> Option<String> {
    let harmony_dir = resolve_harmony_dir(project_root, None).ok()?;
    read_bundle_name(&harmony_dir).ok()
}

fn install_verification_bundle(
    hap_path: &Path,
    explicit_artifact: bool,
    project_root: &Path,
) -> Result<Option<String>> {
    match read_hap_bundle_name(hap_path) {
        Ok(package_id) => Ok(Some(package_id)),
        Err(err) if explicit_artifact => Err(err),
        Err(_) => Ok(infer_harmony_bundle_for_uninstall(project_root)),
    }
}

fn harmony_install_progress(hap_path: &Path, quiet: bool) -> Result<Option<ProgressBar>> {
    if quiet {
        return Ok(None);
    }
    let size_mb = hap_path
        .metadata()
        .map(|metadata| metadata.len() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0);
    let file_name = hap_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("HAP");
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    spinner.set_message(format!("Installing {file_name} ({size_mb:.1} MB)..."));
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    Ok(Some(spinner))
}

fn finish_harmony_install_progress(progress: Option<ProgressBar>) {
    if let Some(progress) = progress {
        progress.finish_and_clear();
    }
}

fn read_hap_bundle_name(hap_path: &Path) -> Result<String> {
    let file = File::open(hap_path)
        .with_context(|| format!("Failed to open HAP {}", hap_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("Failed to read HAP {}", hap_path.display()))?;
    let mut module = archive
        .by_name("module.json")
        .with_context(|| format!("HAP {} is missing module.json", hap_path.display()))?;
    let mut content = String::new();
    module
        .read_to_string(&mut content)
        .with_context(|| format!("Failed to read module.json from {}", hap_path.display()))?;
    let root: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse module.json from {}", hap_path.display()))?;
    root.get("app")
        .and_then(|app| app.get("bundleName"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow!(
                "HAP {} module.json missing app.bundleName",
                hap_path.display()
            )
        })
}

fn verify_bundle_installed(hdc: &Path, device_id: Option<&str>, package_id: &str) -> Result<()> {
    let mut cmd = Command::new(hdc);
    if let Some(device_id) = device_id {
        cmd.arg("-t").arg(device_id);
    }
    cmd.arg("shell")
        .arg("bm")
        .arg("dump")
        .arg("-n")
        .arg(package_id);

    let output = cmd
        .output()
        .with_context(|| format!("Failed to verify installed Harmony bundle {package_id}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success()
        || hdc_command_failed(&stdout, &stderr)
        || stdout.contains("failed to get information")
    {
        return Err(anyhow!(
            "Harmony install did not register bundle `{}`.\n{}\n{}",
            package_id,
            stdout.trim(),
            stderr.trim()
        ));
    }
    Ok(())
}

fn connected_devices() -> Result<Vec<String>> {
    let hdc = ensure_command("hdc")?;
    let output = Command::new(&hdc)
        .arg("list")
        .arg("targets")
        .output()
        .context("Failed to execute hdc list targets")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty() && line != "[Empty]")
        .collect())
}

fn fetch_harmony_udid(target: &str) -> Result<String> {
    let hdc = ensure_command("hdc")?;
    let output = Command::new(&hdc)
        .arg("-t")
        .arg(target)
        .arg("shell")
        .arg("bm")
        .arg("get")
        .arg("-u")
        .output()
        .with_context(|| format!("Failed to query Harmony UDID for target {target}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "Failed to query Harmony UDID for target {}: {}\n{}",
            target,
            stdout.trim(),
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let udid = stdout
        .split(|ch: char| !ch.is_ascii_hexdigit())
        .find(|token| token.len() >= 32)
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow!(
                "Failed to parse Harmony UDID from hdc output: {}",
                stdout.trim()
            )
        })?;

    Ok(udid)
}

fn auto_detect_hap(harmony_dir: &Path) -> Result<PathBuf> {
    // Builds since 0.6.4 mirror the Harmony project into `.lingxia/build/<env>/`
    // and emit the hap there. Older builds (and the SwiftPM-style standalone
    // path) leave artifacts directly under the source tree. Scan staging first,
    // fall back to source, and pick the newest by mtime so the most recent
    // build always wins regardless of where it landed.
    let mut candidates: Vec<PathBuf> = Vec::new();
    let staging_root = harmony_dir.join(".lingxia").join("build");
    if let Ok(entries) = std::fs::read_dir(&staging_root) {
        for entry in entries.flatten() {
            collect_hap_candidates(&entry.path(), &mut candidates);
        }
    }
    collect_hap_candidates(harmony_dir, &mut candidates);

    candidates
        .into_iter()
        .filter(|p| p.is_file())
        .max_by_key(|p| std::fs::metadata(p).and_then(|meta| meta.modified()).ok())
        .ok_or_else(|| {
            anyhow!("No HAP found. Build the project first with 'lingxia build --platform harmony'")
        })
}

fn collect_hap_candidates(base: &Path, out: &mut Vec<PathBuf>) {
    let outputs = base.join("entry/build/default/outputs/default");
    let signed = outputs.join("entry-default-signed.hap");
    let unsigned = outputs.join("entry-default-unsigned.hap");
    if signed.exists() {
        out.push(signed);
    } else if unsigned.exists() {
        out.push(unsigned);
    }
}

fn signed_output_path(input_hap: &Path) -> PathBuf {
    let file_name = input_hap
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if file_name.contains("unsigned") {
        return input_hap.with_file_name(file_name.replace("unsigned", "signed"));
    }
    let stem = input_hap
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    input_hap.with_file_name(format!("{stem}-signed.hap"))
}

fn install_signed_output_path(input_hap: &Path) -> PathBuf {
    let stem = input_hap
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let normalized = stem
        .trim_end_matches("-unsigned")
        .trim_end_matches("-signed")
        .trim_end_matches("-install-signed");
    input_hap.with_file_name(format!("{normalized}-install-signed.hap"))
}

fn preferred_resign_source(path: &Path) -> PathBuf {
    if let Some(unsigned) = sibling_unsigned_hap(path)
        && unsigned.exists()
    {
        return unsigned;
    }
    path.to_path_buf()
}

fn sibling_unsigned_hap(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_string_lossy();
    if file_name.contains("unsigned") {
        return Some(path.to_path_buf());
    }
    if file_name.contains("install-signed") {
        return Some(path.with_file_name(file_name.replace("install-signed", "unsigned")));
    }
    if file_name.contains("signed") {
        return Some(path.with_file_name(file_name.replace("signed", "unsigned")));
    }
    None
}

fn align_unsigned_hap_for_mmap(input_hap: &Path) -> Result<Option<PathBuf>> {
    let file_name = input_hap
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !file_name.contains("unsigned") {
        return Ok(None);
    }

    let aligned = input_hap.with_file_name(file_name.replace("unsigned", "aligned"));
    align_hap_entries(input_hap, &aligned)?;
    Ok(Some(aligned))
}

#[derive(Debug)]
struct CentralEntry {
    start: usize,
    end: usize,
    name: String,
    local_offset: u32,
}

#[derive(Debug)]
struct LocalEntry {
    central_index: usize,
    local_offset: usize,
    name: String,
    method: u16,
    header: Vec<u8>,
    name_bytes: Vec<u8>,
    extra: Vec<u8>,
    data: Vec<u8>,
}

fn align_hap_entries(input_hap: &Path, output_hap: &Path) -> Result<()> {
    const EOCD_MIN_SIZE: usize = 22;
    const LOCAL_HEADER_SIZE: usize = 30;
    const ALIGNMENT: usize = 4096;

    let data = std::fs::read(input_hap)
        .with_context(|| format!("Failed to read HAP {}", input_hap.display()))?;
    let eocd = find_eocd(&data)?;
    let cd_size = read_u32(&data, eocd + 12)? as usize;
    let cd_offset = read_u32(&data, eocd + 16)? as usize;
    let cd_end = cd_offset
        .checked_add(cd_size)
        .ok_or_else(|| anyhow!("Central directory size overflow"))?;
    if eocd < EOCD_MIN_SIZE || cd_end > data.len() || cd_end > eocd {
        bail!("Invalid HAP central directory layout");
    }

    let entries = parse_central_entries(&data, cd_offset, cd_end)?;
    let mut local_entries = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_local_entry(&data, cd_offset, index, entry))
        .collect::<Result<Vec<_>>>()?;
    local_entries.sort_by_key(|entry| {
        (
            runnable_order(&entry.name),
            entry.local_offset,
            entry.central_index,
        )
    });

    let mut out = Vec::with_capacity(data.len() + entries.len() * ALIGNMENT);
    let mut offset_map = std::collections::HashMap::new();
    let mut extra_len_map = std::collections::HashMap::new();
    let mut extra_map = std::collections::HashMap::new();

    for entry in local_entries {
        let new_local_offset = out.len();
        let current_data_offset =
            new_local_offset + LOCAL_HEADER_SIZE + entry.name_bytes.len() + entry.extra.len();
        let padding = if entry.method == 0 {
            (ALIGNMENT - (current_data_offset % ALIGNMENT)) % ALIGNMENT
        } else {
            0
        };
        let new_extra_len = entry.extra.len() + padding;
        if new_extra_len > u16::MAX as usize {
            bail!("ZIP extra field too large for {}", entry.name);
        }
        let mut header = entry.header;
        header[28..30].copy_from_slice(&(new_extra_len as u16).to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(&entry.name_bytes);
        out.extend_from_slice(&entry.extra);
        out.resize(out.len() + padding, 0);
        out.extend_from_slice(&entry.data);

        let mut central_extra = entry.extra;
        central_extra.resize(central_extra.len() + padding, 0);
        offset_map.insert(entry.local_offset as u32, new_local_offset as u32);
        extra_len_map.insert(entry.central_index, new_extra_len);
        extra_map.insert(entry.central_index, central_extra);
    }

    let new_cd_offset = out.len();
    for (index, entry) in entries.iter().enumerate() {
        let Some(new_offset) = offset_map.get(&entry.local_offset) else {
            bail!("Missing rewritten local offset for {}", entry.name);
        };
        let Some(extra) = extra_map.get(&index) else {
            bail!("Missing rewritten central extra for {}", entry.name);
        };
        let Some(extra_len) = extra_len_map.get(&index) else {
            bail!("Missing rewritten central extra length for {}", entry.name);
        };
        let name_len = read_u16(&data, entry.start + 28)? as usize;
        let old_extra_len = read_u16(&data, entry.start + 30)? as usize;
        let name_start = entry.start + 46;
        let extra_start = name_start + name_len;
        let comment_start = extra_start + old_extra_len;
        let mut header = data[entry.start..entry.start + 46].to_vec();
        header[30..32].copy_from_slice(&(*extra_len as u16).to_le_bytes());
        header[42..46].copy_from_slice(&new_offset.to_le_bytes());
        out.extend_from_slice(&header);
        out.extend_from_slice(&data[name_start..extra_start]);
        out.extend_from_slice(extra);
        out.extend_from_slice(&data[comment_start..entry.end]);
    }

    let mut eocd_bytes = data[eocd..].to_vec();
    let new_cd_size = out.len() - new_cd_offset;
    eocd_bytes[12..16].copy_from_slice(&(new_cd_size as u32).to_le_bytes());
    eocd_bytes[16..20].copy_from_slice(&(new_cd_offset as u32).to_le_bytes());
    out.extend_from_slice(&eocd_bytes);

    std::fs::write(output_hap, out)
        .with_context(|| format!("Failed to write aligned HAP {}", output_hap.display()))?;
    Ok(())
}

fn parse_local_entry(
    data: &[u8],
    cd_offset: usize,
    central_index: usize,
    central: &CentralEntry,
) -> Result<LocalEntry> {
    let local_offset = central.local_offset as usize;
    if local_offset + 30 > cd_offset || read_u32(data, local_offset)? != 0x0403_4b50 {
        bail!("Invalid local entry offset for {}", central.name);
    }
    let name_len = read_u16(data, local_offset + 26)? as usize;
    let extra_len = read_u16(data, local_offset + 28)? as usize;
    let method = read_u16(data, local_offset + 8)?;
    let compressed_size = read_u32(data, local_offset + 18)? as usize;
    let name_start = local_offset + 30;
    let extra_start = name_start + name_len;
    let data_offset = extra_start + extra_len;
    let data_end = data_offset
        .checked_add(compressed_size)
        .ok_or_else(|| anyhow!("Local entry size overflow"))?;
    if data_end > cd_offset {
        bail!("Invalid local entry data range for {}", central.name);
    }
    Ok(LocalEntry {
        central_index,
        local_offset,
        name: central.name.clone(),
        method,
        header: data[local_offset..local_offset + 30].to_vec(),
        name_bytes: data[name_start..extra_start].to_vec(),
        extra: data[extra_start..data_offset].to_vec(),
        data: data[data_offset..data_end].to_vec(),
    })
}

fn runnable_order(name: &str) -> u8 {
    if name.ends_with(".abc") {
        0
    } else if name.ends_with(".an") || name.starts_with("libs/") {
        1
    } else if name == ".pages.info" {
        2
    } else if name == "ets/sourceMaps.map" {
        3
    } else {
        4
    }
}

fn parse_central_entries(data: &[u8], mut offset: usize, end: usize) -> Result<Vec<CentralEntry>> {
    let mut entries = Vec::new();
    while offset < end {
        if offset + 46 > end || read_u32(data, offset)? != 0x0201_4b50 {
            bail!("Invalid central directory entry");
        }
        let name_len = read_u16(data, offset + 28)? as usize;
        let extra_len = read_u16(data, offset + 30)? as usize;
        let comment_len = read_u16(data, offset + 32)? as usize;
        let local_offset = read_u32(data, offset + 42)?;
        let name_start = offset + 46;
        let name_end = name_start + name_len;
        let next = name_end + extra_len + comment_len;
        if next > end {
            bail!("Invalid central directory entry size");
        }
        let name = std::str::from_utf8(&data[name_start..name_end])
            .unwrap_or_default()
            .to_string();
        entries.push(CentralEntry {
            start: offset,
            end: next,
            name,
            local_offset,
        });
        offset = next;
    }
    Ok(entries)
}

fn find_eocd(data: &[u8]) -> Result<usize> {
    let min = 22usize;
    if data.len() < min {
        bail!("File too small to be a ZIP");
    }
    let search_start = data.len().saturating_sub(min + 65535);
    for index in (search_start..=data.len() - min).rev() {
        if data[index..index + 4] == [0x50, 0x4b, 0x05, 0x06] {
            let comment_len = read_u16(data, index + 20)? as usize;
            if index + min + comment_len == data.len() {
                return Ok(index);
            }
        }
    }
    bail!("End of central directory not found")
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| anyhow!("Unexpected EOF reading u16"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow!("Unexpected EOF reading u32"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn hdc_install_failed(stdout: &str, stderr: &str) -> bool {
    hdc_command_failed(stdout, stderr)
}

fn hdc_command_failed(stdout: &str, stderr: &str) -> bool {
    fn has_failure_marker(text: &str) -> bool {
        let lower = text.to_ascii_lowercase();
        lower.contains("[fail]")
            || lower.contains("error:")
            || lower.contains("failed")
            || lower.contains("fail to")
            || lower.contains("install failed")
    }

    has_failure_marker(stdout) || has_failure_marker(stderr)
}

#[cfg(test)]
mod tests {
    use super::{
        hdc_install_failed, preferred_resign_source, read_hap_bundle_name, sibling_unsigned_hap,
    };
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn preferred_resign_source_uses_unsigned_sibling_for_signed_hap() {
        let temp = tempdir().unwrap();
        let signed = temp.path().join("entry-default-signed.hap");
        let unsigned = temp.path().join("entry-default-unsigned.hap");
        fs::write(&signed, b"signed").unwrap();
        fs::write(&unsigned, b"unsigned").unwrap();

        assert_eq!(preferred_resign_source(&signed), unsigned);
    }

    #[test]
    fn preferred_resign_source_keeps_signed_when_unsigned_missing() {
        let temp = tempdir().unwrap();
        let signed = temp.path().join("entry-default-signed.hap");
        fs::write(&signed, b"signed").unwrap();

        assert_eq!(preferred_resign_source(&signed), signed);
    }

    #[test]
    fn sibling_unsigned_hap_maps_install_signed_to_unsigned() {
        let path = std::path::Path::new("/tmp/entry-default-install-signed.hap");

        assert_eq!(
            sibling_unsigned_hap(path).unwrap(),
            std::path::Path::new("/tmp/entry-default-unsigned.hap")
        );
    }

    #[test]
    fn hdc_install_failed_detects_fail_marker_with_success_exit() {
        assert!(hdc_install_failed(
            "[Fail][E001005] Device not found or connected",
            ""
        ));
    }

    #[test]
    fn read_hap_bundle_name_uses_app_bundle_name() {
        let temp = tempdir().unwrap();
        let hap_path = temp.path().join("entry-default.hap");
        let file = fs::File::create(&hap_path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("module.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(br#"{"app":{"bundleName":"app.lingxia.test"},"module":{"name":"entry"}}"#)
            .unwrap();
        archive.finish().unwrap();

        assert_eq!(read_hap_bundle_name(&hap_path).unwrap(), "app.lingxia.test");
    }
}
