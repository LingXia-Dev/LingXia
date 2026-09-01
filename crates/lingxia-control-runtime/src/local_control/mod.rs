//! The product's local control socket.
//!
//! This is not the development websocket. That one exists so `lingxia dev` can
//! drive an app across a network to a phone; this one exists so a *shipped*
//! product can offer a command line that drives its declared product surface,
//! reached without a dev session, over an IPC that never
//! leaves the machine.
//!
//! Each platform gets its native mechanism rather than one forced everywhere.
//! Windows has supported `AF_UNIX` since 1803, but a named pipe carries a real
//! security descriptor and can name the process on the other end, and neither
//! is true of `AF_UNIX` there — and the two things this endpoint must get
//! right are exactly "only this user" and "who is asking". A pipe also cannot
//! be left behind by a crash the way a socket file can.
//!
//! The wire is newline-delimited JSON using the transport-neutral
//! [`ControlRequest`] and [`ControlResponse`] contract. Requests are small and
//! strictly request/response, so nothing here needs framing beyond a line.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use lingxia_control_protocol::{ControlMessage, ControlRequest, ControlResponse};

mod legacy;

#[cfg_attr(unix, path = "unix.rs")]
#[cfg_attr(windows, path = "windows.rs")]
mod platform;

pub use platform::endpoint_name;

/// LingXia-owned runtime directory for this product, once [`install`] has run.
static CONTROL_DIR: OnceLock<PathBuf> = OnceLock::new();
static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

struct Running {
    endpoint: String,
    listening: Arc<AtomicBool>,
    /// Joined before this slot is released. The unix socket's pathname is
    /// shared between runs, and a listener still unwinding would otherwise
    /// unlink the socket a rebind had just created.
    accepting: Option<std::thread::JoinHandle<()>>,
}

impl Running {
    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::SeqCst)
            && self
                .accepting
                .as_ref()
                .is_some_and(|accepting| !accepting.is_finished())
    }
}

fn listener_is_live(running: Option<&Running>) -> bool {
    running.is_some_and(Running::is_listening)
}

struct MarkStoppedOnDrop(Arc<AtomicBool>);

impl Drop for MarkStoppedOnDrop {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Bumped every time the endpoint is switched off. A connection accepted under
/// an older epoch is closed before its next request can be answered.
static EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Install the product's local control service in its desired startup state.
///
/// A host calls this from `start_services` when it ships the capability.
/// Product-owned UI may later call [`set_enabled`] to apply its own access
/// preference without a restart. LingXia neither chooses nor persists that
/// preference and exposes no command that can grant its own access.
pub fn install(enabled: bool) -> std::io::Result<()> {
    let app_state_dir =
        lingxia::app::state_dir().map_err(|error| std::io::Error::other(error.to_string()))?;
    let app_data_dir = app_state_dir
        .parent()
        .ok_or_else(|| std::io::Error::other("app state directory has no parent"))?;
    let control_dir = lingxia_control_protocol::local_control::directory(app_data_dir);
    let _ = CONTROL_DIR.set(control_dir);
    if let Err(error) = legacy::cleanup(&app_state_dir) {
        log::warn!("legacy product launcher cleanup failed: {error}");
    }
    set_enabled(enabled)
}

/// Start or stop listening. Persisting the choice is the caller's job — the
/// settings surface owns that, and this stays callable from a test.
pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    let control_dir = CONTROL_DIR
        .get()
        .ok_or_else(|| std::io::Error::other("control socket is not installed"))?;
    let mut running = RUNNING.lock().unwrap_or_else(|error| error.into_inner());
    match enabled {
        true => {
            if listener_is_live(running.as_ref()) {
                return Ok(());
            }
            // A listener can give up after repeated platform errors. Reap its
            // thread and invalidate connections from that epoch before
            // replacing it, instead of preserving a dead RUNNING entry.
            if let Some(mut stopped) = running.take() {
                let _ = stop_accepting(&mut stopped);
            }
            let mut started = start(control_dir, crate::dispatch_line)?;
            if let Err(error) = publish_endpoint(control_dir, &started.endpoint) {
                let _ = stop_accepting(&mut started);
                clear_published_endpoint(control_dir);
                return Err(error);
            }
            *running = Some(started);
            Ok(())
        }
        false => {
            #[cfg(feature = "computer-use")]
            crate::desktop::end_session();
            if let Some(mut existing) = running.take() {
                let _ = stop_accepting(&mut existing);
            }
            clear_published_endpoint(control_dir);
            platform::clear_stale(control_dir);
            log::info!("control socket switched off");
            Ok(())
        }
    }
}

