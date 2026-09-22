//! One runnable payload, multiple delivery formats. The update zip retains the
//! legacy root layout and adds installers for format-aware hosts.
use crate::config::LingXiaConfig;
use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowsPackageFormat {
    Nsis,
    Portable,
    Msix,
    Zip,
}

pub fn validate_selection(
    has_windows: bool,
    formats: &[WindowsPackageFormat],
    msix: bool,
    self_signed: bool,
) -> Result<()> {
    if !has_windows && (!formats.is_empty() || msix || self_signed) {
        bail!("--format, --msix and --self-signed require a Windows host target");
    }
    Ok(())
}

pub fn resolve_formats(
    requested: &[WindowsPackageFormat],
    package: bool,
    msix: bool,
    self_signed: bool,
) -> Vec<WindowsPackageFormat> {
    let mut formats = Vec::new();
    for format in requested {
        if !formats.contains(format) {
            formats.push(*format);
        }
    }
    if (msix || self_signed) && !formats.contains(&WindowsPackageFormat::Msix) {
        formats.push(WindowsPackageFormat::Msix);
    }
    if formats.is_empty() && package {
        formats.push(WindowsPackageFormat::Nsis);
    }
    formats
}

pub fn preflight(formats: &[WindowsPackageFormat]) -> Result<()> {
    if formats.iter().any(|f| {
        matches!(
            f,
            WindowsPackageFormat::Nsis | WindowsPackageFormat::Portable
        )
    }) {
        super::nsis::find_makensis()?;
    }
    if formats.contains(&WindowsPackageFormat::Msix) {
        super::msix::find_makeappx()?;
    }
    if !formats.is_empty() {
        super::signing::preflight_release_signing()?;
    }
    Ok(())
}

/// Read the built PE, rather than assuming that CLI and payload architectures match.
pub fn pe_architecture(exe: &Path) -> Result<&'static str> {
    let mut file = fs::File::open(exe)?;
    let mut dos = [0; 64];
    file.read_exact(&mut dos)
        .context("Truncated Windows executable")?;
    if &dos[..2] != b"MZ" {
        bail!("Not a Windows PE executable: {}", exe.display());
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap());
    file.seek(SeekFrom::Start(u64::from(offset)))?;
    let mut header = [0; 6];
    file.read_exact(&mut header)?;
    if &header[..4] != b"PE\0\0" {
        bail!("Invalid PE signature");
    }
    match u16::from_le_bytes([header[4], header[5]]) {
        0x8664 => Ok("x64"),
        0xaa64 => Ok("arm64"),
        0x014c => Ok("x86"),
        machine => bail!("Unsupported Windows PE machine: {machine:#x}"),
    }
}

pub(super) fn safe_component(value: &str) -> Result<&str> {
    if value.is_empty()
        || value.trim() != value
        || value.ends_with('.')
        || value
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || matches!(value, "." | "..")
    {
        bail!("Invalid Windows artifact name component: {value:?}");
    }
    let stem = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        bail!("Reserved Windows file name: {value}");
    }
    Ok(value)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    format: String,
    file: String,
    sha256: String,
    size: u64,
}

