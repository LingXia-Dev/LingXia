//! The shim that makes the product's command typable.
//!
//! The real executable lives inside an application bundle, which is neither on
//! `PATH` nor pleasant to type. The launcher is generated at runtime so it
//! always names the executable actually running: a development build moves, a
//! release does not.
//!
//! Unix needs only a shell script. Windows needs a tiny native forwarder:
//! batch and PowerShell relays cannot preserve arbitrary Unicode argv and
//! binary standard streams. The forwarder contains no product logic; it starts
//! this same product executable with the invocation contract in its environment.

use std::path::{Path, PathBuf};

use lingxia_control_protocol::invocation;

const RECORDED_NAME: &str = ".lingxia-launcher-name";

/// Directory holding the launcher, added to `PATH` by the user.
pub fn bin_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("bin")
}

/// The stable launcher path a host may publish through its own installer or
/// agent integration. The host owns that external locator; LingXia owns only
/// the launcher inside product state.
pub fn path(state_dir: &Path) -> std::io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let name = invocation::command_name(&executable_stem(&executable));
    Ok(bin_dir(state_dir).join(launcher_file_name(&name)))
}

/// Write the launcher, returning where it landed.
pub fn install(state_dir: &Path, endpoint: &str) -> std::io::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let name = invocation::command_name(&executable_stem(&executable));
    let directory = bin_dir(state_dir);
    std::fs::create_dir_all(&directory)?;

    cleanup_previous(state_dir, &directory, &name)?;
    cleanup_retired(&directory, &name);
    for artifact in artifacts(&name, &executable, endpoint)? {
        let path = directory.join(&artifact.file_name);
        if std::fs::read(&path).unwrap_or_default() != artifact.contents {
            write_artifact(&path, &artifact.contents)?;
            #[cfg(unix)]
            if artifact.executable {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    remove_obsolete_windows_scripts(&directory, &name)?;
    write_artifact(&state_dir.join(RECORDED_NAME), name.as_bytes())?;
    Ok(directory.join(launcher_file_name(&name)))
}

/// Remove every launcher positively recorded as belonging to this product.
pub fn remove(state_dir: &Path) -> std::io::Result<()> {
    let executable = std::env::current_exe()?;
    let current = invocation::command_name(&executable_stem(&executable));
    let recorded = recorded_name(state_dir);
    let directory = bin_dir(state_dir);
    let mut first_error = None;

    for name in recorded.iter().chain(std::iter::once(&current)) {
        remove_artifacts(&directory, name, &mut first_error);
    }
    remove_path(state_dir.join(RECORDED_NAME), &mut first_error);

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn cleanup_previous(state_dir: &Path, directory: &Path, current: &str) -> std::io::Result<()> {
    let Some(previous) = recorded_name(state_dir) else {
        return Ok(());
    };
    if previous == current {
        return Ok(());
    }
    let mut error = None;
    remove_artifacts(directory, &previous, &mut error);
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn recorded_name(state_dir: &Path) -> Option<String> {
    let name = std::fs::read_to_string(state_dir.join(RECORDED_NAME)).ok()?;
    let name = name.trim();
    (!name.is_empty() && invocation::command_name(name) == name).then(|| name.to_string())
}

fn remove_artifacts(directory: &Path, name: &str, first_error: &mut Option<std::io::Error>) {
    cleanup_retired(directory, name);
    for file_name in artifact_file_names(name) {
        remove_path(directory.join(file_name), first_error);
    }
}

#[cfg(windows)]
fn cleanup_retired(directory: &Path, name: &str) {
    let prefix = format!(".{name}.exe.retired-");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().starts_with(&prefix) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(not(windows))]
fn cleanup_retired(_directory: &Path, _name: &str) {}

fn remove_path(path: PathBuf, first_error: &mut Option<std::io::Error>) {
    let removed = std::fs::remove_file(&path).or_else(|error| {
        #[cfg(windows)]
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return retire_running_executable(&path);
        }
        Err(error)
    });
    if let Err(error) = removed
        && error.kind() != std::io::ErrorKind::NotFound
        && first_error.is_none()
    {
        *first_error = Some(error);
    }
}

/// Move a mapped launcher away from its command name while it finishes waiting
/// for the product CLI. A later install or removal deletes the inert tombstone.
#[cfg(windows)]
fn retire_running_executable(path: &Path) -> std::io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_RETIRED: AtomicU64 = AtomicU64::new(0);

    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("launcher.exe");
    for _ in 0..32 {
        let sequence = NEXT_RETIRED.fetch_add(1, Ordering::Relaxed);
        let retired = directory.join(format!(
            ".{file_name}.retired-{}-{sequence}",
            std::process::id()
        ));
        match std::fs::rename(path, retired) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("cannot reserve a retired name for {}", path.display()),
    ))
}

#[cfg(not(windows))]
fn write_artifact(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(windows)]
fn write_artifact(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::HSTRING;

    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    static WRITE_LOCK: Mutex<()> = Mutex::new(());

    let _write_guard = WRITE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("file");
    let (temporary, mut file) = (0..32)
        .find_map(|_| {
            let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
            let temporary =
                path.with_extension(format!("{extension}.tmp-{}-{sequence}", std::process::id()));
            match std::fs::OpenOptions::new()
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
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    let from = HSTRING::from(temporary.as_os_str());
    let to = HSTRING::from(path.as_os_str());
    if let Err(error) = unsafe {
        MoveFileExW(
            &from,
            &to,
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } {
        let _ = std::fs::remove_file(temporary);
        return Err(std::io::Error::other(error.to_string()));
    }
    Ok(())
}

fn executable_stem(executable: &Path) -> String {
    let name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app");
    name.strip_suffix(std::env::consts::EXE_SUFFIX)
        .unwrap_or(name)
        .to_string()
}

struct Artifact {
    file_name: String,
    contents: Vec<u8>,
    #[cfg_attr(windows, allow(dead_code))]
    executable: bool,
}

#[cfg(not(windows))]
fn launcher_file_name(name: &str) -> String {
    name.to_string()
}

#[cfg(windows)]
fn launcher_file_name(name: &str) -> String {
    format!("{name}.exe")
}

#[cfg(not(windows))]
fn artifact_file_names(name: &str) -> Vec<String> {
    vec![name.to_string()]
}

#[cfg(windows)]
fn artifact_file_names(name: &str) -> Vec<String> {
    vec![
        format!("{name}.exe"),
        format!("{name}.control"),
        // Clean up launchers written by pre-native builds of this feature.
        format!("{name}.cmd"),
        format!("{name}.ps1"),
    ]
}

#[cfg(not(windows))]
fn artifacts(name: &str, executable: &Path, endpoint: &str) -> std::io::Result<Vec<Artifact>> {
    let script = format!(
        "#!/bin/sh\n# Generated by LingXia; points at the running executable.\n{}={} exec -a {} {} \"$@\" {}\n",
        invocation::ENDPOINT,
        shell_quote(endpoint),
        shell_quote(name),
        shell_quote(&executable.to_string_lossy()),
        invocation::CLI_ARGUMENT,
    );
    Ok(vec![Artifact {
        file_name: name.to_string(),
        contents: script.into_bytes(),
        executable: true,
    }])
}

#[cfg(windows)]
const WINDOWS_LAUNCHER: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/lingxia-control-launcher.exe"));

#[cfg(windows)]
const CONFIG_MAGIC: &[u8] = b"LXCL\x01\r\n";

#[cfg(windows)]
fn artifacts(name: &str, executable: &Path, endpoint: &str) -> std::io::Result<Vec<Artifact>> {
    Ok(vec![
        Artifact {
            file_name: format!("{name}.control"),
            contents: windows_config(executable, endpoint)?,
            executable: false,
        },
        Artifact {
            file_name: format!("{name}.exe"),
            contents: WINDOWS_LAUNCHER.to_vec(),
            executable: true,
        },
    ])
}

#[cfg(windows)]
fn windows_config(executable: &Path, endpoint: &str) -> std::io::Result<Vec<u8>> {
    use std::os::windows::ffi::OsStrExt;

    let mut contents = CONFIG_MAGIC.to_vec();
    for value in [executable.as_os_str(), std::ffi::OsStr::new(endpoint)] {
        let units = value.encode_wide().collect::<Vec<_>>();
        let len = u32::try_from(units.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "launcher value is too long",
            )
        })?;
        contents.extend_from_slice(&len.to_le_bytes());
        for unit in units {
            contents.extend_from_slice(&unit.to_le_bytes());
        }
    }
    Ok(contents)
}

#[cfg(windows)]
fn remove_obsolete_windows_scripts(directory: &Path, name: &str) -> std::io::Result<()> {
    let mut error = None;
    for extension in ["cmd", "ps1"] {
        remove_path(directory.join(format!("{name}.{extension}")), &mut error);
    }
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(not(windows))]
fn remove_obsolete_windows_scripts(_directory: &Path, _name: &str) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn the_script_names_the_executable_the_cli_argument_and_the_endpoint() {
        let artifacts = artifacts(
            "foo",
            Path::new("/Apps/Foo.app/MacOS/Foo"),
            "/tmp/control.sock",
        )
        .unwrap();
        let body = String::from_utf8(artifacts[0].contents.clone()).unwrap();
        assert_eq!(artifacts[0].file_name, "foo");
        assert!(body.contains("/Apps/Foo.app/MacOS/Foo"));
        assert!(body.contains(invocation::CLI_ARGUMENT));
        assert!(body.contains("/tmp/control.sock"));
    }

    #[cfg(windows)]
    fn decode_windows_config(contents: &[u8]) -> (std::ffi::OsString, std::ffi::OsString) {
        use std::os::windows::ffi::OsStringExt;

        assert!(contents.starts_with(CONFIG_MAGIC));
        let mut cursor = CONFIG_MAGIC.len();
        let mut field = || {
            let len = u32::from_le_bytes(contents[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;
            let units = contents[cursor..cursor + len * 2]
                .chunks_exact(2)
                .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                .collect::<Vec<_>>();
            cursor += len * 2;
            std::ffi::OsString::from_wide(&units)
        };
        let decoded = (field(), field());
        assert_eq!(cursor, contents.len());
        decoded
    }

    #[cfg(windows)]
    #[test]
    fn native_launcher_config_preserves_unicode_and_shell_metacharacters() {
        let executable = Path::new(r"C:\用户\%TEMP%\Bang!\O'Brien\Foo.exe");
        let contents = windows_config(executable, r"\\.\pipe\lingxia-demo-7").unwrap();
        let (target, endpoint) = decode_windows_config(&contents);
        assert_eq!(target, executable.as_os_str());
        assert_eq!(endpoint, r"\\.\pipe\lingxia-demo-7");
        assert!(WINDOWS_LAUNCHER.starts_with(b"MZ"));
    }

    #[cfg(windows)]
    #[test]
    fn native_launcher_waits_for_and_returns_the_child_status() {
        let directory =
            std::env::temp_dir().join(format!("lingxia-native-launcher-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let launcher = directory.join("probe.exe");
        let config = launcher.with_extension("control");
        let probe = directory.join("probe.cmd");
        let command = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
        std::fs::write(
            &probe,
            format!(
                "@echo off\r\nif not \"%1\"==\"{}\" exit /b 8\r\nif not \"%{}%\"==\"test-endpoint\" exit /b 9\r\nexit /b 7\r\n",
                invocation::CLI_ARGUMENT,
                invocation::ENDPOINT,
            ),
        )
        .unwrap();
        std::fs::write(
            &config,
            windows_config(Path::new(&command), "test-endpoint").unwrap(),
        )
        .unwrap();
        std::fs::write(&launcher, WINDOWS_LAUNCHER).unwrap();

        let status = std::process::Command::new(&launcher)
            .args([std::ffi::OsStr::new("/d"), std::ffi::OsStr::new("/c")])
            .arg(&probe)
            .env_remove(invocation::ENDPOINT)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(7));

        let _ = std::fs::remove_file(probe);
        let _ = std::fs::remove_file(config);
        let _ = std::fs::remove_file(launcher);
        let _ = std::fs::remove_dir(directory);
    }

    #[cfg(windows)]
    #[test]
    fn a_live_launcher_is_retired_during_control_disable() {
        let directory = std::env::temp_dir().join(format!(
            "lingxia-live-launcher-delete-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let launcher = directory.join("probe.exe");
        let config = launcher.with_extension("control");
        let probe = directory.join("wait.cmd");
        let ready = directory.join("ready");
        let release = directory.join("release");
        let command = std::env::var_os("ComSpec").unwrap_or_else(|| "cmd.exe".into());
        std::fs::write(
            &probe,
            "@echo off\r\ntype nul > \"%~1\"\r\n:wait\r\nif exist \"%~2\" exit /b 0\r\nping 127.0.0.1 -n 2 > nul\r\ngoto wait\r\n",
        )
        .unwrap();
        std::fs::write(
            &config,
            windows_config(Path::new(&command), "test-endpoint").unwrap(),
        )
        .unwrap();
        std::fs::write(&launcher, WINDOWS_LAUNCHER).unwrap();

        let mut child = std::process::Command::new(&launcher)
            .args([std::ffi::OsStr::new("/d"), std::ffi::OsStr::new("/c")])
            .arg(&probe)
            .arg(&ready)
            .arg(&release)
            .spawn()
            .unwrap();
        let ready_seen = (0..200).any(|_| {
            if ready.exists() {
                true
            } else {
                std::thread::sleep(std::time::Duration::from_millis(25));
                false
            }
        });
        let mut remove_error = None;
        remove_path(launcher.clone(), &mut remove_error);
        let retired = std::fs::read_dir(&directory)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(".probe.exe.retired-"))
            })
            .collect::<Vec<_>>();
        std::fs::write(&release, b"").unwrap();
        let status = child.wait().unwrap();

        assert!(ready_seen, "the launcher child never became ready");
        assert!(
            remove_error.is_none(),
            "retirement failed: {remove_error:?}"
        );
        assert!(
            !launcher.exists(),
            "the running launcher name remained on PATH"
        );
        assert_eq!(
            retired.len(),
            1,
            "the launcher was not retired exactly once"
        );
        assert!(status.success());
        cleanup_retired(&directory, "probe");
        assert!(retired.iter().all(|path| !path.exists()));

        let _ = std::fs::remove_file(release);
        let _ = std::fs::remove_file(ready);
        let _ = std::fs::remove_file(probe);
        let _ = std::fs::remove_file(config);
        let _ = std::fs::remove_dir(directory);
    }

    #[cfg(windows)]
    #[test]
    fn concurrent_artifact_updates_leave_one_complete_file_and_no_temporaries() {
        let directory =
            std::env::temp_dir().join(format!("lingxia-artifact-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("launcher.bin");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut updates = Vec::new();

        for value in 0u8..8 {
            let path = path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            updates.push(std::thread::spawn(move || {
                let contents = vec![value; 4096];
                barrier.wait();
                write_artifact(&path, &contents).unwrap();
                contents
            }));
        }

        let candidates = updates
            .into_iter()
            .map(|update| update.join().unwrap())
            .collect::<Vec<_>>();
        let installed = std::fs::read(&path).unwrap();
        assert!(candidates.contains(&installed));
        assert!(
            std::fs::read_dir(&directory)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().contains(".tmp-"))
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn installing_is_idempotent_and_removing_is_forgiving() {
        let state_dir =
            std::env::temp_dir().join(format!("lingxia-launcher-{}", std::process::id()));
        std::fs::create_dir_all(&state_dir).unwrap();
        let first = install(&state_dir, "/tmp/control.sock").unwrap();
        let second = install(&state_dir, "/tmp/control.sock").unwrap();
        assert_eq!(first, second);
        assert!(first.exists());
        #[cfg(windows)]
        assert!(first.with_extension("control").exists());
        remove(&state_dir).unwrap();
        assert!(!first.exists());
        #[cfg(windows)]
        assert!(!first.with_extension("control").exists());
        remove(&state_dir).unwrap();
        let _ = std::fs::remove_dir_all(&state_dir);
    }

    #[cfg(windows)]
    #[test]
    fn a_recorded_old_name_is_removed_after_an_executable_rename() {
        let state_dir =
            std::env::temp_dir().join(format!("lingxia-renamed-launcher-{}", std::process::id()));
        let directory = bin_dir(&state_dir);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(state_dir.join(RECORDED_NAME), "old-name").unwrap();
        for file_name in artifact_file_names("old-name") {
            std::fs::write(directory.join(file_name), b"old").unwrap();
        }

        cleanup_previous(&state_dir, &directory, "new-name").unwrap();
        for file_name in artifact_file_names("old-name") {
            assert!(!directory.join(file_name).exists());
        }
        let _ = std::fs::remove_dir_all(state_dir);
    }
}
