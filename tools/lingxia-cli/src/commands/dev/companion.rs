use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

const CONFIG_FILE: &str = "dev-companion.json";
const PROTOCOL_VERSION: u32 = 1;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompanionConfig {
    run: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum CompanionMessage {
    Inactive,
    Ready {
        #[serde(rename = "protocolVersion")]
        protocol_version: u32,
        #[serde(default, rename = "runtimeEnv")]
        runtime_env: BTreeMap<String, String>,
    },
}

pub(super) struct DevCompanion {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Option<ChildStdin>,
    stopping: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    monitor: Option<thread::JoinHandle<()>>,
    previous_env: Vec<(String, Option<OsString>)>,
}

impl DevCompanion {
    pub(super) fn start(
        project_root: &Path,
        stop_requested: Arc<AtomicBool>,
    ) -> Result<Option<Self>> {
        let config_path = config_path(project_root);
        if !config_path.exists() {
            return Ok(None);
        }
        let config = load_config(&config_path)?;
        validate_run(&config.run, &config_path)?;

        let mut command = Command::new(&config.run[0]);
        command
            .args(&config.run[1..])
            .current_dir(project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        let mut child = command.spawn().with_context(|| {
            format!(
                "Failed to start the development companion configured in {}",
                config_path.display()
            )
        })?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to read development companion handshake"))?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
            let _ = sender.send(result);
        });

        let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
        let line = loop {
            if stop_requested.load(Ordering::Acquire) {
                stop_child(&mut child);
                bail!("Development companion startup cancelled");
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                stop_child(&mut child);
                bail!("Timed out waiting for the development companion to become ready");
            }
            match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
                Ok(Ok(line)) => break line,
                Ok(Err(error)) => {
                    stop_child(&mut child);
                    return Err(error).context("Failed to read development companion handshake");
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    stop_child(&mut child);
                    bail!("Development companion closed before sending its handshake");
                }
            }
        };
        if line.trim().is_empty() {
            let status = child.try_wait().ok().flatten();
            stop_child(&mut child);
            match status {
                Some(status) => {
                    bail!("Development companion exited before sending its handshake: {status}")
                }
                None => bail!("Development companion sent an empty handshake"),
            }
        }
        let message: CompanionMessage = match serde_json::from_str(line.trim()) {
            Ok(message) => message,
            Err(error) => {
                drop(stdin);
                stop_child(&mut child);
                return Err(error).context("Development companion sent an invalid handshake");
            }
        };
        match message {
            CompanionMessage::Inactive => {
                drop(stdin);
                stop_child(&mut child);
                Ok(None)
            }
            CompanionMessage::Ready {
                protocol_version,
                runtime_env,
            } => {
                if protocol_version != PROTOCOL_VERSION {
                    drop(stdin);
                    stop_child(&mut child);
                    bail!(
                        "Unsupported development companion protocol version {protocol_version}; expected {PROTOCOL_VERSION}"
                    );
                }
                if let Err(error) = validate_runtime_env(&runtime_env) {
                    drop(stdin);
                    stop_child(&mut child);
                    return Err(error);
                }
                let previous_env = apply_runtime_env(&runtime_env);
                println!("  ✓ Development companion ready");

                let child = Arc::new(Mutex::new(Some(child)));
                let stopping = Arc::new(AtomicBool::new(false));
                let failure = Arc::new(Mutex::new(None));
                let monitor = spawn_monitor(
                    child.clone(),
                    stopping.clone(),
                    failure.clone(),
                    stop_requested,
                );
                Ok(Some(Self {
                    child,
                    stdin,
                    stopping,
                    failure,
                    monitor: Some(monitor),
                    previous_env,
                }))
            }
        }
    }

    fn failure(&self) -> Option<String> {
        self.failure.lock().ok().and_then(|failure| failure.clone())
    }
}

impl Drop for DevCompanion {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        self.stdin.take();
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            stop_child(child);
        }
        if let Some(monitor) = self.monitor.take() {
            let _ = monitor.join();
        }
        restore_runtime_env(&self.previous_env);
    }
}

