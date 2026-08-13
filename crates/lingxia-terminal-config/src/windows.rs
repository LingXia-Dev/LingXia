//! Optional Windows ConPTY runtime used for escape-sequence passthrough.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

pub const CONPTY_VERSION: &str = "1.24.260710001";
pub const CONPTY_DOWNLOAD_URL: &str = "https://api.nuget.org/v3-flatcontainer/microsoft.windows.console.conpty/1.24.260710001/microsoft.windows.console.conpty.1.24.260710001.nupkg";
pub const CONPTY_PACKAGE_SHA256: &str =
    "175640566a3b59c4b132070ee96c2c77e5ab7edd2e92732a5eb3610bbf63d90e";
pub const CONPTY_PACKAGE_BYTES: u64 = 1_732_296;

const STATE_FILE: &str = "conpty.json";
const NOTICE: &str = "Microsoft Windows Console ConPTY redistributable\n\
Version: 1.24.260710001\n\
Source: https://www.nuget.org/packages/Microsoft.Windows.Console.ConPTY/1.24.260710001\n\
License: MIT\n\n\
Copyright (c) Microsoft Corporation\n\n\
Permission is hereby granted, free of charge, to any person obtaining a copy\n\
of this software and associated documentation files (the \"Software\"), to deal\n\
in the Software without restriction, including without limitation the rights\n\
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell\n\
copies of the Software, and to permit persons to whom the Software is\n\
furnished to do so, subject to the following conditions:\n\n\
The above copyright notice and this permission notice shall be included in all\n\
copies or substantial portions of the Software.\n\n\
THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR\n\
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,\n\
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE\n\
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER\n\
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,\n\
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE\n\
SOFTWARE.\n";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConptyPackageInfo {
    pub version: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsInlineImageStatus {
    pub enabled: bool,
    pub installed: bool,
    pub package: ConptyPackageInfo,
}

#[derive(Debug)]
pub enum ConptyInstallError {
    Io(std::io::Error),
    InvalidPackage(String),
    Archive(zip::result::ZipError),
    UnsupportedArchitecture,
    Load(String),
}

impl std::fmt::Display for ConptyInstallError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "ConPTY installation I/O error: {error}"),
            Self::InvalidPackage(reason) => write!(formatter, "invalid ConPTY package: {reason}"),
            Self::Archive(error) => write!(formatter, "invalid ConPTY archive: {error}"),
            Self::UnsupportedArchitecture => {
                write!(formatter, "this Windows architecture is not supported")
            }
            Self::Load(reason) => write!(formatter, "failed to activate ConPTY: {reason}"),
        }
    }
}

impl std::error::Error for ConptyInstallError {}

impl From<std::io::Error> for ConptyInstallError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for ConptyInstallError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Archive(error)
    }
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ConptyState {
    enabled: bool,
}

struct Architecture {
    name: &'static str,
    dll_entry: &'static str,
    host_entry: &'static str,
    dll_sha256: &'static str,
    host_sha256: &'static str,
    dll_max_bytes: u64,
    host_max_bytes: u64,
}

fn architecture() -> Result<Architecture, ConptyInstallError> {
    #[cfg(target_arch = "x86_64")]
    return Ok(Architecture {
        name: "x64",
        dll_entry: "runtimes/win-x64/native/conpty.dll",
        host_entry: "build/native/runtimes/x64/OpenConsole.exe",
        dll_sha256: "39fba2713e2495117b1591ae8c32a3b904bea7aa66069cf7815e2844c76d75d8",
        host_sha256: "b7fd936c2668b87b9ecf7b3366dc6568afc1c6f981874cba3e955a1c35cf8160",
        dll_max_bytes: 256 * 1024,
        host_max_bytes: 2 * 1024 * 1024,
    });
    #[cfg(target_arch = "x86")]
    return Ok(Architecture {
        name: "x86",
        dll_entry: "runtimes/win-x86/native/conpty.dll",
        host_entry: "build/native/runtimes/x86/OpenConsole.exe",
        dll_sha256: "11c4d8b3015e593f9f9f6872500bd37a076517cb33092422571c0a28bbd25347",
        host_sha256: "7c199ea9db18c2f99e2ec2dc339a0fd4d8441adedf4589a0c7508bb492066c65",
        dll_max_bytes: 256 * 1024,
        host_max_bytes: 2 * 1024 * 1024,
    });
    #[cfg(target_arch = "aarch64")]
    return Ok(Architecture {
        name: "arm64",
        dll_entry: "runtimes/win-arm64/native/conpty.dll",
        host_entry: "build/native/runtimes/arm64/OpenConsole.exe",
        dll_sha256: "db3d173640b172bafd42d5b541b638a9aeec1c7d0e40dd636bf02822a32c912c",
        host_sha256: "ed7622fd0d3bedc9ab9f122f5e58edf0def9e7999224f52dd395ba9f54edbe09",
        dll_max_bytes: 256 * 1024,
        host_max_bytes: 2 * 1024 * 1024,
    });
    #[allow(unreachable_code)]
    Err(ConptyInstallError::UnsupportedArchitecture)
}