pub fn package(
    project_root: &Path,
    config: &LingXiaConfig,
    exe: &Path,
    formats: &[WindowsPackageFormat],
    self_signed: bool,
) -> Result<Vec<PathBuf>> {
    let mut config = config.clone();
    let payload = exe.parent().context("Executable has no parent")?;
    let generated: serde_json::Value =
        serde_json::from_slice(&fs::read(payload.join("assets/app.json"))?)?;
    let app_id = generated["windowsAppId"]
        .as_str()
        .filter(|v| !v.is_empty())
        .context("Windows payload is missing windowsAppId")?;
    config.windows.get_or_insert_with(Default::default).app_id = Some(app_id.into());
    if let Some(app) = &mut config.app {
        if let Some(name) = generated["productName"].as_str() {
            app.product_name = name.into();
        }
        if generated["env"].as_str() == Some("dev") {
            app.project_name.push_str("-dev");
        }
    }
    let config = &config;
    let app = config.app.as_ref().context("Missing app configuration")?;
    let name = safe_component(&app.project_name)?;
    let version = safe_component(&app.product_version)?;
    let arch = pe_architecture(exe)?;
    let output_dir = project_root.join("dist/windows");
    fs::create_dir_all(&output_dir)?;
    let output_stage = tempfile::tempdir_in(&output_dir)?;
    let output = output_stage.path();
    // Work in a private staging tree, so signing and wrapper-specific files
    // cannot contaminate build/dev output or another packaging format.
    let staging = tempfile::tempdir_in(payload.parent().context("Invalid payload directory")?)?;
    let source = staging.path().join("payload");
    validate_payload(payload)?;
    for reserved in [
        ".lingxia-update",
        ".lingxia-distribution",
        ".lingxia-install-id",
    ] {
        if payload.join(reserved).exists() {
            bail!("Reserved Windows payload entry: {reserved}");
        }
    }
    super::super::apple::copy_dir_recursive(payload, &source)?;
    prune_dev_files(&source)?;
    if super::signing::release_signing_enabled()? {
        sign_payload(&source)?;
    }
    let exe_name = exe.file_name().context("Missing executable name")?;
    let mut artifacts = Vec::new();
    let mut installers = Vec::new();
    for format in formats {
        let dest = match format {
            WindowsPackageFormat::Nsis | WindowsPackageFormat::Portable => {
                let kind = if *format == WindowsPackageFormat::Nsis {
                    "Setup"
                } else {
                    "Portable"
                };
                let dest = output.join(format!("{name}-{version}-{arch}-{kind}.exe"));
                super::nsis::package(config, &source, exe_name, &dest, *format, arch)?;
                super::signing::sign_release_artifact(&dest)?;
                installers.push((*format, dest.clone()));
                dest
            }
            WindowsPackageFormat::Msix => {
                let mode = if self_signed {
                    super::signing::WindowsSigning::SelfSigned
                } else {
                    super::signing::WindowsSigning::None
                };
                let dest = output.join(format!("{name}-{version}-{arch}.msix"));
                super::msix::package(config, &source, exe_name, mode, &dest)?;
                if !self_signed {
                    super::signing::sign_release_artifact(&dest)?;
                }
                dest
            }
            WindowsPackageFormat::Zip => {
                let dest = output.join(format!("{name}-{version}-{arch}-portable.zip"));
                write_zip(&source, &dest)?;
                dest
            }
        };
        artifacts.push(dest);
    }
    // MSIX is OS-managed. Only direct distributions need a feed archive.
    if formats.iter().any(|f| *f != WindowsPackageFormat::Msix) {
        let updates = source.join(".lingxia-update");
        fs::create_dir_all(&updates)?;
        for (format, path) in installers {
            let file = if format == WindowsPackageFormat::Nsis {
                "setup.exe"
            } else {
                "portable.exe"
            };
            fs::copy(path, updates.join(file))?;
        }
        fs::write(
            updates.join("manifest.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": 1, "appId": config.resolved_package_id("windows")?,
                "version": app.product_version, "architecture": arch,
                "executable": exe_name.to_string_lossy(),
            }))?,
        )?;
        // Keep the suffix consumed by publish's platform/metadata detection.
        let zip = output.join(format!("{name}-{version}-{arch}-windows.zip"));
        write_zip(&source, &zip)?;
        artifacts.push(zip);
    }
    let records: Vec<Artifact> = artifacts
        .iter()
        .map(|path| -> Result<Artifact> {
            let mut file = fs::File::open(path)?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hash.update(&buffer[..read]);
            }
            let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
            let format = if file_name.ends_with("-windows.zip") {
                "update"
            } else if file_name.ends_with("-Setup.exe") {
                "nsis"
            } else if file_name.ends_with("-Portable.exe") {
                "portable"
            } else if file_name.ends_with(".msix") {
                "msix"
            } else {
                "zip"
            };
            Ok(Artifact {
                format: format.into(),
                file: file_name,
                sha256: hash.finalize().iter().map(|b| format!("{b:02x}")).collect(),
                size: fs::metadata(path)?.len(),
            })
        })
        .collect::<Result<_>>()?;
    let manifest = output.join(format!("{name}-{version}-{arch}-artifacts.json"));
    fs::write(
        &manifest,
        serde_json::to_vec_pretty(
            &serde_json::json!({"schemaVersion": 1, "appId": config.resolved_package_id("windows")?, "version": version, "architecture": arch, "artifacts": records}),
        )?,
    )?;
    // Publish only after every format and checksum succeeds. Failed compilers
    // leave previously published artifacts and their manifest intact.
    let mut published = Vec::new();
    for path in &artifacts {
        let dest = output_dir.join(path.file_name().context("Invalid artifact path")?);
        fs::rename(path, &dest)?;
        published.push(dest);
    }
    let manifest_dest = output_dir.join(manifest.file_name().context("Invalid manifest path")?);
    fs::rename(manifest, &manifest_dest)?;
    let artifacts = published;
    let manifest = manifest_dest;
    for path in &artifacts {
        println!("✓ package → {}", path.display());
    }
    println!("✓ artifact manifest → {}", manifest.display());
    Ok(artifacts)
}

fn validate_payload(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            bail!(
                "Symlinks are not supported in Windows payloads: {}",
                entry.path().display()
            );
        }
        if ty.is_dir() {
            validate_payload(&entry.path())?;
        }
    }
    Ok(())
}

fn prune_dev_files(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            if entry.file_name() == ".lingxia" {
                fs::remove_dir_all(entry.path())?;
            } else {
                prune_dev_files(&entry.path())?;
            }
        }
    }
    Ok(())
}

fn sign_payload(dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            sign_payload(&path)?;
        } else if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("exe") || s.eq_ignore_ascii_case("dll"))
        {
            super::signing::sign_release_artifact(&path)?;
        }
    }
    Ok(())
}

