//! `lxdev network devtools`: the session's network traffic, live, in Chrome
//! DevTools. The host streams its live feed as `network` events in the
//! session log, which this follows the way `lxdev logs -f` does; it serves
//! them as the DevTools protocol's `Network` domain on a loopback port and
//! opens Chrome's bundled DevTools frontend on that port.
//!
//! For people only: it runs until Ctrl-C or the session ends. Agents use
//! `capture`.

use crate::client::{self, CommandError};
use crate::logs::{follow_session_log, log_end};
use crate::project::SessionInfo;
use anyhow::{Context, Result, anyhow, bail};
use lingxia_control_protocol::dev_session::{DevSessionEvent, event_kinds};
use lingxia_control_protocol::methods::session::network as method;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tungstenite::Message;

/// The host's lease is 30s; renewing well inside it survives a busy host.
const RENEW_EVERY: Duration = Duration::from_secs(5);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// A 3xx hop is a redirect if its next hop starts within this; else it was
/// the answer (`redirect: 'manual'`).
const REDIRECT_WAIT: Duration = Duration::from_secs(1);
/// Hops, and DevTools events, kept for replay to a window that opens
/// later; an event stream can carry a thousand frames in one hop.
const MAX_HOPS: usize = 2_000;
const MAX_LOG_EVENTS: usize = 50_000;
/// The one frame and loader every request belongs to.
const FRAME: &str = "lxdev";

pub fn run(session: &SessionInfo) -> Result<()> {
    let ws = session.ws_url.clone();
    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let interrupted = interrupted.clone();
        let _ = ctrlc::set_handler(move || interrupted.store(true, Ordering::Release));
    }
    // Follow from here, then start the feed: its backlog lands after.
    let start = log_end(Path::new(&session.log_file))?;
    // Fails early on a host without the feed, before a window opens.
    watch(&ws, true)?;

    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let hub = Arc::new(Hub::new(ws.clone()));
    {
        let hub = hub.clone();
        std::thread::Builder::new()
            .name("lxdev-cdp".into())
            .spawn(move || accept(listener, hub))?;
    }
    let url = format!(
        "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:{port}/{FRAME}&panel=network"
    );
    println!("Chrome DevTools: {url}");
    if let Err(err) = open_devtools(&url) {
        println!("Could not open DevTools ({err:#}); open that address in Chrome or Edge.");
    }
    println!("Showing the session's network traffic live. Ctrl-C to stop.");

    // Renewing is a command, which can wait behind another one; the stream
    // never does.
    let done = Arc::new(AtomicBool::new(false));
    {
        let (ws, done) = (ws.clone(), done.clone());
        std::thread::Builder::new()
            .name("lxdev-watch-lease".into())
            .spawn(move || {
                let mut last = Instant::now();
                while !done.load(Ordering::Acquire) {
                    std::thread::sleep(Duration::from_millis(200));
                    if last.elapsed() >= RENEW_EVERY {
                        let _ = watch(&ws, false);
                        last = Instant::now();
                    }
                }
            })?;
    }

    let mut on_line = |line: &str| -> Result<()> {
        let Ok(event) = serde_json::from_str::<DevSessionEvent>(line) else {
            return Ok(());
        };
        if event.kind == event_kinds::NETWORK {
            hub.apply(&event.data, Instant::now());
        }
        Ok(())
    };
    let mut keep_going = || {
        hub.tick(Instant::now());
        !interrupted.load(Ordering::Acquire)
    };
    let ended = follow_session_log(session, start, &mut on_line, &mut keep_going)?;
    done.store(true, Ordering::Release);
    if ended {
        eprintln!("Dev session ended.");
        return Ok(());
    }
    let _ = client::execute_command(&ws, method::UNWATCH, None);
    Ok(())
}

/// Start or renew the host's live feed.
fn watch(ws: &str, backlog: bool) -> Result<()> {
    command(ws, method::WATCH, json!({ "backlog": backlog })).map(|_| ())
}

fn command(ws: &str, handler: &str, args: Value) -> Result<Value> {
    match client::execute_command_until(ws, handler, Some(args), Some(COMMAND_TIMEOUT), &|| false) {
        Ok(value) => Ok(value.unwrap_or(Value::Null)),
        Err(err)
            if err
                .downcast_ref::<CommandError>()
                .is_some_and(predates_watch) =>
        {
            bail!(
                "this host predates `lxdev network devtools`: rebuild it (or update the Runner) \
                 with this lxdev's release"
            )
        }
        Err(err) => Err(err),
    }
}

/// A host without the feed: no such method at all, or a network namespace
/// that does not know it yet.
fn predates_watch(err: &CommandError) -> bool {
    err.code == "unknown_method" || err.message.starts_with("unknown network method")
}

// ------------------------------ the browser ------------------------------

/// How long a browser just started may take to report its debugging port.
const BROWSER_START: Duration = Duration::from_secs(20);

