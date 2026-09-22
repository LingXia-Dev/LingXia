//! MSIX packaging for Windows host apps.
//!
//! Packs the shared runnable payload into a versioned, architecture-specific
//! package. Local self-signing is handled here; production Authenticode signing
//! and final artifact publication are handled by `distribution`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use image::imageops::FilterType;

use crate::config::LingXiaConfig;

/// Tile/store logos MSIX requires, generated from the app icon into the
/// package's `Images/` dir. Kept out of the runtime `assets/` dir to dodge the
/// case-insensitive `assets` vs `Assets` clash on Windows.
const LOGOS: &[(&str, u32)] = &[
    ("Square44x44Logo.png", 44),
    ("Square150x150Logo.png", 150),
    ("StoreLogo.png", 50),
];

/// Pack the assembled payload at `msix_path`, optionally self-signing for local use.
pub fn package(
    config: &LingXiaConfig,
    dist_dir: &Path,
    exe_name: &OsStr,
    signing: super::signing::WindowsSigning,
    msix_path: &Path,
) -> Result<PathBuf> {
    let makeappx = find_makeappx()?;

    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing [app] config for MSIX packaging"))?;
    let product_name = app.product_name.trim();
    let windows_cfg = config.windows.as_ref();

    let architecture = super::distribution::pe_architecture(&dist_dir.join(exe_name))?;
    let identity = sanitize_identity(&config.resolved_package_id("windows")?);
    // The Identity Publisher must match the eventual signing cert's subject.
    // Default to a readable `CN=<product>`; override with `windows.publisher`.
    let publisher = windows_cfg
        .and_then(|w| w.publisher.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("CN={}", sanitize_cn(product_name)));
    let version = four_part_version(&app.product_version);

    // Stage a copy of the payload, then add `Images/` logos + the manifest.
    // (Staging keeps the runnable dist folder clean of MSIX-only files.)
    let temp = tempfile::tempdir_in(dist_dir.parent().context("Invalid Windows payload path")?)?;
    let staging = temp.path().join("msix");
    crate::platform::apple::copy_dir_recursive(dist_dir, &staging)?;

    generate_logos(&staging.join("Images"), &dist_dir.join("assets"))?;

    let manifest = render_manifest(
        &identity,
        &publisher,
        &version,
        product_name,
        &exe_name.to_string_lossy(),
        architecture,
    );
    std::fs::write(staging.join("AppxManifest.xml"), manifest)
        .context("Failed to write AppxManifest.xml")?;

    let status = Command::new(&makeappx)
        .args(["pack", "/d"])
        .arg(&staging)
        .arg("/p")
        .arg(msix_path)
        .arg("/o")
        .status()
        .with_context(|| format!("Failed to run {}", makeappx.display()))?;
    if !status.success() {
        bail!("makeappx pack failed");
    }

    if matches!(signing, super::signing::WindowsSigning::SelfSigned) {
        super::signing::sign_msix(msix_path, &publisher, signing)?;
    } else if !super::signing::release_signing_enabled()? {
        println!(
            "  {} MSIX is unsigned; sign the final artifact or use --self-signed for local testing",
            "note:".yellow()
        );
    }
    Ok(msix_path.to_path_buf())
}

/// Locate `makeappx.exe` from the Windows SDK (newest version, x64), or an
/// explicit `LINGXIA_MAKEAPPX` override.
pub(super) fn find_makeappx() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("LINGXIA_MAKEAPPX").map(PathBuf::from)
        && path.is_file()
    {
        return Ok(path);
    }
    let bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(bin) {
        for entry in entries.flatten() {
            let exe = entry.path().join("x64").join("makeappx.exe");
            if exe.is_file() {
                candidates.push(exe);
            }
        }
    }
    candidates.sort();
    candidates.pop().ok_or_else(|| {
        anyhow!(
            "makeappx.exe not found. Install the Windows 10/11 SDK (it ships makeappx/signtool), \
             or set LINGXIA_MAKEAPPX to its path."
        )
    })
}

fn resolve_icon(assets: &Path) -> Option<PathBuf> {
    let root = assets.join("AppIcon.png");
    if root.is_file() {
        return Some(root);
    }
    std::fs::read_dir(assets).ok().and_then(|entries| {
        entries
            .flatten()
            .map(|entry| entry.path().join("public").join("AppIcon.png"))
            .find(|path| path.is_file())
    })
}

fn generate_logos(images_dir: &Path, assets: &Path) -> Result<()> {
    let icon = resolve_icon(assets).ok_or_else(|| {
        anyhow!(
            "AppIcon.png not found under {}; cannot generate MSIX logos",
            assets.display()
        )
    })?;
    let img = image::open(&icon).with_context(|| format!("Failed to open {}", icon.display()))?;
    std::fs::create_dir_all(images_dir)
        .with_context(|| format!("Failed to create {}", images_dir.display()))?;
    for (name, size) in LOGOS {
        let resized = img.resize_exact(*size, *size, FilterType::Lanczos3);
        let dest = images_dir.join(name);
        resized
            .save(&dest)
            .with_context(|| format!("Failed to write {}", dest.display()))?;
    }
    Ok(())
}

/// Pad/truncate a semver to MSIX's required 4-part `a.b.c.d` (digits only).
fn four_part_version(version: &str) -> String {
    let core = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts: Vec<String> = core
        .split('.')
        .map(|part| {
            let digits: String = part.chars().filter(char::is_ascii_digit).collect();
            if digits.is_empty() {
                "0".to_string()
            } else {
                digits
            }
        })
        .collect();
    while parts.len() < 4 {
        parts.push("0".to_string());
    }
    parts.truncate(4);
    parts.join(".")
}

/// MSIX package Identity `Name`: letters, digits, `.` and `-` only.
pub(crate) fn sanitize_identity(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "LingXiaApp".to_string()
    } else {
        cleaned
    }
}

/// Strip characters that would need escaping in a `CN=` distinguished name.
fn sanitize_cn(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !matches!(c, ',' | '=' | '+' | '<' | '>' | '#' | ';' | '"' | '\\'))
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "LingXia".to_string()
    } else {
        trimmed.to_string()
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn render_manifest(
    identity: &str,
    publisher: &str,
    version: &str,
    display_name: &str,
    executable: &str,
    architecture: &str,
) -> String {
    let display = xml_escape(display_name);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package
    xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
    xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
    xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities">
  <Identity Name="{identity}" Publisher="{publisher}" Version="{version}" ProcessorArchitecture="{architecture}" />
  <Properties>
    <DisplayName>{display}</DisplayName>
    <PublisherDisplayName>{display}</PublisherDisplayName>
    <Logo>Images\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Applications>
    <Application Id="App" Executable="{executable}" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements
        DisplayName="{display}"
        Description="{display}"
        BackgroundColor="transparent"
        Square150x150Logo="Images\Square150x150Logo.png"
        Square44x44Logo="Images\Square44x44Logo.png" />
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
  </Capabilities>
</Package>
"#,
        identity = xml_escape(identity),
        publisher = xml_escape(publisher),
        version = version,
        executable = xml_escape(executable),
    )
}