fn root(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_dir(app_data_dir)
        .join("terminal")
        .join("conpty")
}

fn install_dir(app_data_dir: &Path, architecture: &Architecture) -> PathBuf {
    root(app_data_dir)
        .join(CONPTY_VERSION)
        .join(architecture.name)
}

fn state_path(app_data_dir: &Path) -> PathBuf {
    root(app_data_dir).join(STATE_FILE)
}

fn read_state(app_data_dir: &Path) -> ConptyState {
    fs::read_to_string(state_path(app_data_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_state(app_data_dir: &Path, state: &ConptyState) -> Result<(), ConptyInstallError> {
    let path = state_path(app_data_dir);
    let parent = path.parent().expect("ConPTY state has a parent directory");
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|error| {
        ConptyInstallError::InvalidPackage(format!("failed to encode state: {error}"))
    })?;
    atomic_write(&path, &bytes)?;
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let (temporary, mut file) = (0..32)
        .find_map(|_| {
            let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
            let temporary =
                path.with_extension(format!("{extension}.tmp-{}-{sequence}", std::process::id()));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => Some(Ok((temporary, file))),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .transpose()?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("cannot reserve a temporary file for {}", path.display()),
            )
        })?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);

    let from = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let to = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    if let Err(error) = unsafe {
        MoveFileExW(
            PCWSTR(from.as_ptr()),
            PCWSTR(to.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } {
        let _ = fs::remove_file(temporary);
        return Err(std::io::Error::other(error.to_string()));
    }
    Ok(())
}

fn sha256(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut output = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

fn installed(app_data_dir: &Path, architecture: &Architecture) -> bool {
    let directory = install_dir(app_data_dir, architecture);
    let dll = directory.join("conpty.dll");
    let host = directory.join("OpenConsole.exe");
    sha256(&dll).is_ok_and(|value| value == architecture.dll_sha256)
        && sha256(&host).is_ok_and(|value| value == architecture.host_sha256)
}

pub fn status(app_data_dir: &Path) -> WindowsInlineImageStatus {
    let state = read_state(app_data_dir);
    let is_installed = architecture().is_ok_and(|value| installed(app_data_dir, &value));
    WindowsInlineImageStatus {
        enabled: state.enabled && is_installed,
        installed: is_installed,
        package: ConptyPackageInfo {
            version: CONPTY_VERSION,
            url: CONPTY_DOWNLOAD_URL,
            sha256: CONPTY_PACKAGE_SHA256,
            bytes: CONPTY_PACKAGE_BYTES,
        },
    }
}

fn read_archive_entry(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
    max_bytes: u64,
    expected_sha256: &str,
) -> Result<Vec<u8>, ConptyInstallError> {
    let entry = archive.by_name(name)?;
    if entry.size() > max_bytes {
        return Err(ConptyInstallError::InvalidPackage(format!(
            "{name} exceeds its size limit"
        )));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(max_bytes + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max_bytes {
        return Err(ConptyInstallError::InvalidPackage(format!(
            "{name} exceeds its size limit"
        )));
    }
    let actual = Sha256::digest(&bytes);
    let mut actual_hex = String::with_capacity(64);
    for byte in actual {
        let _ = write!(actual_hex, "{byte:02x}");
    }
    if actual_hex != expected_sha256 {
        return Err(ConptyInstallError::InvalidPackage(format!(
            "{name} failed SHA-256 verification"
        )));
    }
    Ok(bytes)
}

pub fn install(
    app_data_dir: &Path,
    package_path: &Path,
) -> Result<WindowsInlineImageStatus, ConptyInstallError> {
    if fs::metadata(package_path)?.len() != CONPTY_PACKAGE_BYTES {
        return Err(ConptyInstallError::InvalidPackage(
            "download has an unexpected size".to_string(),
        ));
    }
    let actual_package_hash = sha256(package_path)?;
    if actual_package_hash != CONPTY_PACKAGE_SHA256 {
        return Err(ConptyInstallError::InvalidPackage(
            "download failed SHA-256 verification".to_string(),
        ));
    }

    let architecture = architecture()?;
    let mut archive = zip::ZipArchive::new(File::open(package_path)?)?;
    let dll = read_archive_entry(
        &mut archive,
        architecture.dll_entry,
        architecture.dll_max_bytes,
        architecture.dll_sha256,
    )?;
    let host = read_archive_entry(
        &mut archive,
        architecture.host_entry,
        architecture.host_max_bytes,
        architecture.host_sha256,
    )?;

    let destination = install_dir(app_data_dir, &architecture);
    let parent = destination
        .parent()
        .expect("versioned ConPTY directory has a parent");
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(
        ".{}-install-{}-{}",
        architecture.name,
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&staging)?;
    let result: Result<(), ConptyInstallError> = (|| {
        File::create(staging.join("conpty.dll"))?.write_all(&dll)?;
        File::create(staging.join("OpenConsole.exe"))?.write_all(&host)?;
        fs::write(staging.join("NOTICE.txt"), NOTICE)?;
        let backup = parent.join(format!(
            ".{}-backup-{}-{}",
            architecture.name,
            std::process::id(),
            NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        if destination.exists() {
            fs::rename(&destination, &backup)?;
        }
        if let Err(error) = fs::rename(&staging, &destination) {
            if backup.exists() {
                let _ = fs::rename(&backup, &destination);
            }
            return Err(error.into());
        }
        let activation = select_installed_runtime(app_data_dir, &architecture)
            .and_then(|()| write_state(app_data_dir, &ConptyState { enabled: true }));
        if let Err(error) = activation {
            lingxia_terminal::terminal_clear_conpty_path();
            let _ = fs::remove_dir_all(&destination);
            if backup.exists() {
                let _ = fs::rename(&backup, &destination);
            }
            return Err(error);
        }
        if backup.exists() {
            let _ = fs::remove_dir_all(backup);
        }
        Ok(())
    })();
    if staging.exists() {
        let _ = fs::remove_dir_all(staging);
    }
    result?;
    Ok(status(app_data_dir))
}

pub fn set_enabled(
    app_data_dir: &Path,
    enabled: bool,
) -> Result<WindowsInlineImageStatus, ConptyInstallError> {
    let architecture = architecture()?;
    if enabled && !installed(app_data_dir, &architecture) {
        return Err(ConptyInstallError::InvalidPackage(
            "the ConPTY compatibility runtime is not installed".to_string(),
        ));
    }
    if enabled {
        select_installed_runtime(app_data_dir, &architecture)?;
        if let Err(error) = write_state(app_data_dir, &ConptyState { enabled: true }) {
            lingxia_terminal::terminal_clear_conpty_path();
            return Err(error);
        }
    } else {
        lingxia_terminal::terminal_clear_conpty_path();
        write_state(app_data_dir, &ConptyState { enabled: false })?;
    }
    Ok(status(app_data_dir))
}

fn select_installed_runtime(
    app_data_dir: &Path,
    architecture: &Architecture,
) -> Result<(), ConptyInstallError> {
    lingxia_terminal::terminal_set_conpty_path(
        install_dir(app_data_dir, architecture).join("conpty.dll"),
    )
    .map_err(ConptyInstallError::Load)
}

pub fn activate(app_data_dir: &Path) -> Result<(), ConptyInstallError> {
    let architecture = architecture()?;
    if read_state(app_data_dir).enabled && installed(app_data_dir, &architecture) {
        select_installed_runtime(app_data_dir, &architecture)
    } else {
        lingxia_terminal::terminal_clear_conpty_path();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_status_does_not_claim_missing_runtime_is_enabled() {
        let directory = tempfile::tempdir().expect("temp directory");
        let status = status(directory.path());
        assert!(!status.enabled);
        assert!(!status.installed);
        assert_eq!(status.package.version, CONPTY_VERSION);
    }

    #[test]
    fn enabling_requires_a_verified_install() {
        let directory = tempfile::tempdir().expect("temp directory");
        let error = set_enabled(directory.path(), true).expect_err("missing install");
        assert!(matches!(error, ConptyInstallError::InvalidPackage(_)));
    }

    #[test]
    fn state_can_be_replaced_repeatedly() {
        let directory = tempfile::tempdir().expect("temp directory");
        write_state(directory.path(), &ConptyState { enabled: true }).expect("first write");
        write_state(directory.path(), &ConptyState { enabled: false }).expect("replacement write");
        assert!(!read_state(directory.path()).enabled);
    }
}