/// Open `url` in Chrome, else Edge (macOS and Windows). A browser will not
/// open a `devtools://` address handed to it from outside: one on its
/// command line is dropped for a new tab. So lxdev keeps an instance in a
/// profile of its own (`~/.lingxia/devtools-browser`), with remote
/// debugging on loopback, and asks it over CDP (`Target.createTarget`) to
/// open the address. A later run reuses that instance.
fn open_devtools(url: &str) -> Result<()> {
    let (name, program) =
        installed_browser().ok_or_else(|| anyhow!("neither Chrome nor Edge is installed"))?;
    let profile = devtools_profile(name)?;
    let port_file = profile.join("DevToolsActivePort");
    if let Some(endpoint) = browser_endpoint(&port_file)
        && open_target(&endpoint, url, false).is_ok()
    {
        return Ok(());
    }
    let _ = std::fs::remove_file(&port_file);
    std::fs::create_dir_all(&profile)?;
    std::process::Command::new(&program)
        .arg(format!("--user-data-dir={}", profile.display()))
        .args([
            "--remote-debugging-port=0",
            "--no-first-run",
            "--no-default-browser-check",
            "about:blank",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("failed to start {}", program.display()))?;
    let deadline = Instant::now() + BROWSER_START;
    let endpoint = loop {
        if let Some(endpoint) = browser_endpoint(&port_file) {
            break endpoint;
        }
        if Instant::now() > deadline {
            bail!("{name} did not report its debugging port");
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    open_target(&endpoint, url, true)
}

/// Chrome, else Edge, where their installers put them.
fn installed_browser() -> Option<(&'static str, PathBuf)> {
    let candidates: Vec<(&'static str, Option<PathBuf>)> = if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let chrome = "Google Chrome.app/Contents/MacOS/Google Chrome";
        let edge = "Microsoft Edge.app/Contents/MacOS/Microsoft Edge";
        vec![
            ("chrome", Some(Path::new("/Applications").join(chrome))),
            (
                "chrome",
                home.as_ref()
                    .map(|home| home.join("Applications").join(chrome)),
            ),
            ("edge", Some(Path::new("/Applications").join(edge))),
            (
                "edge",
                home.map(|home| home.join("Applications").join(edge)),
            ),
        ]
    } else if cfg!(windows) {
        let under = |root: &str, path: &str| {
            std::env::var_os(root).map(|root| PathBuf::from(root).join(path))
        };
        let chrome = r"Google\Chrome\Application\chrome.exe";
        let edge = r"Microsoft\Edge\Application\msedge.exe";
        vec![
            ("chrome", under("ProgramFiles", chrome)),
            ("chrome", under("ProgramFiles(x86)", chrome)),
            ("chrome", under("LOCALAPPDATA", chrome)),
            ("edge", under("ProgramFiles(x86)", edge)),
            ("edge", under("ProgramFiles", edge)),
        ]
    } else {
        Vec::new()
    };
    candidates
        .into_iter()
        .find_map(|(name, path)| path.filter(|path| path.is_file()).map(|path| (name, path)))
}

/// `~/.lingxia/devtools-browser/<chrome|edge>`: nothing of the person's own
/// browsing is in it.
fn devtools_profile(name: &str) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| anyhow!("cannot locate the home directory"))?;
    Ok(PathBuf::from(home)
        .join(".lingxia")
        .join("devtools-browser")
        .join(name))
}

/// The browser-level CDP endpoint a running instance wrote into its
/// profile: the port, then the path.
fn browser_endpoint(port_file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(port_file).ok()?;
    let mut lines = text.lines();
    let port: u16 = lines.next()?.trim().parse().ok()?;
    let path = lines.next()?.trim();
    path.starts_with('/')
        .then(|| format!("ws://127.0.0.1:{port}{path}"))
}

/// Ask the browser at `endpoint` to open `url`, and close what is left
/// over: the empty tab a just-started instance opened with, and DevTools
/// tabs of earlier runs whose lxdev is gone.
fn open_target(endpoint: &str, url: &str, close_blank: bool) -> Result<()> {
    let (mut socket, _) = tungstenite::connect(endpoint)?;
    if let tungstenite::stream::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    }
    let mut call = |id: u64, method: &str, params: Value| -> Result<Value> {
        let request = json!({ "id": id, "method": method, "params": params });
        socket.send(Message::Text(request.to_string().into()))?;
        loop {
            let Message::Text(text) = socket.read()? else {
                continue;
            };
            let reply: Value = serde_json::from_str(&text)?;
            if reply["id"] != id {
                continue;
            }
            if let Some(error) = reply["error"]["message"].as_str() {
                bail!("{method}: {error}");
            }
            return Ok(reply["result"].clone());
        }
    };
    let opened = call(1, "Target.createTarget", json!({ "url": url }))?;
    let targets = call(2, "Target.getTargets", json!({}))?;
    let leftover = targets["targetInfos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|target| target["type"] == "page" && target["targetId"] != opened["targetId"])
        .filter(|target| {
            let page = target["url"].as_str().unwrap_or("");
            (close_blank && page == "about:blank") || abandoned_devtools(page)
        })
        .filter_map(|target| target["targetId"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    for (n, target) in leftover.into_iter().enumerate() {
        let _ = call(
            3 + n as u64,
            "Target.closeTarget",
            json!({ "targetId": target }),
        );
    }
    Ok(())
}

/// A DevTools tab of an lxdev that has exited: nothing listens on its port.
fn abandoned_devtools(page: &str) -> bool {
    let Some(rest) = page.strip_prefix("devtools://devtools/bundled/inspector.html?ws=127.0.0.1:")
    else {
        return false;
    };
    let Some(port) = rest
        .split('/')
        .next()
        .and_then(|port| port.parse::<u16>().ok())
    else {
        return false;
    };
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_err()
}

// ------------------------------- the feed -------------------------------

/// What lxdev knows of the traffic, and the DevTools events made of it,
/// replayed to each window that connects.
struct Hub {
    ws: String,
    state: Mutex<HubState>,
}

#[derive(Default)]
struct HubState {
    /// `(CDP request id, CDP event text)` in emission order.
    log: VecDeque<(String, String)>,
    /// Per host request id: one hop of a request (a redirect makes more).
    hops: HashMap<u64, Hop>,
    order: VecDeque<u64>,
    /// CDP request id → its newest hop, whose body DevTools asks for.
    chains: HashMap<String, u64>,
    /// 3xx hops that ended, waiting to see whether a next hop follows.
    redirects: HashMap<u64, Instant>,
    clients: Vec<Sender<String>>,
}

struct Hop {
    /// The CDP request id: the first hop's host id.
    cdp: String,
    /// The host's `request` event, or the finished record standing in.
    sent: Value,
    response: Option<Value>,
    /// The response is held back as a possible redirect.
    held: bool,
    /// A next hop continues this request: its 3xx went out as that hop's
    /// `redirectResponse`, so nothing more of this hop is shown.
    followed: bool,
    responded: bool,
    /// The host's `end` event (no bodies).
    record: Option<Value>,
    /// The kept record with bodies, once fetched.
    body: Option<Value>,
}

impl Hub {
    fn new(ws: String) -> Self {
        Self {
            ws,
            state: Mutex::new(HubState::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HubState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn apply(&self, event: &Value, now: Instant) {
        let mut state = self.lock();
        state.apply(event, now);
        state.trim();
    }

    fn tick(&self, now: Instant) {
        self.lock().release_redirects(now);
    }

    /// Replay what happened so far to a new window, then stream to it.
    fn subscribe(&self, tx: Sender<String>) {
        let mut state = self.lock();
        for (_, text) in &state.log {
            let _ = tx.send(text.clone());
        }
        state.clients.push(tx);
    }

    fn answer(&self, message: &Value) -> Value {
        let id = message["id"].clone();
        let method = message["method"].as_str().unwrap_or("");
        let result = match method {
            "Network.getResponseBody" => self
                .body(&message["params"], "response")
                .map(|(body, base64)| json!({ "body": body, "base64Encoded": base64 })),
            "Network.getRequestPostData" => self
                .body(&message["params"], "request")
                .map(|(body, _)| json!({ "postData": body })),
            "Page.getResourceTree" => {
                Ok(json!({ "frameTree": { "frame": frame(), "resources": [] } }))
            }
            "Page.getFrameTree" => Ok(json!({ "frameTree": { "frame": frame() } })),
            "Runtime.evaluate" | "Runtime.callFunctionOn" => {
                Err(format!("'{method}' is not available"))
            }
            _ if method.starts_with("Network.")
                || method.ends_with(".enable")
                || method.ends_with(".disable") =>
            {
                Ok(json!({}))
            }
            _ => Err(format!("'{method}' wasn't found")),
        };
        match result {
            Ok(result) => json!({ "id": id, "result": result }),
            Err(message) => json!({ "id": id, "error": { "code": -32601, "message": message } }),
        }
    }

    /// A request's body, fetched from the host on first ask (without
    /// holding the lock: the command can wait behind another one).
    fn body(&self, params: &Value, side: &str) -> Result<(String, bool), String> {
        const NOT_FOUND: &str = "No data found for resource with given identifier";
        let cdp = params["requestId"].as_str().unwrap_or("");
        let (hop_id, cached) = {
            let state = self.lock();
            let hop_id = *state.chains.get(cdp).ok_or(NOT_FOUND)?;
            let hop = state.hops.get(&hop_id).ok_or(NOT_FOUND)?;
            let record = hop.record.as_ref().ok_or(NOT_FOUND)?;
            if record["past"] == true {
                return Err("finished before lxdev network devtools started".into());
            }
            (hop_id, hop.body.clone())
        };
        let full = match cached {
            Some(full) => full,
            None => {
                let full =
                    command(&self.ws, method::BODY, json!({ "id": hop_id })).map_err(|err| {
                        match err.downcast_ref::<CommandError>() {
                            Some(err) if err.code == "not_found" => {
                                "no longer kept by the host".to_string()
                            }
                            _ => format!("{err:#}"),
                        }
                    })?;
                if let Some(hop) = self.lock().hops.get_mut(&hop_id) {
                    hop.body = Some(full.clone());
                }
                full
            }
        };
        body_text(&full, side).ok_or_else(|| "no body was kept".into())
    }
}

impl HubState {
    fn emit(&mut self, cdp: &str, method: &str, params: Value) {
        let text = json!({ "method": method, "params": params }).to_string();
        self.clients.retain(|tx| tx.send(text.clone()).is_ok());
        self.log.push_back((cdp.to_string(), text));
    }

    fn apply(&mut self, event: &Value, now: Instant) {
        match event["type"].as_str() {
            Some("lost") => {
                eprintln!(
                    "warning: {} live network events were dropped (the host fell behind)",
                    event["events"]
                );
            }
            Some("sse") => {
                // An event-stream frame's own `id` is its event id.
                let Some(id) = event["requestId"].as_u64() else {
                    return;
                };
                if let Some(cdp) = self.hops.get(&id).map(|hop| hop.cdp.clone()) {
                    let message = sse_message(&cdp, event);
                    self.emit(&cdp, "Network.eventSourceMessageReceived", message);
                }
            }
            kind => {
                let Some(id) = event["id"].as_u64() else {
                    return;
                };
                match kind {
                    Some("request") => self.start(id, event),
                    Some("response") => self.respond(id, event),
                    Some("data") => self.data(id, event),
                    Some("end") => self.end(id, event, now),
                    _ => {}
                }
            }
        }
    }

    /// A hop starts. One following a held 3xx continues that request, as
    /// DevTools shows redirects: the same id, with the 3xx as
    /// `redirectResponse`. The 3xx hop may not have ended yet.
    fn start(&mut self, id: u64, request: &Value) {
        if self.hops.contains_key(&id) {
            return;
        }
        let previous = request["redirectedFrom"]
            .as_u64()
            .and_then(|from| self.hops.get_mut(&from))
            .filter(|hop| hop.held && !hop.followed);
        let (cdp, redirect, from) = match previous {
            Some(hop) => {
                hop.followed = true;
                let redirect = hop
                    .response
                    .as_ref()
                    .map(|response| response_object(&hop.sent, response));
                (
                    hop.cdp.clone(),
                    redirect,
                    request["redirectedFrom"].as_u64(),
                )
            }
            None => (id.to_string(), None, None),
        };
        if let Some(from) = from {
            self.redirects.remove(&from);
        }
        let params = will_be_sent(&cdp, request, redirect);
        self.emit(&cdp, "Network.requestWillBeSent", params);
        self.chains.insert(cdp.clone(), id);
        self.order.push_back(id);
        self.hops.insert(
            id,
            Hop {
                cdp,
                sent: request.clone(),
                response: None,
                held: false,
                followed: false,
                responded: false,
                record: None,
                body: None,
            },
        );
    }

    fn respond(&mut self, id: u64, response: &Value) {
        let Some(hop) = self.hops.get_mut(&id) else {
            return;
        };
        if hop.responded || hop.held {
            return;
        }
        hop.response = Some(response.clone());
        if is_redirect(response) {
            hop.held = true;
            return;
        }
        self.release(id);
    }

    /// Show a hop's response (held or not).
    fn release(&mut self, id: u64) {
        let Some(hop) = self.hops.get_mut(&id) else {
            return;
        };
        let Some(response) = hop.response.clone() else {
            return;
        };
        hop.held = false;
        hop.responded = true;
        let (cdp, sent) = (hop.cdp.clone(), hop.sent.clone());
        self.emit(
            &cdp,
            "Network.responseReceived",
            response_received(&cdp, &sent, &response),
        );
    }

    fn data(&mut self, id: u64, event: &Value) {
        let Some(hop) = self.hops.get(&id).filter(|hop| hop.responded) else {
            return;
        };
        let cdp = hop.cdp.clone();
        let bytes = event["bytes"].as_u64().unwrap_or(0);
        self.emit(
            &cdp,
            "Network.dataReceived",
            json!({
                "requestId": cdp,
                "timestamp": seconds(event["time"].as_f64().unwrap_or(0.0)),
                "dataLength": bytes,
                "encodedDataLength": bytes,
            }),
        );
    }

    /// A hop finished; one never seen start is told whole. A completed 3xx
    /// waits to see whether it was a redirect.
    fn end(&mut self, id: u64, record: &Value, now: Instant) {
        if self.hops.get(&id).is_some_and(|hop| hop.record.is_some()) {
            return;
        }
        self.start(id, record);
        if let Some(hop) = self.hops.get_mut(&id).filter(|hop| hop.followed) {
            // Its request goes on under the next hop; only keep the record.
            hop.record = Some(record.clone());
            return;
        }
        let status = record["status"].as_u64().unwrap_or(0);
        if status > 0 {
            self.respond(id, record);
        }
        let Some(hop) = self.hops.get_mut(&id) else {
            return;
        };
        hop.record = Some(record.clone());
        if hop.held && record["outcome"] == "completed" {
            self.redirects.insert(id, now);
            return;
        }
        self.finish(id);
    }

    /// 3xx hops no next hop followed were answers: show them.
    fn release_redirects(&mut self, now: Instant) {
        let due: Vec<u64> = self
            .redirects
            .iter()
            .filter(|(_, at)| now.duration_since(**at) >= REDIRECT_WAIT)
            .map(|(id, _)| *id)
            .collect();
        for id in due {
            self.redirects.remove(&id);
            self.finish(id);
        }
    }

    fn finish(&mut self, id: u64) {
        if self.hops.get(&id).is_some_and(|hop| hop.held) {
            self.release(id);
        }
        let Some(hop) = self.hops.get(&id) else {
            return;
        };
        let Some(record) = hop.record.clone() else {
            return;
        };
        let (cdp, sent) = (hop.cdp.clone(), hop.sent.clone());
        let start = record["time"].as_f64().unwrap_or(0.0);
        let done = seconds(start + record["durationMs"].as_f64().unwrap_or(0.0));
        // Frames that were not streamed (an encoded stream) come at the end.
        for frame in record["events"].as_array().into_iter().flatten() {
            let mut frame = frame.clone();
            frame["time"] = json!(start + record["durationMs"].as_f64().unwrap_or(0.0));
            let message = sse_message(&cdp, &frame);
            self.emit(&cdp, "Network.eventSourceMessageReceived", message);
        }
        let outcome = record["outcome"].as_str().unwrap_or("completed");
        if outcome == "completed" {
            self.emit(
                &cdp,
                "Network.loadingFinished",
                json!({
                    "requestId": cdp,
                    "timestamp": done,
                    "encodedDataLength": record["responseBytes"].as_u64().unwrap_or(0),
                }),
            );
        } else {
            let mut params = json!({
                "requestId": cdp,
                "timestamp": done,
                "type": resource_type(&sent),
                "errorText": record["error"].as_str().unwrap_or(outcome),
                "canceled": outcome == "aborted",
            });
            if outcome == "blocked" {
                params["blockedReason"] = json!("other");
            }
            self.emit(&cdp, "Network.loadingFailed", params);
        }
    }

    /// Forget the oldest tenth of the hops, and their events, until both
    /// bounds hold.
    fn trim(&mut self) {
        while self.order.len() > MAX_HOPS || self.log.len() > MAX_LOG_EVENTS {
            let keep = self.order.len() * 9 / 10;
            while self.order.len() > keep {
                if let Some(id) = self.order.pop_front() {
                    self.hops.remove(&id);
                    self.redirects.remove(&id);
                }
            }
            let hops = &self.hops;
            self.chains.retain(|_, hop| hops.contains_key(hop));
            let chains = &self.chains;
            self.log.retain(|(cdp, _)| chains.contains_key(cdp));
        }
    }
}

// ------------------------------- CDP shapes -------------------------------

fn seconds(ms: f64) -> f64 {
    ms / 1000.0
}

fn frame() -> Value {
    json!({
        "id": FRAME,
        "loaderId": FRAME,
        "url": "lingxia://lxdev",
        "domainAndRegistry": "",
        "securityOrigin": "lingxia://lxdev",
        "mimeType": "text/html",
        "secureContextType": "Secure",
        "crossOriginIsolatedContextType": "NotIsolated",
        "gatedAPIFeatures": [],
    })
}

fn resource_type(request: &Value) -> &'static str {
    match request["kind"].as_str() {
        Some("fetch") => "Fetch",
        Some("sse") => "EventSource",
        _ => "Other",
    }
}

/// `[name, value]` pairs as a CDP header object; repeats join with `\n`.
fn headers(pairs: &Value) -> Value {
    let mut out = serde_json::Map::new();
    for pair in pairs.as_array().into_iter().flatten() {
        let (Some(name), Some(value)) = (pair[0].as_str(), pair[1].as_str()) else {
            continue;
        };
        match out.get_mut(name) {
            Some(Value::String(existing)) => {
                existing.push('\n');
                existing.push_str(value);
            }
            _ => {
                out.insert(name.to_string(), json!(value));
            }
        }
    }
    Value::Object(out)
}

fn header<'a>(pairs: &'a Value, name: &str) -> Option<&'a str> {
    pairs.as_array()?.iter().find_map(|pair| {
        pair[0]
            .as_str()
            .filter(|key| key.eq_ignore_ascii_case(name))
            .and(pair[1].as_str())
    })
}

fn is_redirect(response: &Value) -> bool {
    response["status"]
        .as_u64()
        .is_some_and(|status| (300..400).contains(&status))
        && header(&response["responseHeaders"], "location").is_some()
}

fn will_be_sent(cdp: &str, request: &Value, redirect: Option<Value>) -> Value {
    let time = seconds(request["time"].as_f64().unwrap_or(0.0));
    let owner = request["owner"].as_str().unwrap_or("unknown");
    let mut params = json!({
        "requestId": cdp,
        "loaderId": FRAME,
        "documentURL": "lingxia://lxdev",
        "request": {
            "url": request["url"],
            "method": request["method"],
            "headers": headers(&request["requestHeaders"]),
            "hasPostData": request["requestBytes"].as_u64().is_some_and(|bytes| bytes > 0),
            "initialPriority": "High",
            "referrerPolicy": "no-referrer",
        },
        "timestamp": time,
        "wallTime": time,
        // The Initiator column names who sent it: an lxapp or `host:<subsystem>`.
        "initiator": { "type": "script", "url": owner },
        "type": resource_type(request),
        "frameId": FRAME,
        "redirectHasExtraInfo": false,
    });
    if let Some(redirect) = redirect {
        params["redirectResponse"] = redirect;
    }
    params
}

fn response_received(cdp: &str, request: &Value, response: &Value) -> Value {
    let start = request["time"].as_f64().unwrap_or(0.0);
    let ttfb = response["ttfbMs"].as_f64().unwrap_or(0.0);
    json!({
        "requestId": cdp,
        "loaderId": FRAME,
        "frameId": FRAME,
        "timestamp": seconds(start + ttfb),
        "type": resource_type(request),
        "response": response_object(request, response),
    })
}

fn response_object(request: &Value, response: &Value) -> Value {
    let start = request["time"].as_f64().unwrap_or(0.0);
    let ttfb = response["ttfbMs"].as_f64().unwrap_or(0.0);
    let url = request["url"].as_str().unwrap_or("");
    let mime = header(&response["responseHeaders"], "content-type")
        .and_then(|kind| kind.split(';').next())
        .unwrap_or("")
        .trim();
    let protocol = match response["httpVersion"].as_str() {
        Some("HTTP/2.0") => "h2",
        Some("HTTP/3.0") => "h3",
        Some("HTTP/1.0") => "http/1.0",
        _ => "http/1.1",
    };
    json!({
        "url": url,
        "status": response["status"],
        "statusText": "",
        "headers": headers(&response["responseHeaders"]),
        "mimeType": mime,
        "connectionReused": false,
        "connectionId": 0,
        "encodedDataLength": 0,
        "protocol": protocol,
        "securityState": if url.starts_with("https:") { "secure" } else { "insecure" },
        "timing": timing(start, ttfb),
    })
}

/// Only send and first byte are known: the rest of the phases are `-1`, so
/// the waterfall shows the wait for the server and then the download.
fn timing(start_ms: f64, ttfb_ms: f64) -> Value {
    let mut out = json!({
        "requestTime": seconds(start_ms),
        "sendStart": 0.0,
        "sendEnd": 0.0,
        "receiveHeadersStart": ttfb_ms,
        "receiveHeadersEnd": ttfb_ms,
    });
    for phase in [
        "proxyStart",
        "proxyEnd",
        "dnsStart",
        "dnsEnd",
        "connectStart",
        "connectEnd",
        "sslStart",
        "sslEnd",
        "workerStart",
        "workerReady",
        "workerFetchStart",
        "workerRespondWithSettled",
        "pushStart",
        "pushEnd",
    ] {
        out[phase] = json!(-1);
    }
    out
}

/// An event-stream frame `{ event?, id?, data, time }` of request `cdp` as
/// an `eventSourceMessageReceived`.
fn sse_message(cdp: &str, frame: &Value) -> Value {
    json!({
        "requestId": cdp,
        "timestamp": seconds(frame["time"].as_f64().unwrap_or(0.0)),
        "eventName": frame["event"].as_str().unwrap_or("message"),
        "eventId": frame["id"].as_str().unwrap_or(""),
        "data": frame_text(&frame["data"]),
    })
}

fn frame_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// A kept record's `request` or `response` body as DevTools text, and
/// whether it is base64.
fn body_text(record: &Value, side: &str) -> Option<(String, bool)> {
    if side == "response" {
        if let Some(events) = record["events"].as_array() {
            let mut text = String::new();
            for event in events {
                if let Some(name) = event["event"].as_str() {
                    text.push_str(&format!("event: {name}\n"));
                }
                for line in frame_text(&event["data"]).split('\n') {
                    text.push_str(&format!("data: {line}\n"));
                }
                text.push('\n');
            }
            return Some((text, false));
        }
        if let Some(lines) = record["lines"].as_array() {
            let text: Vec<String> = lines.iter().map(frame_text).collect();
            return Some((text.join("\n"), false));
        }
    }
    let Some(body) = record.get(format!("{side}Body")) else {
        // No body field and no bytes on the wire: an empty body, not a lost one.
        let bytes = record[format!("{side}Bytes")].as_u64();
        return (bytes == Some(0)).then(|| (String::new(), false));
    };
    if let Some(object) = body.as_object()
        && object.len() == 1
    {
        if let Some(data) = object.get("base64").and_then(Value::as_str) {
            return Some((data.to_string(), true));
        }
        if let Some(note) = object.get("omitted").and_then(Value::as_str) {
            return Some((format!("({note})"), false));
        }
    }
    Some((frame_text(body), false))
}

// ------------------------------- the server -------------------------------

fn accept(listener: TcpListener, hub: Arc<Hub>) {
    for stream in listener.incoming().flatten() {
        let hub = hub.clone();
        let _ = std::thread::Builder::new()
            .name("lxdev-cdp-client".into())
            .spawn(move || serve(stream, &hub));
    }
}

/// One DevTools window: answer its commands; after `Network.enable`, replay
/// the traffic so far and stream the rest.
fn serve(stream: TcpStream, hub: &Hub) {
    use tungstenite::handshake::server::{ErrorResponse, Request, Response};
    // A web page can open a WebSocket to loopback; only DevTools may read
    // the traffic. Tools that are not pages send no Origin.
    #[allow(clippy::result_large_err)] // tungstenite's callback signature
    let only_devtools = |request: &Request, response: Response| {
        let origin = request
            .headers()
            .get("origin")
            .map(|origin| origin.to_str().unwrap_or("?"));
        if allowed_origin(origin) {
            return Ok(response);
        }
        let mut refused = ErrorResponse::new(Some("only Chrome DevTools may connect".into()));
        *refused.status_mut() = tungstenite::http::StatusCode::FORBIDDEN;
        Err(refused)
    };
    let Ok(mut socket) = tungstenite::accept_hdr(stream, only_devtools) else {
        return;
    };
    let _ = socket
        .get_mut()
        .set_read_timeout(Some(Duration::from_millis(50)));
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    let mut subscribed = false;
    loop {
        while let Ok(text) = rx.try_recv() {
            if socket.send(Message::Text(text.into())).is_err() {
                return;
            }
        }
        match socket.read() {
            Ok(Message::Text(text)) => {
                let Ok(message) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let reply = hub.answer(&message);
                if socket
                    .send(Message::Text(reply.to_string().into()))
                    .is_err()
                {
                    return;
                }
                if !subscribed && message["method"] == "Network.enable" {
                    subscribed = true;
                    hub.subscribe(tx.clone());
                }
            }
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(tungstenite::Error::Io(err))
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
    }
}

/// No Origin (not a page), or DevTools' own.
fn allowed_origin(origin: Option<&str>) -> bool {
    origin.is_none_or(|origin| origin.starts_with("devtools://"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(state: &HubState) -> Vec<Value> {
        state
            .log
            .iter()
            .map(|(_, text)| serde_json::from_str(text).unwrap())
            .collect()
    }

    fn methods(state: &HubState) -> Vec<String> {
        events(state)
            .iter()
            .map(|event| event["method"].as_str().unwrap().to_string())
            .collect()
    }

    fn feed(state: &mut HubState, now: Instant, list: Value) {
        for event in list.as_array().unwrap() {
            state.apply(event, now);
        }
    }

    #[test]
    fn a_live_request_becomes_the_devtools_sequence() {
        let mut state = HubState::default();
        feed(
            &mut state,
            Instant::now(),
            json!([
                { "type": "request", "id": 7, "owner": "app", "kind": "fetch", "method": "POST",
                  "url": "https://api.test/x", "time": 1000, "requestBytes": 2,
                  "requestHeaders": [["accept", "a"], ["accept", "b"]] },
                { "type": "response", "id": 7, "status": 200, "httpVersion": "HTTP/2.0",
                  "ttfbMs": 12.5, "responseHeaders": [["content-type", "application/json; charset=utf-8"]] },
                { "type": "data", "id": 7, "bytes": 9, "time": 1013 },
                { "type": "end", "id": 7, "status": 200, "outcome": "completed", "time": 1000,
                  "durationMs": 20, "responseBytes": 9, "kind": "fetch" },
            ]),
        );
        assert_eq!(
            methods(&state),
            [
                "Network.requestWillBeSent",
                "Network.responseReceived",
                "Network.dataReceived",
                "Network.loadingFinished"
            ]
        );
        let log = events(&state);
        assert_eq!(log[0]["params"]["request"]["headers"]["accept"], "a\nb");
        assert_eq!(log[0]["params"]["timestamp"], 1.0);
        let response = &log[1]["params"]["response"];
        assert_eq!(response["mimeType"], "application/json");
        assert_eq!(response["protocol"], "h2");
        assert_eq!(response["timing"]["receiveHeadersEnd"], 12.5);
    }

    #[test]
    fn a_redirect_is_one_devtools_request_with_its_3xx() {
        let mut state = HubState::default();
        let now = Instant::now();
        feed(
            &mut state,
            now,
            json!([
                { "type": "request", "id": 1, "kind": "fetch", "method": "GET",
                  "url": "https://a.test/old", "time": 0 },
                { "type": "response", "id": 1, "status": 302,
                  "responseHeaders": [["location", "/new"]] },
                { "type": "end", "id": 1, "status": 302, "outcome": "completed",
                  "time": 0, "durationMs": 5 },
                { "type": "request", "id": 2, "kind": "fetch", "method": "GET",
                  "url": "https://a.test/new", "time": 6, "redirectedFrom": 1 },
                { "type": "response", "id": 2, "status": 200, "responseHeaders": [] },
                { "type": "end", "id": 2, "status": 200, "outcome": "completed",
                  "time": 6, "durationMs": 4 },
            ]),
        );
        state.release_redirects(now + REDIRECT_WAIT * 2);
        assert_eq!(
            methods(&state),
            [
                "Network.requestWillBeSent",
                "Network.requestWillBeSent",
                "Network.responseReceived",
                "Network.loadingFinished"
            ]
        );
        let log = events(&state);
        assert_eq!(log[1]["params"]["requestId"], "1");
        assert_eq!(log[1]["params"]["redirectResponse"]["status"], 302);
        assert_eq!(log[3]["params"]["requestId"], "1");
        // DevTools asks for the newest hop's body.
        assert_eq!(state.chains["1"], 2);
    }

    #[test]
    fn a_redirect_merges_even_when_its_next_hop_starts_first() {
        let mut state = HubState::default();
        let now = Instant::now();
        feed(
            &mut state,
            now,
            json!([
                { "type": "request", "id": 1, "kind": "fetch", "method": "GET",
                  "url": "https://a.test/old", "time": 0 },
                { "type": "response", "id": 1, "status": 307,
                  "responseHeaders": [["location", "/new"]] },
                { "type": "request", "id": 2, "kind": "fetch", "method": "GET",
                  "url": "https://a.test/new", "time": 6, "redirectedFrom": 1 },
                // The 3xx hop's end arrives after its successor started.
                { "type": "end", "id": 1, "status": 307, "outcome": "completed",
                  "time": 0, "durationMs": 5 },
                { "type": "response", "id": 2, "status": 200, "responseHeaders": [] },
                { "type": "end", "id": 2, "status": 200, "outcome": "completed",
                  "time": 6, "durationMs": 4 },
            ]),
        );
        state.release_redirects(now + REDIRECT_WAIT * 2);
        assert_eq!(
            methods(&state),
            [
                "Network.requestWillBeSent",
                "Network.requestWillBeSent",
                "Network.responseReceived",
                "Network.loadingFinished"
            ]
        );
        assert!(
            events(&state)
                .iter()
                .all(|event| event["params"]["requestId"] == "1")
        );
    }

    #[test]
    fn replay_keeps_to_its_bounds_and_drops_whole_requests() {
        let mut state = HubState::default();
        let now = Instant::now();
        for id in 0..3u64 {
            state.apply(
                &json!({ "type": "request", "id": id, "kind": "sse", "method": "GET",
                         "url": "https://a.test/s", "time": 0 }),
                now,
            );
            state.apply(
                &json!({ "type": "response", "id": id, "status": 200, "responseHeaders": [] }),
                now,
            );
            for n in 0..MAX_LOG_EVENTS / 2 {
                state.apply(
                    &json!({ "type": "sse", "requestId": id, "data": n, "time": 1 }),
                    now,
                );
            }
            state.trim();
        }
        assert!(state.log.len() <= MAX_LOG_EVENTS);
        // What stays is whole: every kept event's request is still known.
        assert!(
            state
                .log
                .iter()
                .all(|(cdp, _)| state.chains.contains_key(cdp))
        );
        assert!(state.chains.contains_key("2"), "the newest request stays");
    }

    #[test]
    fn only_devtools_or_a_tool_may_connect() {
        assert!(allowed_origin(None));
        assert!(allowed_origin(Some("devtools://devtools")));
        assert!(!allowed_origin(Some("https://evil.example")));
        assert!(!allowed_origin(Some("null")));
    }

    #[test]
    fn a_3xx_nothing_followed_is_the_answer() {
        let mut state = HubState::default();
        let now = Instant::now();
        feed(
            &mut state,
            now,
            json!([
                { "type": "request", "id": 1, "kind": "fetch", "method": "GET",
                  "url": "https://a.test/x", "time": 0 },
                { "type": "response", "id": 1, "status": 301,
                  "responseHeaders": [["location", "/y"]] },
                { "type": "end", "id": 1, "status": 301, "outcome": "completed",
                  "time": 0, "durationMs": 5 },
            ]),
        );
        assert_eq!(methods(&state), ["Network.requestWillBeSent"]);
        state.release_redirects(now + REDIRECT_WAIT);
        assert_eq!(
            methods(&state),
            [
                "Network.requestWillBeSent",
                "Network.responseReceived",
                "Network.loadingFinished"
            ]
        );
    }

    #[test]
    fn event_stream_frames_arrive_live_and_failures_fail() {
        let mut state = HubState::default();
        feed(
            &mut state,
            Instant::now(),
            json!([
                { "type": "request", "id": 3, "kind": "sse", "method": "GET",
                  "url": "https://a.test/s", "time": 0 },
                { "type": "response", "id": 3, "status": 200,
                  "responseHeaders": [["content-type", "text/event-stream"]] },
                { "type": "sse", "requestId": 3, "event": "delta", "id": "e1",
                  "data": { "t": "hi" }, "time": 50 },
                { "type": "end", "id": 3, "status": 200, "outcome": "aborted",
                  "time": 0, "durationMs": 90 },
                // Seen only finished, from the backlog.
                { "type": "end", "id": 4, "past": true, "kind": "fetch", "method": "GET",
                  "url": "https://a.test/old", "time": 5, "status": 204,
                  "outcome": "completed", "durationMs": 3, "responseBytes": 0 },
            ]),
        );
        assert_eq!(
            methods(&state),
            [
                "Network.requestWillBeSent",
                "Network.responseReceived",
                "Network.eventSourceMessageReceived",
                "Network.loadingFailed",
                "Network.requestWillBeSent",
                "Network.responseReceived",
                "Network.loadingFinished"
            ]
        );
        let log = events(&state);
        let message = &log[2]["params"];
        assert_eq!(message["requestId"], "3");
        assert_eq!(message["eventName"], "delta");
        assert_eq!(message["eventId"], "e1");
        assert_eq!(message["data"], r#"{"t":"hi"}"#);
        assert_eq!(message["timestamp"], 0.05);
        assert_eq!(log[3]["params"]["canceled"], true);
        assert_eq!(log[3]["params"]["type"], "EventSource");
    }

    #[test]
    fn a_past_request_has_no_body_to_fetch() {
        let hub = Hub::new("ws://unused".into());
        hub.apply(
            &json!({ "type": "end", "id": 1, "past": true, "kind": "fetch", "method": "GET",
                     "url": "https://a.test/", "time": 0, "status": 200, "outcome": "completed" }),
            Instant::now(),
        );
        let reply = hub.answer(&json!({ "id": 1, "method": "Network.getResponseBody",
            "params": { "requestId": "1" } }));
        assert!(
            reply["error"]["message"]
                .as_str()
                .unwrap()
                .contains("before")
        );
        let unknown = hub.answer(&json!({ "id": 2, "method": "Network.getResponseBody",
            "params": { "requestId": "9" } }));
        assert!(unknown["error"].is_object());
    }

    #[test]
    fn the_devtools_browser_is_found_through_its_port_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("DevToolsActivePort");
        assert!(browser_endpoint(&file).is_none());
        std::fs::write(&file, "9222\n/devtools/browser/abc\n").unwrap();
        assert_eq!(
            browser_endpoint(&file).as_deref(),
            Some("ws://127.0.0.1:9222/devtools/browser/abc")
        );
        // Half written: the browser has not finished starting.
        std::fs::write(&file, "9222\n").unwrap();
        assert!(browser_endpoint(&file).is_none());
    }

    #[test]
    fn only_devtools_tabs_of_exited_runs_are_abandoned() {
        let live = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = live.local_addr().unwrap().port();
        let page = |port: u16| {
            format!(
                "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:{port}/lxdev&panel=network"
            )
        };
        assert!(!abandoned_devtools(&page(port)));
        drop(live);
        assert!(abandoned_devtools(&page(port)));
        assert!(!abandoned_devtools("https://example.com/"));
    }

    #[test]
    fn an_older_host_is_told_apart_from_a_failing_one() {
        let err = |code: &str, message: &str| CommandError {
            code: code.into(),
            message: message.into(),
            data: None,
        };
        assert!(predates_watch(&err("unknown_method", "x")));
        assert!(predates_watch(&err(
            "usage",
            "unknown network method session.network.watch"
        )));
        assert!(!predates_watch(&err("request_failed", "boom")));
    }

    #[test]
    fn bodies_read_as_devtools_text() {
        let record =
            json!({ "events": [{ "event": "delta", "data": { "t": "hi" } }, { "data": "end" }] });
        let (text, base64) = body_text(&record, "response").unwrap();
        assert!(!base64);
        assert_eq!(text, "event: delta\ndata: {\"t\":\"hi\"}\n\ndata: end\n\n");
        let empty = json!({ "responseBytes": 0 });
        assert_eq!(
            body_text(&empty, "response").unwrap(),
            (String::new(), false)
        );
        assert!(body_text(&json!({ "responseBytes": 9 }), "response").is_none());
        let binary = json!({ "responseBody": { "base64": "AAE=" } });
        assert_eq!(
            body_text(&binary, "response").unwrap(),
            ("AAE=".into(), true)
        );
        let json_body = json!({ "requestBody": { "q": 1 } });
        assert_eq!(body_text(&json_body, "request").unwrap().0, r#"{"q":1}"#);
    }
}