fn publish_endpoint(control_dir: &Path, endpoint: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(control_dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(control_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let destination = lingxia_control_protocol::local_control::endpoint_file(control_dir);
    write_endpoint_atomically(&destination, endpoint.as_bytes())
}

fn clear_published_endpoint(control_dir: &Path) {
    let path = lingxia_control_protocol::local_control::endpoint_file(control_dir);
    let _ = std::fs::remove_file(path);
}

fn create_endpoint_temporary(path: &Path) -> std::io::Result<(PathBuf, std::fs::File)> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
    for _ in 0..32 {
        let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("tmp-{}-{sequence}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!(
            "cannot reserve a temporary endpoint file for {}",
            path.display()
        ),
    ))
}

#[cfg(not(windows))]
fn write_endpoint_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let (temporary, mut file) = create_endpoint_temporary(path)?;
    let result = file.write_all(contents).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

#[cfg(windows)]
fn write_endpoint_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::HSTRING;

    let (temporary, mut file) = create_endpoint_temporary(path)?;
    let result = file.write_all(contents).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = result {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
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

fn stop_accepting(running: &mut Running) -> bool {
    running.listening.store(false, Ordering::SeqCst);
    EPOCH.fetch_add(1, Ordering::SeqCst);
    let Some(accepting) = running.accepting.take() else {
        return true;
    };
    // A Windows accept creates one pipe instance at a time. Keep waking until
    // the loop observes the flag so a toggle never blocks between instances.
    for _ in 0..100 {
        if accepting.is_finished() {
            break;
        }
        platform::poke(&running.endpoint);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !accepting.is_finished() {
        log::warn!("control accept thread took longer than two seconds to stop");
    }
    if accepting.join().is_err() {
        log::warn!("control accept thread panicked on the way out");
        false
    } else {
        true
    }
}

/// Whether product-owned agent access is enabled right now.
pub fn is_enabled() -> bool {
    let running = RUNNING.lock().unwrap_or_else(|error| error.into_inner());
    listener_is_live(running.as_ref())
}

/// Backward-compatible name for [`is_enabled`].
pub fn is_listening() -> bool {
    is_enabled()
}

fn start(control_dir: &Path, handle: Handler) -> std::io::Result<Running> {
    let epoch = EPOCH.load(Ordering::SeqCst);
    let listener = platform::Listener::bind(control_dir, epoch)?;
    let endpoint = listener.name();
    log::info!("control socket listening on {endpoint}");
    let listening = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&listening);
    let accepting = std::thread::Builder::new()
        .name("lingxia-control".to_string())
        .spawn(move || {
            // Every exit path, including a panic, must make status truthful
            // and let a later enable replace this listener.
            let _mark_stopped = MarkStoppedOnDrop(Arc::clone(&flag));
            let mut consecutive_failures = 0u32;
            while flag.load(Ordering::SeqCst) {
                match listener.accept(&flag) {
                    Ok(stream) => {
                        consecutive_failures = 0;
                        if !flag.load(Ordering::SeqCst) {
                            break;
                        }
                        std::thread::Builder::new()
                            .name("lingxia-control-conn".to_string())
                            .spawn(move || serve_connection(stream, handle, epoch))
                            .ok();
                    }
                    Err(error) => {
                        if !flag.load(Ordering::SeqCst) {
                            break;
                        }
                        consecutive_failures += 1;
                        log::warn!("control socket accept failed: {error}");
                        // A single failed accept is usually transient, but a
                        // listener that is really gone fails every time, and
                        // spinning on it would burn a core in silence.
                        if consecutive_failures >= 16 {
                            log::error!("control socket giving up after repeated accept failures");
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }
            }
            drop(listener);
        })?;
    Ok(Running {
        endpoint,
        listening,
        accepting: Some(accepting),
    })
}

/// Turns one request line into one reply line. A function pointer rather than
/// a direct call so the accept loop can be exercised without [`crate::dispatch`],
/// which drags the whole platform runtime into the link.
type Handler = fn(&str) -> ControlResponse;

/// One client, one request at a time, until it hangs up or the endpoint is
/// switched off underneath it.
fn serve_connection(stream: platform::Stream, handle: Handler, epoch: u64) {
    let mut writer = match platform::split_writer(&stream) {
        Ok(writer) => writer,
        Err(error) => {
            log::warn!("control connection unusable: {error}");
            return;
        }
    };
    for line in BufReader::new(stream).lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                log::debug!("control connection closed: {error}");
                return;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        // Checked per request rather than once: a client that was connected
        // when the user switched control off must not keep driving the product
        // for as long as it holds the socket open.
        if EPOCH.load(Ordering::SeqCst) != epoch {
            return;
        }
        let reply = handle(&line);
        let Ok(mut encoded) = serde_json::to_vec(&ControlMessage::Response(reply)) else {
            continue;
        };
        encoded.push(b'\n');
        if writer.write_all(&encoded).is_err() || writer.flush().is_err() {
            return;
        }
    }
}

/// The framing half, kept apart from the handler chain so it can be tested
/// without one — reaching `dispatch` pulls in the whole platform runtime.
pub(crate) fn reply_with(
    line: &str,
    dispatch: impl FnOnce(ControlRequest) -> ControlResponse,
) -> ControlResponse {
    match serde_json::from_str::<ControlMessage>(line) {
        Ok(ControlMessage::Request(request)) => match refuse_unless_declared(&request.method) {
            Some(reason) => ControlResponse::error(request.id, "not_declared", reason),
            None => dispatch(request),
        },
        // Anything else on this transport is a client mistake, and saying so
        // beats closing the connection on it.
        Ok(_) => ControlResponse::error(
            String::new(),
            "unsupported",
            "the control socket takes requests".to_string(),
        ),
        Err(error) => ControlResponse::error(String::new(), "bad_request", error.to_string()),
    }
}

/// Why a method is closed on this transport, or `None` when it is open.
///
/// The development websocket is deliberately not filtered — a developer
/// driving their own session should see everything the runtime can do. This
/// endpoint is different: a product declared what it exposes, and the names
/// it declared are what a user consented to. A namespace that is merely
/// compiled in must not therefore be reachable.
fn refuse_unless_declared(method: &str) -> Option<String> {
    // A client has to be able to ask whether anyone is home.
    if method == lingxia_control_protocol::methods::ECHO {
        return None;
    }
    let namespace = method
        .split_once('.')
        .map(|(head, _)| head)
        .unwrap_or(method);
    // A provider registers extra namespaces at startup. Shipping the handler
    // is the declaration, so this does not wait for YAML or app.json.
    if crate::extra::is_registered_host_namespace(namespace) {
        return None;
    }
    // Every gate below closes when it cannot prove otherwise. A product whose
    // configuration has not loaded yet, or one carrying a namespace nobody has
    // added a row for, must refuse rather than hand a local process the whole
    // automation surface on the strength of a missing file.
    let Some(config) = lingxia_app_context::app_config() else {
        return Some("this product has no configuration to declare capabilities in".to_string());
    };
    refuse_for_product(method, config.capabilities.as_ref())
}

/// Apply the shipped-product method allowlist and its declared capabilities.
/// The dev websocket dispatches directly and deliberately never calls this.
fn refuse_for_product(
    method: &str,
    capabilities: Option<&lingxia_app_context::CapabilitiesConfig>,
) -> Option<String> {
    use lingxia_control_protocol::methods;

    if method == methods::ECHO {
        return None;
    }
    let namespace = method
        .split_once('.')
        .map(|(head, _)| head)
        .unwrap_or(method);
    if crate::extra::is_registered_host_namespace(namespace) {
        return None;
    }
    let Some(capabilities) = capabilities else {
        return Some(format!("{namespace} is not declared by this product"));
    };
    let (declared_as, declared) = match namespace {
        "desktop" => ("computerUse", capabilities.computer_use),
        "browser" => ("browserUse", capabilities.browser_use),
        "app"
            if matches!(
                method,
                methods::app::DOCTOR
                    | methods::app::SCREENSHOT
                    | methods::app::WINDOWS
                    | methods::app::MOUSE
                    | methods::app::KEYBOARD
            ) =>
        {
            ("appUse", capabilities.app_use_effective())
        }
        "app" => return Some(format!("{method} is not exposed by product control")),
        // `lxapp.*` is the development surface: it includes arbitrary Logic
        // evaluation plus app installation and lifecycle operations. appUse
        // grants only this product's host windows, never those dev handlers.
        "lxapp" => {
            return Some(format!(
                "{method} is available only through a development session"
            ));
        }
        // A namespace nobody has declared a capability for is not a namespace
        // the user consented to, whatever the build happens to have linked in.
        other => {
            return Some(format!("{other} is not declared by this product"));
        }
    };
    (!declared).then(|| format!("{declared_as} is not declared by this product"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests deliberately advance the process-wide connection epoch.
    // Cargo runs sibling tests in parallel, so serialize the cases that stop
    // listeners or one test can make another test's fresh client look stale.
    static LIFECYCLE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn stub(request: ControlRequest) -> ControlResponse {
        ControlResponse::success(
            request.id,
            Some(serde_json::json!({"method": request.method, "params": request.params})),
        )
    }

    #[test]
    fn routes_a_request_to_the_handler_chain() {
        let replied = reply_with(
            r#"{"type":"request","id":"1","method":"echo","params":{"a":1}}"#,
            stub,
        );
        assert_eq!(replied.id, "1");
        assert!(replied.error.is_none());
        assert_eq!(
            replied.result,
            Some(serde_json::json!({"method": "echo", "params": {"a": 1}}))
        );
    }

    #[test]
    fn answers_rather_than_hangs_up_on_bad_input() {
        for (line, code) in [
            ("not json", "bad_request"),
            (r#"{"type":"response","id":"1"}"#, "unsupported"),
        ] {
            let response = reply_with(line, |_| panic!("must not dispatch"));
            assert_eq!(
                response.error.map(|error| error.code).as_deref(),
                Some(code)
            );
        }
    }

    #[test]
    fn endpoint_publication_atomically_replaces_the_previous_value() {
        let control_dir = std::env::temp_dir().join(format!(
            "lingxia-control-publish-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&control_dir);
        std::fs::create_dir_all(&control_dir).unwrap();
        let endpoint = lingxia_control_protocol::local_control::endpoint_file(&control_dir);
        std::fs::write(&endpoint, "old-endpoint").unwrap();

        publish_endpoint(&control_dir, "new-endpoint").unwrap();

        assert_eq!(std::fs::read_to_string(&endpoint).unwrap(), "new-endpoint");
        assert_eq!(
            std::fs::read_dir(&control_dir).unwrap().count(),
            1,
            "temporary endpoint publication files must not remain"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&control_dir)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }

        let _ = std::fs::remove_dir_all(control_dir);
    }

    fn stub_line(line: &str) -> ControlResponse {
        reply_with(line, stub)
    }

    /// A product that cannot prove a namespace was declared must refuse it.
    /// The failure this guards against is silent: no configuration loaded yet
    /// reads exactly like no restrictions, and the endpoint would hand a local
    /// process the whole automation surface on the strength of a missing file.
    #[test]
    fn an_undeclared_namespace_is_refused_not_allowed() {
        // No app config is installed in a unit test, which is precisely the
        // "cannot prove it" case.
        for method in [
            "desktop.windows",
            "browser.open",
            "app.doctor",
            "lxapp.list",
        ] {
            assert!(
                refuse_unless_declared(method).is_some(),
                "{method} must be refused when nothing declares it"
            );
        }
        // A name nobody has written a row for is not a name a user consented
        // to, whatever the build linked in.
        assert!(refuse_unless_declared("filesystem.read").is_some());
        // Except the liveness probe: a client has to be able to ask whether
        // anyone is home before it knows what it may ask for.
        assert!(refuse_unless_declared(lingxia_control_protocol::methods::ECHO).is_none());
    }

    #[test]
    fn a_host_namespace_cannot_shadow_a_privileged_framework_namespace() {
        let registration = std::panic::catch_unwind(|| {
            crate::register_control_namespace("desktop", |_, _| Some(Ok(None)));
        });
        assert!(registration.is_err());

        let capabilities = lingxia_app_context::CapabilitiesConfig::default();
        assert!(
            refuse_for_product(
                lingxia_control_protocol::methods::desktop::pointer::CLICK,
                Some(&capabilities),
            )
            .is_some()
        );
    }

    #[test]
    fn product_control_exposes_only_explicit_host_app_methods() {
        use lingxia_control_protocol::methods;

        let capabilities = lingxia_app_context::CapabilitiesConfig {
            app_use: true,
            ..Default::default()
        };
        for method in [
            methods::app::DOCTOR,
            methods::app::SCREENSHOT,
            methods::app::WINDOWS,
            methods::app::MOUSE,
            methods::app::KEYBOARD,
        ] {
            assert_eq!(
                refuse_for_product(method, Some(&capabilities)),
                None,
                "{method} is part of the product's own-window surface"
            );
        }

        for method in [
            methods::lxapp::LIST,
            methods::lxapp::EVAL,
            methods::lxapp::OPEN,
            methods::lxapp::CLOSE,
            methods::lxapp::RESTART,
            methods::lxapp::UNINSTALL,
            methods::lxapp_page::EVAL,
            methods::lxapp_nav::TO,
        ] {
            let reason = refuse_for_product(method, Some(&capabilities))
                .unwrap_or_else(|| panic!("{method} escaped the product allowlist"));
            assert!(
                reason.contains("development session"),
                "unexpected refusal for {method}: {reason}"
            );
        }

        assert!(refuse_for_product("app.future_method", Some(&capabilities)).is_some());

        crate::register_control_namespace("allowlist_extra_test", |_, _| Some(Ok(None)));
        assert_eq!(
            refuse_for_product("allowlist_extra_test.ping", Some(&capabilities)),
            None,
            "a registered extra namespace is admitted because a handler exists"
        );
        assert!(
            crate::app::declared_capabilities().contains(&"allowlist_extra_test"),
            "registering the handler declares the host namespace"
        );

        // The shared dispatcher is also the dev websocket's entry point. It
        // remains unfiltered; only the shipped-product transport applies the
        // allowlist above.
        let dev_response = crate::dispatch(ControlRequest {
            id: "dev".to_string(),
            method: methods::lxapp::DOCTOR.to_string(),
            params: None,
        });
        assert!(dev_response.error.is_none());
        assert_eq!(
            dev_response
                .result
                .as_ref()
                .and_then(|value| value["target"].as_str()),
            Some("lxapp")
        );
    }

    #[test]
    fn an_exited_accept_loop_is_reported_stopped_and_can_be_reaped() {
        let listening = Arc::new(AtomicBool::new(true));
        let thread_flag = Arc::clone(&listening);
        let accepting = std::thread::spawn(move || {
            let _mark_stopped = MarkStoppedOnDrop(thread_flag);
        });
        while !accepting.is_finished() {
            std::thread::yield_now();
        }
        let mut running = Running {
            endpoint: "test-endpoint".to_string(),
            listening,
            accepting: Some(accepting),
        };

        assert!(!listener_is_live(Some(&running)));
        assert!(running.accepting.take().unwrap().join().is_ok());
        assert!(running.accepting.is_none());
    }

    /// The real listener, over a real socket, switched off the way the
    /// settings toggle switches it off — the part that a framing test cannot
    /// reach and the part a user's decision depends on.
    #[cfg(unix)]
    #[test]
    fn stops_listening_when_switched_off() {
        use std::io::BufRead;

        let _lifecycle = LIFECYCLE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let state_dir = std::env::temp_dir().join(format!(
            "lingxia-control-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&state_dir).unwrap();
        let mut running = start(&state_dir, stub_line).expect("listener starts");
        let endpoint = running.endpoint.clone();

        let mut client = std::os::unix::net::UnixStream::connect(&endpoint).expect("connects");
        client
            .write_all(b"{\"type\":\"request\",\"id\":\"1\",\"method\":\"echo\"}\n")
            .unwrap();
        let mut reply = String::new();
        BufReader::new(client.try_clone().unwrap())
            .read_line(&mut reply)
            .unwrap();
        assert!(reply.contains("\"id\":\"1\""), "answered while on: {reply}");

        assert!(stop_accepting(&mut running), "accept thread exits cleanly");
        assert!(
            std::os::unix::net::UnixStream::connect(&endpoint).is_err(),
            "endpoint still accepts after being switched off"
        );
        let _ = std::fs::remove_dir_all(&state_dir);
    }

    #[cfg(windows)]
    #[derive(Debug)]
    enum PipeRead {
        Line(String),
        Closed,
        TimedOut,
    }

    #[cfg(windows)]
    fn read_pipe_line(client: &std::fs::File) -> PipeRead {
        use std::io::BufRead;

        let reader = client.try_clone().expect("pipe handle duplicates");
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(reader).read_line(&mut line);
            let _ = send.send((result, line));
        });
        match receive.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok((Ok(read), line)) if read > 0 => PipeRead::Line(line),
            Ok(_) => PipeRead::Closed,
            Err(_) => PipeRead::TimedOut,
        }
    }

    /// An idle client owns a pipe instance after the listener stops. Restarting
    /// must still claim a fresh endpoint, while the old connection stays shut.
    #[cfg(windows)]
    #[test]
    fn named_pipe_restarts_while_an_old_client_is_open() {
        let _lifecycle = LIFECYCLE_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let state_dir = std::env::temp_dir();
        let epoch = EPOCH.load(Ordering::SeqCst);
        drop(platform::Listener::bind(&state_dir, epoch).expect("first pipe claims its name"));
        drop(
            platform::Listener::bind(&state_dir, epoch)
                .expect("dropping an unused first instance releases its name"),
        );

        let mut running = start(&state_dir, stub_line).expect("named pipe starts");
        let endpoint = running.endpoint.clone();

        let mut client = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&endpoint)
            .expect("named pipe connects");
        client
            .write_all(b"{\"type\":\"request\",\"id\":\"1\",\"method\":\"echo\"}\n")
            .unwrap();
        let reply = match read_pipe_line(&client) {
            PipeRead::Line(reply) => reply,
            other => panic!("named pipe did not answer: {other:?}"),
        };
        assert!(reply.contains("\"id\":\"1\""), "answered while on: {reply}");

        assert!(stop_accepting(&mut running), "first listener exits cleanly");
        let mut restarted = start(&state_dir, stub_line).expect("named pipe restarts");
        assert_ne!(restarted.endpoint, endpoint);
        let mut fresh = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&restarted.endpoint)
            .expect("restarted named pipe connects");
        fresh
            .write_all(b"{\"type\":\"request\",\"id\":\"2\",\"method\":\"echo\"}\n")
            .unwrap();
        let fresh_reply = match read_pipe_line(&fresh) {
            PipeRead::Line(reply) => reply,
            other => panic!("restarted named pipe did not answer: {other:?}"),
        };
        assert!(
            fresh_reply.contains("\"id\":\"2\""),
            "answered after restart: {fresh_reply}"
        );

        let stale = match client
            .write_all(b"{\"type\":\"request\",\"id\":\"stale\",\"method\":\"echo\"}\n")
        {
            Ok(()) => read_pipe_line(&client),
            Err(_) => PipeRead::Closed,
        };
        assert!(
            matches!(&stale, PipeRead::Closed),
            "old connection: {stale:?}"
        );

        drop(fresh);
        assert!(
            stop_accepting(&mut restarted),
            "restarted listener exits cleanly"
        );
    }
}
