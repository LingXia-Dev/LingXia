//! `lxdev test --isolate / --state / --save-state`: run a suite on an
//! isolated data profile of the app, seeded from and saved to snapshots.
//!
//! The host does the isolating (it points the app's data at a throwaway
//! profile and restores it when the run ends); this side validates flags and
//! files before the run exists, moves snapshots over `session.profile.*`, and
//! writes them where only the developer can read them.

use crate::client::execute_command;
use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use clap::{Args, ValueEnum};
use lingxia_control_protocol::{dev_session::session_test::*, methods};
use owo_colors::OwoColorize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const STATE_EXT: &str = "lxstate";
/// A snapshot this old likely holds expired sessions; say so.
const STALE_AFTER: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Args, Debug, Clone, Default)]
pub struct StateOptions {
    /// Run the app on an isolated, empty data profile. Its own data is not
    /// touched and is back in place when the run ends
    #[arg(long)]
    pub isolate: bool,

    /// Start from a saved snapshot (implies --isolate): a NAME kept under
    /// ~/.lingxia/test-state, or a PATH to a .lxstate file
    #[arg(long, value_name = "NAME|PATH")]
    pub state: Option<String>,

    /// Save the isolated data after the run (implies --isolate). With the
    /// same --state, a missing snapshot starts empty and is created
    #[arg(long, value_name = "NAME|PATH")]
    pub save_state: Option<String>,

    /// When --save-state writes the snapshot
    #[arg(long, value_enum, default_value_t = SaveOn::Pass, requires = "save_state")]
    pub save_state_on: SaveOn,
}

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaveOn {
    /// Only after a run that passed
    #[default]
    Pass,
    /// After any run that finished
    Always,
}

impl StateOptions {
    pub fn isolated(&self) -> bool {
        self.isolate || self.state.is_some() || self.save_state.is_some()
    }

    /// Flags a rerun hint must repeat so the rerun sees the same data.
    ///
    /// A snapshot saved back to where it was read from is a rolling one: the
    /// app may rotate what it holds (a refresh token used once), so a rerun
    /// that read it without saving would leave the next run a stale copy.
    /// Such a rerun saves it the same way. Any other `--save-state` is left
    /// out: a one-spec rerun must not overwrite a snapshot a full run made.
    pub fn rerun_flags(&self, quote: impl Fn(&str) -> String) -> String {
        let mut flags = String::new();
        if let Some(state) = &self.state {
            flags.push_str(&format!(" --state {}", quote(state)));
            if self.save_state.as_ref() == Some(state) {
                flags.push_str(&format!(" --save-state {}", quote(state)));
                if self.save_state_on == SaveOn::Always {
                    flags.push_str(" --save-state-on always");
                }
            }
        } else if self.isolated() {
            flags.push_str(" --isolate");
        }
        flags
    }
}

/// Whether a `--state`/`--save-state` value is a PATH rather than a NAME.
pub fn names_a_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\') || value.ends_with(&format!(".{STATE_EXT}"))
}

/// Where a `--state`/`--save-state` value points.
fn resolve(value: &str, project_root: &Path) -> Result<PathBuf> {
    if names_a_path(value) {
        return Ok(PathBuf::from(value));
    }
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if !valid || value.starts_with('.') {
        bail!("invalid state name {value:?}: use letters, digits, '-', '_' or '.'");
    }
    Ok(state_home()?
        .join(project_key(project_root))
        .join(format!("{value}.{STATE_EXT}")))
}

/// `~/.lingxia/test-state`: outside every project, so a snapshot (which
/// can hold sign-in tokens) is never committed by accident.
fn state_home() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| anyhow!("cannot locate the home directory for saved test state"))?;
    Ok(PathBuf::from(home).join(".lingxia").join("test-state"))
}

