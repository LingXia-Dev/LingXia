//! The product's local control socket.
//!
//! This is not the development websocket. That one exists so `lingxia dev` can
//! drive an app across a network to a phone; this one exists so a *shipped*
//! product can offer a command line and agent skills that drive it — the same
//! methods, reached without a dev session, over an IPC that never leaves the
//! machine.
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

pub mod launcher;

#[cfg_attr(unix, path = "unix.rs")]
#[cfg_attr(windows, path = "windows.rs")]
mod platform;

pub use platform::endpoint_name;

/// Where this product's endpoint lives, once [`install`] has run.
static STATE_DIR: OnceLock<PathBuf> = OnceLock::new();
static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

struct Running {
    endpoint: String,
    listening: Arc<AtomicBool>,
    /// Joined before this slot is released. The unix socket's pathname is
    /// shared between runs, and a listener still unwinding would otherwise
    /// unlink the socket a rebind had just created.
    accepting: Option<std::thread::JoinHandle<()>>,
}

/// Bumped every time the endpoint is switched off. A connection accepted under
/// an older epoch stops being answered, so closing the door also closes the
/// connections already through it rather than only the door.
static EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Make the control socket available, and start it if the user has said yes.
///
/// A host calls this from `start_services` when it ships the capability.
/// Shipping the capability is not the same as switching it on: this endpoint
/// hands any local process the product's whole automation surface, so the
/// build decides whether the ability exists and the user decides whether it
/// listens. It is off until they say otherwise, and [`set_enabled`] flips it
/// while the app runs — no restart, because a settings toggle that needs one
/// is a toggle people stop trusting.
pub fn install() -> std::io::Result<()> {
    let state_dir =
        lingxia::app::state_dir().map_err(|error| std::io::Error::other(error.to_string()))?;
    let enabled = lingxia_settings::control_enabled(
        state_dir
            .parent()
            .ok_or_else(|| std::io::Error::other("app state directory has no parent"))?,
    );
    let _ = STATE_DIR.set(state_dir);
    if enabled {
        set_enabled(true)?;
    } else {
        // Not just "do not start": a launcher left from a previous run would
        // still be on `PATH`, pointing at an endpoint nobody is listening on.
        // Turning the capability off between runs has to clean up too.
        let state_dir = STATE_DIR.get().expect("just set");
        if let Err(error) = launcher::remove(state_dir) {
            log::warn!("stale product command not removed: {error}");
        }
        platform::clear_stale(state_dir);
        log::info!("control socket available but switched off");
    }
    Ok(())
}

/// Start or stop listening. Persisting the choice is the caller's job — the
/// settings surface owns that, and this stays callable from a test.
pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    let state_dir = STATE_DIR
        .get()
        .ok_or_else(|| std::io::Error::other("control socket is not installed"))?;
    let mut running = RUNNING.lock().unwrap_or_else(|error| error.into_inner());
    match (enabled, running.take()) {
        (true, Some(existing)) => {
            *running = Some(existing);
            Ok(())
        }
        (true, None) => {
            let started = start(state_dir, crate::dispatch_line)?;
            // Opening the endpoint and making the command typable are the same
            // decision; splitting them would leave a product that answers but
            // that nobody can address.
            if let Err(error) = launcher::install(state_dir, &started.endpoint) {
                log::warn!("product command not installed: {error}");
            }
            *running = Some(started);
            Ok(())
        }
        (false, Some(mut existing)) => {
            existing.listening.store(false, Ordering::SeqCst);
            EPOCH.fetch_add(1, Ordering::SeqCst);
            // The accept call is blocking, so it has to be woken to notice.
            // Connecting to ourselves is the wake-up; the loop then sees the
            // flag and drops the listener, which is what actually closes the
            // door.
            platform::poke(&existing.endpoint);
            // The accept loop unlinks the socket on its way out, and that has
            // to finish before anything can bind the same name again.
            if let Some(accepting) = existing.accepting.take()
                && accepting.join().is_err()
            {
                log::warn!("control accept thread panicked on the way out");
            }
            if let Err(error) = launcher::remove(state_dir) {
                log::warn!("product command not removed: {error}");
            }
            log::info!("control socket switched off");
            Ok(())
        }
        (false, None) => Ok(()),
    }
}

