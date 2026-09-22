//! NSIS backend: render and compile Setup/Portable wrappers around a staged payload.
use super::distribution::{WindowsPackageFormat, safe_component};
use crate::config::LingXiaConfig;
use anyhow::{Context, Result, bail};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn find_makensis() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("LINGXIA_MAKENSIS") {
        let path = PathBuf::from(path);
        if !path.is_file() {
            bail!("LINGXIA_MAKENSIS does not point to a file");
        }
        return Ok(path);
    }
    if let Ok(path) = which::which("makensis") {
        return Ok(path);
    }
    for root in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(root) = std::env::var_os(root) {
            let path = PathBuf::from(root).join("NSIS/makensis.exe");
            if path.is_file() {
                return Ok(path);
            }
        }
    }
    bail!(
        "NSIS 3 not found. Install NSIS (winget install NSIS.NSIS), or set LINGXIA_MAKENSIS to makensis.exe. Use --format zip or --format msix if no EXE wrapper is needed."
    )
}

// Values become NSIS source, never shell arguments. NSIS preprocesses defines
// before interpreting runtime escapes, so reject preprocessor/escape sequences.
fn literal(value: &str) -> Result<String> {
    if value.chars().any(|c| c.is_control()) {
        bail!("NSIS metadata cannot contain control characters");
    }
    if ["${", "$%", "$\\"]
        .iter()
        .any(|sequence| value.contains(sequence))
    {
        bail!("NSIS metadata/path contains a reserved preprocessor sequence");
    }
    Ok(value.replace('$', "$$").replace('"', "$\\\""))
}

pub(super) fn package(
    config: &LingXiaConfig,
    payload: &Path,
    executable: &OsStr,
    output: &Path,
    format: WindowsPackageFormat,
    architecture: &str,
) -> Result<()> {
    let compiler = find_makensis()?;
    let dir = tempfile::tempdir()?;
    let script = render(
        config,
        payload,
        executable,
        output,
        format,
        architecture,
        dir.path(),
    )?;
    fs::write(
        dir.path().join("common.nsh"),
        include_str!("../../../templates/windows/common.nsh"),
    )?;
    fs::write(
        dir.path().join("webview2.ps1"),
        include_str!("../../../templates/windows/webview2.ps1"),
    )?;
    let script_path = dir.path().join("package.nsi");
    fs::write(&script_path, format!("\u{feff}{script}"))?;
    let verbosity = if cfg!(windows) { "/V2" } else { "-V2" };
    let result = Command::new(compiler)
        .arg(verbosity)
        .arg(&script_path)
        .status()
        .context("Failed to execute NSIS")?;
    if !result.success() {
        bail!("NSIS packaging failed ({result})");
    }
    if !output.is_file() {
        bail!("NSIS did not produce {}", output.display());
    }
    Ok(())
}

fn render(
    config: &LingXiaConfig,
    payload: &Path,
    executable: &OsStr,
    output: &Path,
    format: WindowsPackageFormat,
    architecture: &str,
    resources: &Path,
) -> Result<String> {
    let app = config.app.as_ref().context("Missing app configuration")?;
    let app_id = config.resolved_package_id("windows")?;
    safe_component(&app_id)?;
    let exe = executable
        .to_str()
        .context("Executable name must be UTF-8")?;
    safe_component(exe)?;
    let shortcut_name: String = app
        .product_name
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '-' } else { c })
        .collect();
    let shortcut = format!("{shortcut_name} ({app_id})");
    safe_component(&shortcut)?;
    let version = semver::Version::parse(&app.product_version)?;
    for component in [version.major, version.minor, version.patch] {
        if component > u64::from(u16::MAX) {
            bail!("Windows executable version components must fit in 16 bits");
        }
    }
    let file_version = format!("{}.{}.{}.0", version.major, version.minor, version.patch);
    let portable_data = config.windows.as_ref().is_some_and(|w| w.portable_data);
    let mut script = String::new();
    for (key, value) in [
        ("PRODUCT", app.product_name.clone()),
        ("VERSION", app.product_version.clone()),
        ("FILE_VERSION", file_version),
        ("APP_ID", app_id),
        ("EXE", exe.into()),
        ("ARCH", architecture.into()),
        ("SHORTCUT", shortcut),
        ("PORTABLE_DATA", portable_data.to_string()),
        ("PAYLOAD", payload.to_string_lossy().into_owned()),
        ("OUTPUT", output.to_string_lossy().into_owned()),
        (
            "COMMON",
            resources.join("common.nsh").to_string_lossy().into_owned(),
        ),
        (
            "WEBVIEW_SCRIPT",
            resources
                .join("webview2.ps1")
                .to_string_lossy()
                .into_owned(),
        ),
    ] {
        script.push_str(&format!("!define {key} \"{}\"\n", literal(&value)?));
    }
    let ico = payload.join("AppIcon.ico");
    if ico.is_file() {
        script.push_str(&format!("Icon \"{}\"\n", literal(&ico.to_string_lossy())?));
    }
    if format == WindowsPackageFormat::Nsis {
        script.push_str(&super::signing::nsis_uninstaller_signing()?);
        script.push_str(include_str!("../../../templates/windows/setup.nsi"));
    } else {
        script.push_str(include_str!("../../../templates/windows/portable.nsi"));
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_nsis_injection() {
        assert_eq!(literal("$INSTDIR \"").unwrap(), "$$INSTDIR $\\\"");
        for value in ["${PRODUCT}", "$%PATH%", "$\\n", "app\n!system bad"] {
            assert!(literal(value).is_err(), "{value}");
        }
    }
    #[test]
    fn compile_installers_when_nsis_available() {
        if find_makensis().is_err() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let payload = temp.path().join("payload");
        fs::create_dir_all(payload.join("assets")).unwrap();
        fs::write(payload.join("app.exe"), b"fixture").unwrap();
        fs::write(payload.join("assets/app.json"), b"{}").unwrap();
        let config: LingXiaConfig = serde_yaml_ng::from_str("app:\n  projectName: test\n  productName: Test App\n  productVersion: 1.2.3\n  packageId: com.lingxia.packaging_test\n  platforms: [windows]\n").unwrap();
        for format in [WindowsPackageFormat::Nsis, WindowsPackageFormat::Portable] {
            let out = temp.path().join(format!("{format:?}.exe"));
            package(
                &config,
                &payload,
                OsStr::new("app.exe"),
                &out,
                format,
                "x64",
            )
            .unwrap();
            assert_eq!(&fs::read(out).unwrap()[..2], b"MZ");
        }
    }
}