/// `<dir-name>-<hash>` of the project, stable across runs.
fn project_key(project_root: &Path) -> String {
    // A relative entry run from the project itself resolves its root to "".
    let project_root = if project_root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        project_root
    };
    let canonical = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let name = canonical
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());
    let slug: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!(
        "{slug}-{}",
        &sha256_hex(canonical.to_string_lossy().as_bytes())[..12]
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A snapshot path inside the project that git does not ignore would be one
/// `git add .` away from a commit.
fn in_project_warning(path: &Path, project_root: &Path) -> Option<String> {
    let absolute = std::path::absolute(path).ok()?;
    let root = std::path::absolute(project_root).ok()?;
    if !absolute.starts_with(&root) {
        return None;
    }
    let ignored = std::process::Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["check-ignore", "-q"])
        .arg(&absolute)
        .status()
        .is_ok_and(|status| status.success());
    (!ignored).then(|| {
        format!(
            "{} is inside the project and not ignored by git; a snapshot can hold sign-in \
             tokens. Add `*.{STATE_EXT}` to .gitignore, or use a NAME to keep it under \
             ~/.lingxia/test-state.",
            path.display()
        )
    })
}

fn execute_typed<A: Serialize, R: DeserializeOwned>(
    ws_url: &str,
    handler: &str,
    args: &A,
) -> Result<R> {
    let args = serde_json::to_value(args)?;
    let value = execute_command(ws_url, handler, Some(args))?
        .ok_or_else(|| anyhow!("{handler} returned no data"))?;
    serde_json::from_value(value).with_context(|| format!("invalid {handler} response"))
}

/// Isolation settled before the run starts.
pub struct PreparedState {
    ws_url: String,
    pub profile: TestProfileArgs,
    /// Run controls the report records (`meta.run`): never paths or contents.
    pub control: Vec<(String, String)>,
    save: Option<PathBuf>,
    save_on: SaveOn,
}

/// Validate the flags, the seed and the save target, confirm the host can
/// isolate, and upload the seed. Everything that can fail locally fails here,
/// before a run exists.
pub fn prepare(
    ws_url: &str,
    options: &StateOptions,
    project_root: &Path,
    machine: bool,
) -> Result<Option<PreparedState>> {
    if !options.isolated() {
        return Ok(None);
    }
    let save = options
        .save_state
        .as_deref()
        .map(|value| resolve(value, project_root))
        .transpose()?;
    if let Some(save) = &save {
        let parent = save
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        create_private_dir(parent)
            .with_context(|| format!("cannot create {} for --save-state", parent.display()))?;
        if !machine && let Some(warning) = in_project_warning(save, project_root) {
            eprintln!("{} {warning}", "warning".yellow());
        }
    }
    let seed = match options.state.as_deref() {
        None => None,
        Some(value) => {
            let path = resolve(value, project_root)?;
            if path.is_file() {
                Some(path)
            } else if save.as_ref() == Some(&path) {
                if !machine {
                    eprintln!(
                        "{} no saved state at {} yet; starting empty and saving it after the run",
                        "test".cyan(),
                        path.display()
                    );
                }
                None
            } else {
                bail!("no saved test state at {}", path.display());
            }
        }
    };

    let capabilities = require_profile_capability(ws_url)?;
    let mut control = vec![("profile".to_string(), "isolated".to_string())];
    let mut seed_state_id = None;
    if let Some(seed) = &seed {
        let bytes =
            std::fs::read(seed).with_context(|| format!("failed to read {}", seed.display()))?;
        if bytes.len() as u64 > capabilities.max_state_bytes {
            bail!(
                "{} is {} bytes; the host accepts at most {}",
                seed.display(),
                bytes.len(),
                capabilities.max_state_bytes
            );
        }
        if !machine && let Some(age) = age(seed).filter(|age| *age > STALE_AFTER) {
            eprintln!(
                "{} {} is {} days old; its sign-in may have expired",
                "warning".yellow(),
                seed.display(),
                age.as_secs() / 86_400
            );
        }
        let digest = sha256_hex(&bytes);
        let name = seed
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        control.push((
            "profile_seed".to_string(),
            format!("{name}@{}", &digest[..8]),
        ));
        seed_state_id = Some(upload(ws_url, &bytes, &digest, capabilities.chunk_bytes)?);
    }
    Ok(Some(PreparedState {
        ws_url: ws_url.to_string(),
        profile: TestProfileArgs {
            isolate: true,
            seed_state_id,
            retain: save.is_some(),
            appid: None,
        },
        control,
        save,
        save_on: options.save_state_on,
    }))
}

