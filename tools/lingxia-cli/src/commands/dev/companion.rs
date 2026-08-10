use anyhow::{Context, Result, anyhow, bail};
use lingxia_control_protocol::{
    ControlRequest, ControlResponse,
    dev_session::{
        DEV_SESSION_PROTOCOL_VERSION, DevSessionEvent, DevSessionMessage, DevSessionPrepareResult,
        DevSessionRole, capabilities,
    },
    methods,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{ProcessesToUpdate, System};

use super::server::SessionLogWriter;

const CONFIG_FILE: &str = "dev-companion.json";
const PREPARE_REQUEST_ID: &str = "session-prepare";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_EVENT_BATCH: usize = 512;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompanionConfig {
    run: Vec<String>,
}

#[derive(Debug)]
pub(super) struct DevCompanion {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Option<ChildStdin>,
    stopping: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    stdout_reader: Option<thread::JoinHandle<()>>,
    event_dispatcher: Option<thread::JoinHandle<()>>,
    monitor: Option<thread::JoinHandle<()>>,
    previous_env: Vec<(String, Option<OsString>)>,
}

impl DevCompanion {
    pub(super) fn start(
        project_root: &Path,
        stop_requested: Arc<AtomicBool>,
        writer: Arc<SessionLogWriter>,
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
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to open development companion stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to read development companion stdout"))?;

        let (message_tx, message_rx) = mpsc::channel();
        let stdout_reader = thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match stdout.read_line(&mut line) {
                    Ok(0) => return,
                    Ok(_) if line.len() > MAX_FRAME_BYTES => {
                        let _ = message_tx.send(Err(anyhow!(
                            "Development companion protocol frame exceeds {MAX_FRAME_BYTES} bytes"
                        )));
                        return;
                    }
                    Ok(_) => {
                        let parsed = serde_json::from_str::<DevSessionMessage>(line.trim())
                            .context("Development companion sent an invalid protocol frame");
                        if message_tx.send(parsed).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ =
                            message_tx
                                .send(Err(error).context(
                                    "Failed to read development companion protocol frame",
                                ));
                        return;
                    }
                }
            }
        });

        let hello = match recv_message(&message_rx, &mut child, &stop_requested) {
            Ok(message) => message,
            Err(error) => {
                stop_child_tree(&mut child);
                return Err(error);
            }
        };
        let DevSessionMessage::Hello {
            version,
            role,
            capabilities: peer_capabilities,
        } = hello
        else {
            stop_child_tree(&mut child);
            bail!("Development companion must begin with a hello frame");
        };
        if version != DEV_SESSION_PROTOCOL_VERSION {
            stop_child_tree(&mut child);
            bail!(
                "Unsupported development companion protocol version {version}; expected {DEV_SESSION_PROTOCOL_VERSION}"
            );
        }
        if role != DevSessionRole::Companion {
            stop_child_tree(&mut child);
            bail!("Development companion hello must use the companion role");
        }
        if !peer_capabilities
            .iter()
            .any(|capability| capability == capabilities::REQUESTS)
        {
            stop_child_tree(&mut child);
            bail!("Development companion does not support session requests");
        }

        if let Err(error) = write_message(
            &mut stdin,
            &DevSessionMessage::Request(ControlRequest {
                id: PREPARE_REQUEST_ID.to_string(),
                method: methods::session::PREPARE.to_string(),
                params: None,
            }),
        ) {
            stop_child_tree(&mut child);
            return Err(error);
        }

        let response = match recv_message(&message_rx, &mut child, &stop_requested) {
            Ok(message) => message,
            Err(error) => {
                stop_child_tree(&mut child);
                return Err(error);
            }
        };
        let DevSessionMessage::Response(ControlResponse { id, result, error }) = response else {
            stop_child_tree(&mut child);
            bail!("Development companion must respond to session.prepare before sending events");
        };
        if id != PREPARE_REQUEST_ID {
            stop_child_tree(&mut child);
            bail!("Development companion returned an unexpected response id `{id}`");
        }
        if let Some(error) = error {
            stop_child_tree(&mut child);
            bail!(
                "Development companion preparation failed: {}",
                error.message
            );
        }
        let prepare: DevSessionPrepareResult = match result
            .ok_or_else(|| anyhow!("Development companion returned no session.prepare result"))
            .and_then(|result| {
                serde_json::from_value(result)
                    .context("Development companion returned an invalid session.prepare result")
            }) {
            Ok(prepare) => prepare,
            Err(error) => {
                stop_child_tree(&mut child);
                return Err(error);
            }
        };
        if !prepare.active {
            drop(stdin);
            stop_child_tree(&mut child);
            let _ = stdout_reader.join();
            return Ok(None);
        }
        if let Err(error) = validate_runtime_env(&prepare.runtime_env) {
            stop_child_tree(&mut child);
            return Err(error);
        }
        let previous_env = apply_runtime_env(&prepare.runtime_env);

        let stopping = Arc::new(AtomicBool::new(false));
        let failure = Arc::new(Mutex::new(None));
        let dispatcher_stopping = stopping.clone();
        let dispatcher_failure = failure.clone();
        let dispatcher_stop_requested = stop_requested.clone();
        let event_dispatcher = thread::spawn(move || {
            while let Ok(message) = message_rx.recv() {
                if dispatcher_stopping.load(Ordering::Acquire) {
                    return;
                }
                let result = message.and_then(|message| match message {
                    DevSessionMessage::EventBatch { events } => {
                        validate_events(&events)?;
                        writer.append_events(&events)
                    }
                    _ => Err(anyhow!(
                        "Development companion sent a non-event frame after preparation"
                    )),
                });
                if let Err(error) = result {
                    if !dispatcher_stopping.load(Ordering::Acquire) {
                        set_failure(&dispatcher_failure, format!("{error:#}"));
                        dispatcher_stop_requested.store(true, Ordering::Release);
                    }
                    return;
                }
            }
            if !dispatcher_stopping.load(Ordering::Acquire) {
                set_failure(
                    &dispatcher_failure,
                    "Development companion closed its protocol stream".to_string(),
                );
                dispatcher_stop_requested.store(true, Ordering::Release);
            }
        });

        println!("  ✓ Development integration ready");
        let child = Arc::new(Mutex::new(Some(child)));
        let monitor = spawn_monitor(
            child.clone(),
            stopping.clone(),
            failure.clone(),
            stop_requested,
        );
        Ok(Some(Self {
            child,
            stdin: Some(stdin),
            stopping,
            failure,
            stdout_reader: Some(stdout_reader),
            event_dispatcher: Some(event_dispatcher),
            monitor: Some(monitor),
            previous_env,
        }))
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
            stop_child_tree(child);
        }
        if let Some(reader) = self.stdout_reader.take() {
            let _ = reader.join();
        }
        if let Some(dispatcher) = self.event_dispatcher.take() {
            let _ = dispatcher.join();
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

fn recv_message(
    receiver: &mpsc::Receiver<Result<DevSessionMessage>>,
    child: &mut Child,
    stop_requested: &AtomicBool,
) -> Result<DevSessionMessage> {
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    loop {
        if stop_requested.load(Ordering::Acquire) {
            bail!("Development companion startup cancelled");
        }
        if let Some(status) = child.try_wait()? {
            bail!("Development companion exited during preparation: {status}");
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("Timed out waiting for the development companion");
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
            Ok(message) => return message,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                bail!("Development companion closed its protocol stream during preparation")
            }
        }
    }
}

