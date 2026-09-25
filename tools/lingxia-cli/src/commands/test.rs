//! `lingxia test`: the one-shot CI lifecycle — start a dev session in the
//! background, run `lxdev test` in it, stop it. The exit code is the test
//! run's; the session is stopped on every way out (pass, failure, an error,
//! Ctrl-C) unless `--keep-session`.
//!
//! `lxdev test` stays the primitive that attaches to a live session; this
//! command only owns the session's lifetime around it.

use super::dev::log_store::{self, SessionInfo};
use super::dev::{DevProject, RUNNER_TARGET, is_runner_target};
use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct TestExecuteOptions {
    pub entry: Option<String>,
    pub preset: Option<String>,
    pub headless: bool,
    pub keep_session: bool,
    pub platform: Option<String>,
    pub name: Option<String>,
    pub device: Option<String>,
    pub display_language: Option<String>,
    /// `lingxia dev` build flags, as given (`--release`, `--skip-native`, …).
    pub dev_flags: Vec<String>,
    /// Arguments after `--`, passed to `lxdev test` unchanged.
    pub lxdev_args: Vec<String>,
}

/// What `run_once` needs from the world; faked in tests.
trait SessionOps {
    fn start(&mut self) -> Result<SessionInfo>;
    /// `lxdev test`'s exit code.
    fn run(&mut self, session: &SessionInfo) -> Result<i32>;
    fn stop(&mut self, session: &SessionInfo) -> Result<()>;
}

/// Stops the session when dropped — the one place every path goes through,
/// including `?` and a panic.
struct StopOnDrop<'a, O: SessionOps> {
    ops: &'a mut O,
    session: SessionInfo,
    armed: bool,
}

impl<O: SessionOps> Drop for StopOnDrop<'_, O> {
    fn drop(&mut self) {
        if self.armed
            && let Err(err) = self.ops.stop(&self.session)
        {
            eprintln!(
                "{} could not stop the dev session: {err:#}",
                "warning:".yellow()
            );
        }
    }
}

/// Start, run, stop. `interrupted` is set by the Ctrl-C handler; `lxdev`
/// gets the same signal and cancels its run, then the session is stopped.
fn run_once(
    ops: &mut impl SessionOps,
    keep_session: bool,
    interrupted: &AtomicBool,
) -> Result<i32> {
    let session = ops.start()?;
    let guard = StopOnDrop {
        ops,
        session,
        armed: !keep_session,
    };
    if interrupted.load(Ordering::SeqCst) {
        return Ok(130);
    }
    let session = guard.session.clone();
    let code = guard.ops.run(&session)?;
    Ok(if interrupted.load(Ordering::SeqCst) {
        130
    } else {
        code
    })
}

pub fn execute(options: TestExecuteOptions) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let project = DevProject::resolve(&cwd, options.platform.as_deref())?;
    let project_root = project.root().to_path_buf();
    refuse_live_session(&project_root, session_target(&project, &options))?;
    // The session would refuse the same skew from its background log.
    crate::compat::ensure_project(&project_root, &[])?;
    // Here rather than in the session's background log, where a broker that
    // cannot keep `--name` would only be a warning nobody reads.
    log_store::ensure_current_broker(options.name.is_some())?;

    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let interrupted = interrupted.clone();
        ctrlc::set_handler(move || interrupted.store(true, Ordering::SeqCst))
            .context("failed to install the Ctrl-C handler")?;
    }
    let keep_session = options.keep_session;
    let mut ops = RealSession {
        cwd,
        project,
        options,
    };
    let code = run_once(&mut ops, keep_session, &interrupted)?;
    std::process::exit(code);
}

/// The session target a run of `project` registers as, when it is known
/// before the session starts: `runner`, or the platform asked for.
fn session_target<'a>(project: &DevProject, options: &'a TestExecuteOptions) -> Option<&'a str> {
    match project {
        DevProject::Runner(_) => Some(RUNNER_TARGET),
        DevProject::Host(_) => options.platform.as_deref(),
    }
}

/// A developer's own session is not ours to take over and stop.
fn refuse_live_session(project_root: &Path, platform: Option<&str>) -> Result<()> {
    let live: Vec<SessionInfo> = log_store::list_sessions(project_root)?
        .into_iter()
        .filter(|session| platform.is_none_or(|platform| same_target(&session.target, platform)))
        .collect();
    if live.is_empty() {
        return Ok(());
    }
    let targets = live
        .iter()
        .map(|session| session.target.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "a dev session ({targets}) is already running for this project; `lingxia test` starts \
         and stops its own. Run `lxdev test` against the running one, or stop it first \
         (`lingxia dev stop`)."
    )
}

