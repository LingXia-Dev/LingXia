//! MSIX packaging for Windows host apps.
//!
//! Packs the assembled `windows/.lingxia/dist/<Product>/` payload (the exe next
//! to the runtime `assets/`) into an installable `.msix` at
//! `<project>/dist/windows/<Product>.msix` — the Windows counterpart of how
//! macOS packages its `.app` into `dist/macos/<Product>.dmg`.
//!
//! The package is produced **unsigned**. Windows refuses to install an unsigned
//! MSIX, so the caller must sign it (`signtool`) before installation; the CLI
//! prints the command. Built-in signing can be layered on later.

use std::ffi::OsString;
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

/// Pack the assembled `dist_dir` (`windows/.lingxia/dist/<Product>/`) into an
/// unsigned `<project>/dist/windows/<Product>.msix`. Returns the `.msix` path.
pub fn package(project_root: &Path, config: &LingXiaConfig, dist_dir: &Path) -> Result<PathBuf> {
    let makeappx = find_makeappx()?;

    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing [app] config for MSIX packaging"))?;
    let product_name = app.product_name.trim();
    let windows_cfg = config.windows.as_ref();

    let exe_name = dist_exe_name(dist_dir)?;
    let identity = sanitize_identity(
        windows_cfg
            .and_then(|w| w.app_id.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(product_name),
    );
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
    let lingxia_dir = dist_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow!("unexpected dist path: {}", dist_dir.display()))?;
    let staging = lingxia_dir.join("msix-staging");
    if staging.exists() {
        std::fs::remove_dir_all(&staging)
            .with_context(|| format!("Failed to clear {}", staging.display()))?;
    }
    crate::platform::apple::copy_dir_recursive(dist_dir, &staging)?;
    // Drop any nested `.lingxia/` dirs that rode along inside lxapp bundles
    // (e.g. a dev-runner mirror) so the shipped package isn't bloated with
    // duplicated dev cruft — matching what the build-script asset copy skips.
    prune_nested_lingxia(&staging)?;

    generate_logos(&staging.join("Images"), &dist_dir.join("assets"))?;

    let manifest = render_manifest(
        &identity,
        &publisher,
        &version,
        product_name,
        &exe_name.to_string_lossy(),
    );
    std::fs::write(staging.join("AppxManifest.xml"), manifest)
        .context("Failed to write AppxManifest.xml")?;

    let out_dir = project_root.join("dist").join("windows");
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("Failed to create {}", out_dir.display()))?;
    let msix_path = out_dir.join(format!("{product_name}.msix"));

    let status = Command::new(&makeappx)
        .args(["pack", "/d"])
        .arg(&staging)
        .arg("/p")
        .arg(&msix_path)
        .arg("/o")
        .status()
        .with_context(|| format!("Failed to run {}", makeappx.display()))?;
    if !status.success() {
        bail!("makeappx pack failed");
    }

    println!(
        "{} Packed MSIX → {}",
        "[Windows]".cyan(),
        msix_path.display()
    );
    println!(
        "  {} unsigned — Windows won't install it until signed, e.g.\n     signtool sign /fd SHA256 /a /f <cert.pfx> /p <password> \"{}\"",
        "note:".yellow(),
        msix_path.display()
    );
    Ok(msix_path)
}

/// Locate `makeappx.exe` from the Windows SDK (newest version, x64), or an
/// explicit `LINGXIA_MAKEAPPX` override.
fn find_makeappx() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("LINGXIA_MAKEAPPX").map(PathBuf::from) {
        if path.is_file() {
            return Ok(path);
        }
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

/// Recursively remove any directory named `.lingxia` under `dir` (generated
/// dev artifacts that shouldn't ship inside the package).
fn prune_nested_lingxia(dir: &Path) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read {}", dir.display()))?
        .flatten()
    {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        if entry.file_name() == ".lingxia" {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
        } else {
            prune_nested_lingxia(&path)?;
        }
    }
    Ok(())
}

fn dist_exe_name(dist_dir: &Path) -> Result<OsString> {
    for entry in std::fs::read_dir(dist_dir)
        .with_context(|| format!("Failed to read {}", dist_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        {
            return Ok(entry.file_name());
        }
    }
    Err(anyhow!("no .exe found in {}", dist_dir.display()))
}

/// The runtime icon the SDK loads: `<assets>/AppIcon.png` (host icon) first,
/// then `<assets>/<app>/public/AppIcon.png`.
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
fn sanitize_identity(raw: &str) -> String {
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
) -> String {
    let display = xml_escape(display_name);
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package
    xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
    xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
    xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities">
  <Identity Name="{identity}" Publisher="{publisher}" Version="{version}" ProcessorArchitecture="x64" />
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