fn write_message(stdin: &mut ChildStdin, message: &DevSessionMessage) -> Result<()> {
    serde_json::to_writer(&mut *stdin, message)
        .context("Failed to encode development companion protocol frame")?;
    stdin
        .write_all(b"\n")
        .context("Failed to write development companion protocol frame")?;
    stdin
        .flush()
        .context("Failed to flush development companion protocol frame")
}

fn validate_events(events: &[DevSessionEvent]) -> Result<()> {
    if events.len() > MAX_EVENT_BATCH {
        bail!("Development companion event batch exceeds {MAX_EVENT_BATCH} entries");
    }
    for event in events {
        if event.origin.trim().is_empty() {
            bail!("Development companion event origin cannot be empty");
        }
        if event.kind.trim().is_empty() {
            bail!("Development companion event kind cannot be empty");
        }
        if event.kind == lingxia_control_protocol::dev_session::event_kinds::LOG {
            event
                .as_log()
                .context("Development companion sent an invalid log event")?;
        }
    }
    Ok(())
}

fn set_failure(failure: &Mutex<Option<String>>, message: String) {
    if let Ok(mut failure) = failure.lock()
        && failure.is_none()
    {
        *failure = Some(message);
    }
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
    if run.iter().any(|part| part.contains('\0')) {
        bail!("{}: `run` cannot contain NUL bytes", path.display());
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
        // The supervised participant starts before runtime threads or child
        // processes and restoration happens after all of them have stopped.
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
                    set_failure(
                        &failure,
                        format!(
                            "Development companion stopped while the dev session was active: {status}"
                        ),
                    );
                    stop_requested.store(true, Ordering::Release);
                }
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    })
}