fn same_target(target: &str, platform: &str) -> bool {
    if target.eq_ignore_ascii_case(platform) {
        return true;
    }
    platform
        .parse::<crate::platform::detector::PlatformType>()
        .is_ok_and(|parsed| target.eq_ignore_ascii_case(parsed.as_str()))
}

struct RealSession {
    cwd: PathBuf,
    project: DevProject,
    options: TestExecuteOptions,
}

impl RealSession {
    fn dev_args(&self) -> Vec<OsString> {
        let options = &self.options;
        let mut args: Vec<OsString> = vec!["dev".into()];
        // `lingxia dev` in an lxapp directory is the Runner by itself; any
        // platform it is given must be the local desktop's, which it checks.
        if let Some(platform) = options
            .platform
            .as_deref()
            .filter(|platform| !is_runner_target(platform))
        {
            args.extend(["--platform".into(), platform.into()]);
        }
        if let Some(name) = &options.name {
            args.extend(["--name".into(), name.into()]);
        }
        if let Some(device) = &options.device {
            args.extend(["--device".into(), device.into()]);
        }
        if let Some(language) = &options.display_language {
            args.extend(["--display-language".into(), language.into()]);
        }
        if options.headless {
            args.push("--headless".into());
        }
        args.extend(options.dev_flags.iter().map(OsString::from));
        args
    }

    fn lxdev_args(&self, session: &SessionInfo) -> Vec<String> {
        let options = &self.options;
        let mut args = vec![
            "--session".to_string(),
            session.session_id.clone(),
            "test".to_string(),
        ];
        if let Some(entry) = &options.entry {
            args.push(entry.clone());
        }
        if let Some(preset) = &options.preset {
            args.extend(["--preset".to_string(), preset.clone()]);
        }
        args.extend(options.lxdev_args.iter().cloned());
        args
    }

    /// The command a Rerun hint starts with: this one's own flags, then
    /// `--`, so the spec's flags reach `lxdev test`.
    fn rerun_prefix(&self) -> String {
        let options = &self.options;
        let mut words = vec!["lingxia".to_string(), "test".to_string()];
        if let Some(platform) = &options.platform {
            words.extend(["--platform".to_string(), platform.clone()]);
        }
        if options.headless {
            words.push("--headless".to_string());
        }
        words.extend(options.dev_flags.iter().cloned());
        words.push("--".to_string());
        words.join(" ")
    }
}

impl SessionOps for RealSession {
    fn start(&mut self) -> Result<SessionInfo> {
        eprintln!(
            "{} starting a dev session in {} ({})",
            "test".cyan(),
            self.project.root().display(),
            match &self.project {
                DevProject::Runner(_) => "the lxapp in the desktop Runner",
                DevProject::Host(_) => "the host app",
            }
        );
        super::dev::start_background_session(
            self.project.root(),
            self.dev_args(),
            SESSION_READY_WITHIN,
            true,
        )?
        .ok_or_else(|| {
            anyhow!(
                "the dev session did not become ready; see `lingxia dev status` and the \
                     background log under .lingxia/dev/background"
            )
        })
    }

    fn run(&mut self, session: &SessionInfo) -> Result<i32> {
        let lxdev = lxdev_binary();
        let status = std::process::Command::new(&lxdev)
            .args(self.lxdev_args(session))
            .current_dir(&self.cwd)
            .env(RERUN_PREFIX_ENV, self.rerun_prefix())
            .status()
            .with_context(|| format!("failed to run {}", lxdev.display()))?;
        Ok(status.code().unwrap_or(1))
    }

    fn stop(&mut self, session: &SessionInfo) -> Result<()> {
        eprintln!(
            "{} stopping the {} dev session",
            "test".cyan(),
            session.target
        );
        super::dev::stop_session_info(session)
    }
}

/// A cold CI build of a host app can take this long before its session is
/// ready; `lingxia dev --background` itself gives up waiting sooner.
const SESSION_READY_WITHIN: std::time::Duration = std::time::Duration::from_secs(1800);

/// Read by `lxdev test` for its Rerun lines.
const RERUN_PREFIX_ENV: &str = "LXDEV_RERUN_PREFIX";