pub(super) fn finish(result: Result<()>, companion: Option<&DevCompanion>) -> Result<()> {
    if let Some(message) = companion.and_then(DevCompanion::failure) {
        return Err(anyhow!(message));
    }
    result
}

fn config_path(project_root: &Path) -> PathBuf {
    project_root.join(".lingxia").join(CONFIG_FILE)
}

fn load_config(path: &Path) -> Result<CompanionConfig> {
    let bytes = fs::read(path).with_context(|| {
        format!(
            "Failed to read development companion config {}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("Invalid development companion config {}", path.display()))
}

fn validate_run(run: &[String], path: &Path) -> Result<()> {
    if run.is_empty() || run.iter().any(|part| part.is_empty()) {
        bail!(
            "{}: `run` must be a non-empty argv array without empty values",
            path.display()
        );
    }
    Ok(())
}

fn validate_runtime_env(values: &BTreeMap<String, String>) -> Result<()> {
    for (name, value) in values {
        let mut chars = name.chars();
        let valid_name = chars
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
        if !valid_name {
            bail!("Development companion returned invalid environment variable name `{name}`");
        }
        if value.contains('\0') {
            bail!("Development companion returned an invalid value for `{name}`");
        }
    }
    Ok(())
}

fn apply_runtime_env(values: &BTreeMap<String, String>) -> Vec<(String, Option<OsString>)> {
    let previous = values
        .keys()
        .map(|name| (name.clone(), std::env::var_os(name)))
        .collect::<Vec<_>>();
    for (name, value) in values {
        // No LingXia worker threads exist yet, and restoration happens only after
        // every dev child and server has stopped.
        unsafe { std::env::set_var(name, value) };
    }
    previous
}

fn restore_runtime_env(previous: &[(String, Option<OsString>)]) {
    for (name, value) in previous {
        // Dev execution has joined its children and server threads before this
        // guard is dropped, so process-environment mutation is isolated here.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

fn spawn_monitor(
    child: Arc<Mutex<Option<Child>>>,
    stopping: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    stop_requested: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
            if stopping.load(Ordering::Acquire) {
                return;
            }
            let status = child.lock().ok().and_then(|mut child| {
                child
                    .as_mut()
                    .and_then(|child| child.try_wait().ok().flatten())
            });
            if let Some(status) = status {
                if !stopping.load(Ordering::Acquire) {
                    if let Ok(mut failure) = failure.lock() {
                        *failure = Some(format!(
                            "Development companion stopped while the dev session was active: {status}"
                        ));
                    }
                    stop_requested.store(true, Ordering::Release);
                }
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    })
}

fn stop_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_is_a_single_argv() {
        let config: CompanionConfig =
            serde_json::from_str(r#"{"run":["fusheng","companion"]}"#).unwrap();
        assert_eq!(config.run, ["fusheng", "companion"]);
    }

    #[test]
    fn config_rejects_extra_surface() {
        assert!(
            serde_json::from_str::<CompanionConfig>(r#"{"run":["fusheng"],"id":"fusheng"}"#,)
                .is_err()
        );
    }

    #[test]
    fn handshake_has_only_generic_runtime_environment() {
        let message: CompanionMessage = serde_json::from_str(
            r#"{"type":"ready","protocolVersion":1,"runtimeEnv":{"EXAMPLE_ENDPOINT":"http://127.0.0.1:1"}}"#,
        )
        .unwrap();
        let CompanionMessage::Ready {
            protocol_version,
            runtime_env,
        } = message
        else {
            panic!("expected ready message");
        };
        assert_eq!(protocol_version, PROTOCOL_VERSION);
        assert_eq!(runtime_env["EXAMPLE_ENDPOINT"], "http://127.0.0.1:1");
    }

    #[test]
    fn config_is_project_local() {
        let root = Path::new("/tmp/example");
        assert_eq!(
            config_path(root),
            root.join(".lingxia").join("dev-companion.json")
        );
    }
}