fn stop_child_tree(child: &mut Child) {
    let root_pid = sysinfo::Pid::from_u32(child.id());
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let mut tree = HashSet::from([root_pid]);
    loop {
        let before = tree.len();
        for (pid, process) in system.processes() {
            if process
                .parent()
                .is_some_and(|parent| tree.contains(&parent))
            {
                tree.insert(*pid);
            }
        }
        if tree.len() == before {
            break;
        }
    }

    let deadline = Instant::now() + STOP_TIMEOUT;
    let mut child_exited = false;
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            child_exited = true;
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    system.refresh_processes(ProcessesToUpdate::All, true);
    for pid in tree.iter().copied().filter(|pid| *pid != root_pid) {
        if let Some(process) = system.process(pid) {
            process.kill();
        }
    }
    if !child_exited {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lingxia_control_protocol::dev_session::{DevSessionLog, DevSessionLogLevel};

    #[test]
    fn config_is_a_single_argv() {
        let config: CompanionConfig =
            serde_json::from_str(r#"{"run":["example","companion"]}"#).unwrap();
        assert_eq!(config.run, ["example", "companion"]);
    }

    #[test]
    fn config_rejects_extra_surface() {
        assert!(
            serde_json::from_str::<CompanionConfig>(r#"{"run":["example"],"id":"integration"}"#)
                .is_err()
        );
    }

    #[test]
    fn prepare_result_has_only_generic_runtime_environment() {
        let result: DevSessionPrepareResult = serde_json::from_str(
            r#"{"active":true,"runtime_env":{"EXAMPLE_ENDPOINT":"http://127.0.0.1:1"}}"#,
        )
        .unwrap();
        assert!(result.active);
        assert_eq!(result.runtime_env["EXAMPLE_ENDPOINT"], "http://127.0.0.1:1");
    }

    #[test]
    fn event_validation_accepts_dynamic_log_origins() {
        let event = DevSessionEvent::log(
            123,
            "service.api",
            DevSessionLog {
                level: DevSessionLogLevel::Info,
                appid: None,
                path: None,
                target: Some("request".to_string()),
                message: "ready".to_string(),
                attributes: BTreeMap::from([(
                    "request.id".to_string(),
                    serde_json::json!("req-1"),
                )]),
            },
        )
        .unwrap();
        validate_events(&[event]).unwrap();
    }

    #[test]
    fn event_validation_rejects_empty_origin() {
        let event = DevSessionEvent {
            timestamp_ms: 123,
            origin: String::new(),
            kind: "log".to_string(),
            data: serde_json::json!({"level":"info","message":"hello"}),
        };
        assert!(validate_events(&[event]).is_err());
    }

    #[test]
    fn config_is_project_local() {
        let root = Path::new("/tmp/example");
        assert_eq!(
            config_path(root),
            root.join(".lingxia").join("dev-companion.json")
        );
    }

    #[cfg(unix)]
    #[test]
    fn supervised_protocol_appends_events_to_the_session() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join(".lingxia");
        fs::create_dir_all(&config_dir).unwrap();
        let script = concat!(
            "printf '%s\\n' '{\"type\":\"hello\",\"version\":2,\"role\":\"companion\",",
            "\"capabilities\":[\"requests\",\"events.log\"]}'; ",
            "IFS= read -r request; ",
            "printf '%s\\n' '{\"type\":\"response\",\"id\":\"session-prepare\",",
            "\"result\":{\"active\":true,\"runtime_env\":{}}}'; ",
            "printf '%s\\n' '{\"type\":\"event_batch\",\"events\":[{",
            "\"timestamp_ms\":123,\"origin\":\"service.api\",\"kind\":\"log\",",
            "\"data\":{\"level\":\"info\",\"message\":\"hello\"}}]}'; ",
            "while IFS= read -r line; do :; done"
        );
        fs::write(
            config_dir.join(CONFIG_FILE),
            serde_json::to_vec(&serde_json::json!({"run": ["sh", "-c", script]})).unwrap(),
        )
        .unwrap();

        let session = super::super::log_store::create_session(root.path()).unwrap();
        let writer = Arc::new(SessionLogWriter::new(&session).unwrap());
        let stop_requested = Arc::new(AtomicBool::new(false));
        let companion = DevCompanion::start(root.path(), stop_requested, writer)
            .unwrap()
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        let line = loop {
            let line = fs::read_to_string(&session.log_file).unwrap_or_default();
            if line.ends_with('\n') {
                break line;
            }
            assert!(Instant::now() < deadline, "session event was not persisted");
            thread::sleep(Duration::from_millis(10));
        };
        let event: DevSessionEvent = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(event.origin, "service.api");
        assert_eq!(event.as_log().unwrap().unwrap().message, "hello");
        drop(companion);
    }
}