/// `lxdev` next to this `lingxia` (how both are installed and built), else
/// the one on PATH; `LXDEV_BIN` overrides both.
fn lxdev_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("LXDEV_BIN") {
        return PathBuf::from(path);
    }
    let name = if cfg!(windows) { "lxdev.exe" } else { "lxdev" };
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        start_fails: bool,
        code: i32,
        run_fails: bool,
        run_panics: bool,
        interrupt_during_run: Option<Arc<AtomicBool>>,
        events: Vec<String>,
    }

    fn session() -> SessionInfo {
        serde_json::from_value(serde_json::json!({
            "session_id": "abc123", "project_root": "/p", "target": "macos", "pid": 1,
            "ws_url": "ws://127.0.0.1:1", "log_file": ""
        }))
        .unwrap()
    }

    impl SessionOps for Fake {
        fn start(&mut self) -> Result<SessionInfo> {
            self.events.push("start".into());
            if self.start_fails {
                bail!("no session");
            }
            Ok(session())
        }
        fn run(&mut self, _: &SessionInfo) -> Result<i32> {
            self.events.push("run".into());
            if let Some(flag) = &self.interrupt_during_run {
                flag.store(true, Ordering::SeqCst);
            }
            if self.run_panics {
                panic!("lxdev blew up");
            }
            if self.run_fails {
                bail!("lxdev not found");
            }
            Ok(self.code)
        }
        fn stop(&mut self, _: &SessionInfo) -> Result<()> {
            self.events.push("stop".into());
            Ok(())
        }
    }

    #[test]
    fn a_passing_or_failing_run_stops_the_session_and_keeps_its_exit_code() {
        for code in [0, 1, 2] {
            let mut fake = Fake {
                code,
                ..Default::default()
            };
            let result = run_once(&mut fake, false, &AtomicBool::new(false)).unwrap();
            assert_eq!(result, code);
            assert_eq!(fake.events, ["start", "run", "stop"]);
        }
    }

    #[test]
    fn an_interrupted_run_stops_the_session_and_exits_130() {
        let flag = Arc::new(AtomicBool::new(false));
        let mut fake = Fake {
            code: 1,
            interrupt_during_run: Some(flag.clone()),
            ..Default::default()
        };
        assert_eq!(run_once(&mut fake, false, &flag).unwrap(), 130);
        assert_eq!(fake.events, ["start", "run", "stop"]);
        // Interrupted while it started: the run is skipped, the session still
        // stopped.
        let mut fake = Fake::default();
        assert_eq!(
            run_once(&mut fake, false, &AtomicBool::new(true)).unwrap(),
            130
        );
        assert_eq!(fake.events, ["start", "stop"]);
    }

    #[test]
    fn an_error_or_a_panic_still_stops_the_session() {
        let mut fake = Fake {
            run_fails: true,
            ..Default::default()
        };
        assert!(run_once(&mut fake, false, &AtomicBool::new(false)).is_err());
        assert_eq!(fake.events, ["start", "run", "stop"]);

        let mut fake = Fake {
            run_panics: true,
            ..Default::default()
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_once(&mut fake, false, &AtomicBool::new(false))
        }));
        assert!(outcome.is_err());
        assert_eq!(fake.events, ["start", "run", "stop"]);

        // Nothing started, nothing to stop.
        let mut fake = Fake {
            start_fails: true,
            ..Default::default()
        };
        assert!(run_once(&mut fake, false, &AtomicBool::new(false)).is_err());
        assert_eq!(fake.events, ["start"]);
    }

    #[test]
    fn keep_session_leaves_it_running() {
        let mut fake = Fake {
            code: 1,
            ..Default::default()
        };
        assert_eq!(
            run_once(&mut fake, true, &AtomicBool::new(false)).unwrap(),
            1
        );
        assert_eq!(fake.events, ["start", "run"]);
    }

    #[test]
    fn the_session_and_the_run_get_their_own_flags() {
        let real = RealSession {
            cwd: PathBuf::from("/p/lxapp"),
            project: DevProject::Host(PathBuf::from("/p")),
            options: TestExecuteOptions {
                entry: Some("tests/".into()),
                preset: Some("ci".into()),
                headless: false,
                keep_session: false,
                platform: Some("macos".into()),
                name: Some("ci-run".into()),
                device: None,
                display_language: None,
                dev_flags: vec!["--skip-native".into()],
                lxdev_args: vec!["--grep".into(), "home".into()],
            },
        };
        assert_eq!(
            real.dev_args(),
            [
                "dev",
                "--platform",
                "macos",
                "--name",
                "ci-run",
                "--skip-native"
            ]
            .map(OsString::from)
        );
        assert_eq!(
            real.lxdev_args(&session()),
            [
                "--session",
                "abc123",
                "test",
                "tests/",
                "--preset",
                "ci",
                "--grep",
                "home"
            ]
        );
        assert_eq!(
            real.rerun_prefix(),
            "lingxia test --platform macos --skip-native --"
        );
    }

    /// `<host>/lingxia.yaml` with the lxapp in `<host>/lxapp`.
    fn host_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("lingxia.yaml"), "app: {}\n").unwrap();
        std::fs::create_dir_all(dir.path().join("lxapp/tests")).unwrap();
        std::fs::create_dir_all(dir.path().join("android")).unwrap();
        std::fs::write(dir.path().join("lxapp/lxapp.json"), "{}").unwrap();
        dir
    }

    #[test]
    fn it_runs_what_lingxia_dev_runs_in_the_nearest_project() {
        let host = host_project();
        let root = host.path();
        let lxapp = root.join("lxapp");
        let resolve = |dir: &Path, target: Option<&str>| DevProject::resolve(dir, target).unwrap();
        // `lingxia dev` in the lxapp directory starts the Runner, so does this.
        assert_eq!(
            DevProject::at(&lxapp),
            Some(DevProject::Runner(lxapp.clone()))
        );
        assert_eq!(resolve(&lxapp, None), DevProject::Runner(lxapp.clone()));
        assert_eq!(
            resolve(&lxapp.join("tests"), None),
            DevProject::Runner(lxapp.clone())
        );
        assert_eq!(resolve(root, None), DevProject::Host(root.to_path_buf()));
        assert_eq!(
            resolve(&root.join("android"), None),
            DevProject::Host(root.to_path_buf())
        );

        let standalone = tempfile::tempdir().unwrap();
        std::fs::write(standalone.path().join("lxapp.json"), "{}").unwrap();
        assert_eq!(
            resolve(standalone.path(), None),
            DevProject::Runner(standalone.path().to_path_buf())
        );
        let nothing = tempfile::tempdir().unwrap();
        assert!(DevProject::resolve(nothing.path(), None).is_err());
    }

    #[test]
    fn a_target_picks_the_runner_or_the_host_app() {
        let host = host_project();
        let root = host.path();
        let lxapp = root.join("lxapp");
        let resolve = |dir: &Path, target: &str| DevProject::resolve(dir, Some(target));
        for runner in ["runner", "Runner"] {
            assert_eq!(
                resolve(&lxapp.join("tests"), runner).unwrap(),
                DevProject::Runner(lxapp.clone())
            );
        }
        // A platform means the host app, from anywhere inside it.
        assert_eq!(
            resolve(&lxapp, "ios").unwrap(),
            DevProject::Host(root.to_path_buf())
        );
        // The host project root is not an lxapp.
        let err = resolve(root, "runner").unwrap_err().to_string();
        assert!(err.contains("no lxapp directory"), "{err}");
        // A standalone lxapp has only its Runner.
        let standalone = tempfile::tempdir().unwrap();
        std::fs::write(standalone.path().join("lxapp.json"), "{}").unwrap();
        assert_eq!(
            resolve(standalone.path(), "macos").unwrap(),
            DevProject::Runner(standalone.path().to_path_buf())
        );
    }

    #[test]
    fn a_runner_session_gets_no_platform_flag() {
        let real = RealSession {
            cwd: PathBuf::from("/p/lxapp"),
            project: DevProject::Runner(PathBuf::from("/p/lxapp")),
            options: TestExecuteOptions {
                entry: None,
                preset: None,
                headless: false,
                keep_session: false,
                platform: Some("runner".into()),
                name: Some("ui".into()),
                device: None,
                display_language: None,
                dev_flags: vec![],
                lxdev_args: vec![],
            },
        };
        assert_eq!(real.dev_args(), ["dev", "--name", "ui"].map(OsString::from));
        assert_eq!(real.rerun_prefix(), "lingxia test --platform runner --");
        assert_eq!(session_target(&real.project, &real.options), Some("runner"));
        assert!(same_target("runner", "runner"));
        assert!(same_target("macos", "mac"));
        assert!(!same_target("android", "macos"));
    }
}