/// A host that predates isolation would ignore the start field and run on
/// the developer's real data. Refuse instead.
fn require_profile_capability(ws_url: &str) -> Result<ProfileCapability> {
    let capabilities: TestCapabilities =
        execute_typed(ws_url, methods::session::test::CAPABILITIES, &json!({})).map_err(
            |error| {
                if error.to_string().contains("unknown session.test handler") {
                    anyhow!(
                        "this host cannot isolate a test run (it predates \
                     `session.test.capabilities`); rebuild it with the current LingXia, \
                     or drop --isolate/--state/--save-state"
                    )
                } else {
                    error
                }
            },
        )?;
    capabilities
        .profile
        .ok_or_else(|| anyhow!("this host does not support isolated test runs"))
}

fn age(path: &Path) -> Option<Duration> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
}

fn upload(ws_url: &str, bytes: &[u8], digest: &str, chunk_bytes: u64) -> Result<String> {
    let upload_id = format!(
        "lxdev-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    );
    let chunk = usize::try_from(chunk_bytes.max(1)).unwrap_or(usize::MAX);
    let mut offset = 0usize;
    loop {
        let end = (offset + chunk).min(bytes.len());
        let done = end == bytes.len();
        let response: ProfileUploadResponse = execute_typed(
            ws_url,
            methods::session::profile::UPLOAD,
            &ProfileUploadArgs {
                upload_id: upload_id.clone(),
                offset: offset as u64,
                base64: BASE64.encode(&bytes[offset..end]),
                done,
                sha256: done.then(|| digest.to_string()),
            },
        )
        .context("failed to upload the saved test state")?;
        if done {
            return response
                .state_id
                .ok_or_else(|| anyhow!("the host did not accept the saved test state"));
        }
        offset = end;
    }
}

impl PreparedState {
    /// Delete the uploaded seed when the run never started (the host
    /// consumes it at start otherwise).
    pub fn discard_seed(&self) {
        if let Some(state_id) = &self.profile.seed_state_id {
            let _ = execute_command(
                &self.ws_url,
                methods::session::profile::DISCARD,
                serde_json::to_value(ProfileDiscardArgs {
                    state_id: Some(state_id.clone()),
                    run_id: None,
                })
                .ok(),
            );
        }
    }

    /// Guard for the profile the host retains for this run: dropped
    /// without [`RetainedProfile::release`], it discards it on the host.
    pub fn retained(&self, run_id: &str) -> Option<RetainedProfile> {
        self.profile.retain.then(|| RetainedProfile {
            ws_url: self.ws_url.clone(),
            run_id: run_id.to_string(),
            armed: true,
        })
    }

    /// After the run: save the snapshot when asked to and the outcome
    /// allows, then release the host's copy. Returns the saved path.
    pub fn finish(
        &self,
        retained: Option<RetainedProfile>,
        state: TestRunState,
        partial: bool,
        machine: bool,
    ) -> Result<Option<PathBuf>> {
        let (Some(save), Some(mut retained)) = (&self.save, retained) else {
            return Ok(None);
        };
        let wanted = match self.save_on {
            SaveOn::Always => state.is_terminal(),
            SaveOn::Pass => state == TestRunState::Passed && !partial,
        };
        if !wanted {
            if !machine {
                eprintln!(
                    "{} run did not pass; {} left unchanged",
                    "test".cyan(),
                    save.display()
                );
            }
            return Ok(None);
        }
        let bytes = export(&self.ws_url, &retained.run_id)?;
        write_private_atomic(save, &bytes)?;
        retained.release();
        if !machine {
            eprintln!("{} saved test state to {}", "test".cyan(), save.display());
        }
        Ok(Some(save.clone()))
    }
}

fn export(ws_url: &str, run_id: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut expected = None;
    loop {
        let response: ProfileExportResponse = execute_typed(
            ws_url,
            methods::session::profile::EXPORT,
            &ProfileExportArgs {
                run_id: run_id.to_string(),
                offset: bytes.len() as u64,
            },
        )
        .context("failed to export the run's test state")?;
        bytes.extend(
            BASE64
                .decode(response.base64.as_bytes())
                .context("invalid test state chunk")?,
        );
        expected.get_or_insert(response.sha256);
        if response.done || bytes.len() as u64 >= response.total {
            break;
        }
    }
    let expected = expected.unwrap_or_default();
    let actual = sha256_hex(&bytes);
    if !actual.eq_ignore_ascii_case(&expected) {
        bail!("exported test state is corrupt: digest {actual}, expected {expected}");
    }
    Ok(bytes)
}

/// The profile the host keeps for a `--save-state` run.
pub struct RetainedProfile {
    ws_url: String,
    run_id: String,
    armed: bool,
}

impl RetainedProfile {
    /// Discard the host's copy now; the drop will not do it again.
    pub fn release(&mut self) {
        if std::mem::replace(&mut self.armed, false) {
            let _ = execute_command(
                &self.ws_url,
                methods::session::profile::DISCARD,
                serde_json::to_value(ProfileDiscardArgs {
                    run_id: Some(self.run_id.clone()),
                    state_id: None,
                })
                .ok(),
            );
        }
    }
}

impl Drop for RetainedProfile {
    fn drop(&mut self) {
        self.release();
    }
}

fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    if let Ok(home) = state_home()
        && dir.starts_with(&home)
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Write `bytes` to `path` through a 0600 temp file and a rename, so a
/// crash never leaves a half-written snapshot where a good one was.
fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    create_private_dir(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
        std::process::id()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let written = options.open(&tmp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(err) = written.and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(err).with_context(|| format!("failed to write {}", path.display()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use lingxia_control_protocol::{ControlResponse, dev_session::DevSessionMessage};

    #[derive(Parser)]
    struct Harness {
        #[command(flatten)]
        state: StateOptions,
    }

    #[test]
    fn a_project_run_from_its_own_directory_is_keyed_by_that_directory() {
        let here = std::env::current_dir().unwrap().canonicalize().unwrap();
        let name = here.file_name().unwrap().to_string_lossy().to_string();
        let key = project_key(Path::new(""));
        assert_eq!(key, project_key(&here));
        assert!(key.starts_with(&name.replace(|c: char| !c.is_ascii_alphanumeric(), "-")));
        // Not the key of an empty path.
        assert!(!key.ends_with(&sha256_hex(b"")[..12]));
    }

    fn parse(args: &[&str]) -> Result<StateOptions, clap::Error> {
        Harness::try_parse_from(std::iter::once("test").chain(args.iter().copied()))
            .map(|harness| harness.state)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lxdev-state-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn state_flags_imply_isolation() {
        assert!(!parse(&[]).unwrap().isolated());
        assert!(parse(&["--isolate"]).unwrap().isolated());
        assert!(parse(&["--state", "auth"]).unwrap().isolated());
        let saving = parse(&["--save-state", "auth", "--save-state-on", "always"]).unwrap();
        assert!(saving.isolated());
        assert_eq!(saving.save_state_on, SaveOn::Always);
        assert!(
            parse(&["--save-state-on", "always"]).is_err(),
            "--save-state-on needs --save-state"
        );
    }

    #[test]
    fn rerun_keeps_the_isolation_flags() {
        let quote = |value: &str| format!("'{value}'");
        assert_eq!(parse(&[]).unwrap().rerun_flags(quote), "");
        assert_eq!(
            parse(&["--isolate"]).unwrap().rerun_flags(quote),
            " --isolate"
        );
        // A rolling snapshot is read and saved back the same way.
        assert_eq!(
            parse(&["--state", "auth", "--save-state", "auth"])
                .unwrap()
                .rerun_flags(quote),
            " --state 'auth' --save-state 'auth'"
        );
        assert_eq!(
            parse(&[
                "--state",
                "auth",
                "--save-state",
                "auth",
                "--save-state-on",
                "always"
            ])
            .unwrap()
            .rerun_flags(quote),
            " --state 'auth' --save-state 'auth' --save-state-on always"
        );
        // A rerun must not overwrite a snapshot made by another source.
        assert_eq!(
            parse(&["--save-state", "auth"]).unwrap().rerun_flags(quote),
            " --isolate"
        );
        assert_eq!(
            parse(&["--state", "seed", "--save-state", "auth"])
                .unwrap()
                .rerun_flags(quote),
            " --state 'seed'"
        );
    }

    #[test]
    fn names_live_outside_the_project_and_paths_stay_as_given() {
        let project = scratch("project");
        let named = resolve("auth", &project).unwrap();
        assert!(named.starts_with(state_home().unwrap()));
        assert!(!named.starts_with(&project));
        assert_eq!(named.extension().unwrap(), STATE_EXT);
        assert_eq!(
            resolve("auth", &project).unwrap(),
            named,
            "stable per project"
        );
        assert_eq!(
            resolve("./a.lxstate", &project).unwrap(),
            PathBuf::from("./a.lxstate")
        );
        assert_eq!(
            resolve("x.lxstate", &project).unwrap(),
            PathBuf::from("x.lxstate")
        );
        assert!(resolve("..", &project).is_err());
        assert!(resolve("a b", &project).is_err());
        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn a_snapshot_inside_the_project_is_flagged_unless_ignored() {
        let project = scratch("inside");
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&project)
                .args(args)
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        let path = project.join("auth.lxstate");
        assert!(in_project_warning(&path, &project).is_some());
        std::fs::write(project.join(".gitignore"), "*.lxstate\n").unwrap();
        assert!(in_project_warning(&path, &project).is_none());
        assert!(in_project_warning(&std::env::temp_dir().join("x.lxstate"), &project).is_none());
        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn saved_state_is_written_atomically_and_private() {
        let dir = scratch("write");
        let path = dir.join("nested").join("auth.lxstate");
        write_private_atomic(&path, b"first").unwrap();
        write_private_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Accepts one command and records `(method, params)`.
    fn one_shot_server(
        answer: serde_json::Value,
    ) -> (String, std::thread::JoinHandle<(String, serde_json::Value)>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            loop {
                let message = socket.read().unwrap();
                let Ok(DevSessionMessage::Request(request)) =
                    serde_json::from_str(message.to_text().unwrap())
                else {
                    continue;
                };
                let seen = (
                    request.method.clone(),
                    request.params.clone().unwrap_or_default(),
                );
                let response =
                    DevSessionMessage::Response(ControlResponse::success(request.id, Some(answer)));
                socket
                    .send(tungstenite::Message::Text(
                        serde_json::to_string(&response).unwrap().into(),
                    ))
                    .unwrap();
                return seen;
            }
        });
        (url, handle)
    }

    #[test]
    fn a_retained_profile_is_discarded_when_the_client_gives_up() {
        let (url, server) = one_shot_server(json!({ "discarded": true }));
        let prepared = PreparedState {
            ws_url: url,
            profile: TestProfileArgs {
                isolate: true,
                retain: true,
                ..Default::default()
            },
            control: Vec::new(),
            save: None,
            save_on: SaveOn::Pass,
        };
        drop(prepared.retained("run-1").expect("retained"));
        let (method, params) = server.join().unwrap();
        assert_eq!(method, methods::session::profile::DISCARD);
        assert_eq!(params["run_id"], "run-1");
    }

    #[test]
    fn isolation_is_refused_on_a_host_without_the_capability() {
        let (url, server) = one_shot_server(json!({}));
        let error = require_profile_capability(&url).unwrap_err();
        assert!(error.to_string().contains("does not support isolated"));
        assert_eq!(
            server.join().unwrap().0,
            methods::session::test::CAPABILITIES
        );
    }
}