/// The interface answering questions about itself.
///
/// Both are reachable without a declared capability. `status` reveals only
/// what a successful connect already reveals, and `disable` can only take
/// automation away — a switch that needed permission to turn *off* would be
/// the wrong way round.
pub(crate) fn handle_control_command(
    method: &str,
) -> Option<Result<Option<serde_json::Value>, String>> {
    use lingxia_control_protocol::methods::control as name;
    match method {
        // The declared list rides here rather than on `app.doctor`, which is
        // itself behind `appUse` — a product that declared only `browserUse`
        // could not have answered, and a skill written against "unknown" then
        // describes every namespace the product will refuse.
        name::STATUS => Some(Ok(Some(serde_json::json!({
            "listening": is_listening(),
            "declared": crate::app::declared_capabilities(),
        })))),
        name::DISABLE => {
            // Stop now, and persist it, so the answer does not depend on which
            // of the two a later question happens to ask.
            let persisted = STATE_DIR
                .get()
                .and_then(|state_dir| state_dir.parent())
                .map(|app_data_dir| lingxia_settings::set_control_enabled(app_data_dir, false));
            let stopped = set_enabled(false);
            Some(match (stopped, persisted) {
                (Err(error), _) => Err(error.to_string()),
                (_, Some(Err(error))) => Err(error.to_string()),
                _ => Ok(Some(serde_json::json!({ "listening": false }))),
            })
        }
        _ => None,
    }
}

/// Whether the endpoint is listening right now.
pub fn is_listening() -> bool {
    RUNNING
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .is_some()
}

fn start(state_dir: &Path, handle: Handler) -> std::io::Result<Running> {
    let listener = platform::Listener::bind(state_dir)?;
    let endpoint = listener.name();
    log::info!("control socket listening on {endpoint}");
    let listening = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&listening);
    let epoch = EPOCH.load(Ordering::SeqCst);
    let accepting = std::thread::Builder::new()
        .name("lingxia-control".to_string())
        .spawn(move || {
            let mut consecutive_failures = 0u32;
            while flag.load(Ordering::SeqCst) {
                match listener.accept() {
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
    // The liveness probe, and the two methods that only ever reduce what is
    // possible: a client has to be able to ask whether anyone is home, and
    // switching automation off must never itself need permission.
    if matches!(
        method,
        lingxia_control_protocol::methods::ECHO
            | lingxia_control_protocol::methods::control::STATUS
            | lingxia_control_protocol::methods::control::DISABLE
    ) {
        return None;
    }
    let namespace = method
        .split_once('.')
        .map(|(head, _)| head)
        .unwrap_or(method);
    // Every gate below closes when it cannot prove otherwise. A product whose
    // configuration has not loaded yet, or one carrying a namespace nobody has
    // added a row for, must refuse rather than hand a local process the whole
    // automation surface on the strength of a missing file.
    let Some(config) = lingxia_app_context::app_config() else {
        return Some("this product has no configuration to declare capabilities in".to_string());
    };
    let Some(capabilities) = config.capabilities.as_ref() else {
        return Some(format!("{namespace} is not declared by this product"));
    };
    let (declared_as, declared) = match namespace {
        "desktop" => ("computerUse", capabilities.computer_use),
        "browser" => ("browserUse", capabilities.browser_use),
        // An lxapp page is this product's own surface, so it rides with
        // `appUse` rather than having a row of its own. Splitting it out is one
        // line here and one field there, the day a product wants to expose its
        // native windows without its pages.
        "app" | "lxapp" => ("appUse", capabilities.app_use_effective()),
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

    /// The real listener, over a real socket, switched off the way the
    /// settings toggle switches it off — the part that a framing test cannot
    /// reach and the part a user's decision depends on.
    #[cfg(unix)]
    #[test]
    fn stops_listening_when_switched_off() {
        use std::io::BufRead;

        let state_dir = std::env::temp_dir().join(format!(
            "lingxia-control-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&state_dir).unwrap();
        let running = start(&state_dir, stub_line).expect("listener starts");
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

        running.listening.store(false, Ordering::SeqCst);
        platform::poke(&endpoint);

        // The accept thread drops the listener on its way out, which unlinks
        // the socket; until it does, a connect may still succeed.
        let mut refused = false;
        for _ in 0..100 {
            if std::os::unix::net::UnixStream::connect(&endpoint).is_err() {
                refused = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(refused, "endpoint still accepts after being switched off");
        let _ = std::fs::remove_dir_all(&state_dir);
    }
}