fn write_zip(source: &Path, dest: &Path) -> Result<()> {
    // The entire output directory is already private until packaging succeeds.
    let mut writer = zip::ZipWriter::new(fs::File::create(dest)?);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    crate::commands::build::add_zip_dir(&mut writer, source, "", options)?;
    writer.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Exercised by scripts/ci/windows-package-smoke.ps1 with a real PE fixture.
    #[test]
    #[ignore = "requires Windows and the packaging smoke-test fixture"]
    fn windows_distribution_smoke_fixture() {
        let root =
            PathBuf::from(std::env::var_os("LINGXIA_PACKAGING_TEST_ROOT").expect("fixture root"));
        let config: LingXiaConfig =
            serde_yaml_ng::from_slice(&fs::read(root.join("lingxia.yaml")).unwrap()).unwrap();
        image::RgbaImage::from_pixel(256, 256, image::Rgba([35, 80, 150, 255]))
            .save(root.join("payload/assets/AppIcon.png"))
            .unwrap();
        package(
            &root,
            &config,
            &root.join("payload/demo.exe"),
            &[
                WindowsPackageFormat::Nsis,
                WindowsPackageFormat::Portable,
                WindowsPackageFormat::Zip,
                WindowsPackageFormat::Msix,
            ],
            false,
        )
        .unwrap();
    }

    #[test]
    fn formats_default_and_legacy_aliases() {
        use WindowsPackageFormat::*;
        assert_eq!(resolve_formats(&[], true, false, false), [Nsis]);
        assert_eq!(resolve_formats(&[], true, true, false), [Msix]);
        assert_eq!(resolve_formats(&[], false, false, false), []);
        assert_eq!(
            resolve_formats(&[Portable, Portable, Zip], true, false, true),
            [Portable, Zip, Msix]
        );
        assert!(validate_selection(false, &[Zip], false, false).is_err());
    }
    #[test]
    fn reject_unsafe_names() {
        for name in [
            "",
            "../app",
            "a/b",
            "a\\b",
            "a\n",
            "CON",
            "nul.exe",
            "LPT1",
            "trailing.",
        ] {
            assert!(safe_component(name).is_err(), "{name}");
        }
        assert!(safe_component("浮生-1.2.3").is_ok());
    }
    #[test]
    fn zip_distribution_separates_update_metadata_and_preserves_payload() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let payload = root.join("payload");
        fs::create_dir_all(payload.join("assets/.lingxia/cache")).unwrap();
        let mut pe = vec![0u8; 70];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..68].copy_from_slice(b"PE\0\0");
        pe[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
        fs::write(payload.join("demo.exe"), &pe).unwrap();
        fs::write(
            payload.join("assets/.lingxia/cache/dev.txt"),
            "not for shipping",
        )
        .unwrap();
        fs::write(
            payload.join("assets/app.json"),
            r#"{"windowsAppId":"com.example.demo.dev","productName":"Demo Dev","env":"dev"}"#,
        )
        .unwrap();
        let config: LingXiaConfig = serde_yaml_ng::from_str("app:\n  projectName: demo\n  productName: Demo\n  productVersion: 1.2.3\n  packageId: com.example.demo\n  platforms: [windows]\n").unwrap();
        let files = package(
            root,
            &config,
            &payload.join("demo.exe"),
            &[WindowsPackageFormat::Zip],
            false,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        let mut portable = zip::ZipArchive::new(fs::File::open(&files[0]).unwrap()).unwrap();
        assert!(portable.by_name("demo.exe").is_ok());
        assert!(portable.by_name(".lingxia-update/manifest.json").is_err());
        assert!(portable.by_name("assets/.lingxia/cache/dev.txt").is_err());
        let mut update = zip::ZipArchive::new(fs::File::open(&files[1]).unwrap()).unwrap();
        let metadata: serde_json::Value =
            serde_json::from_reader(update.by_name(".lingxia-update/manifest.json").unwrap())
                .unwrap();
        assert_eq!(metadata["appId"], "com.example.demo.dev");
        assert_eq!(metadata["architecture"], "x64");
        assert!(!payload.join(".lingxia-update").exists());
        assert!(payload.join("assets/.lingxia/cache/dev.txt").is_file());
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("dist/windows/demo-dev-1.2.3-x64-artifacts.json")).unwrap(),
        )
        .unwrap();
        for record in manifest["artifacts"].as_array().unwrap() {
            let bytes = fs::read(
                root.join("dist/windows")
                    .join(record["file"].as_str().unwrap()),
            )
            .unwrap();
            let checksum: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(record["sha256"], checksum);
            assert_eq!(record["size"].as_u64().unwrap(), bytes.len() as u64);
        }
    }

    #[test]
    fn identifies_pe_machine_not_host() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("app.exe");
        let mut bytes = vec![0; 70];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        for (machine, arch) in [(0x8664u16, "x64"), (0xaa64, "arm64"), (0x14c, "x86")] {
            bytes[68..70].copy_from_slice(&machine.to_le_bytes());
            fs::write(&file, &bytes).unwrap();
            assert_eq!(pe_architecture(&file).unwrap(), arch);
        }
        fs::write(&file, b"MZ").unwrap();
        assert!(pe_architecture(&file).is_err());
    }
}
