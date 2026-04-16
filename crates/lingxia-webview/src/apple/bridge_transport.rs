use crate::webview::WebTag;
use crate::{SystemPipeReader, WebViewError};
use std::collections::VecDeque;
use std::io::Write;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Condvar, Mutex};

pub(super) const APPLE_INTERNAL_SCHEME: &str = "lx-apple";
const APPLE_BRIDGE_DOWNSTREAM_HOST: &str = "bridge";
const APPLE_BRIDGE_DOWNSTREAM_PATH: &str = "/downstream";
const APPLE_BRIDGE_QUEUE_LIMIT: usize = 1024;

pub(super) fn is_bridge_downstream_request(request: &http::Request<Vec<u8>>) -> bool {
    let uri = request.uri();
    uri.scheme_str() == Some(APPLE_INTERNAL_SCHEME)
        && uri.authority().is_some_and(|authority| {
            authority
                .as_str()
                .eq_ignore_ascii_case(APPLE_BRIDGE_DOWNSTREAM_HOST)
        })
        && uri.path() == APPLE_BRIDGE_DOWNSTREAM_PATH
}

pub(super) fn bridge_downstream_cors_origin(request: &http::Request<Vec<u8>>) -> String {
    let Some(origin) = request
        .headers()
        .get(http::header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
    else {
        return "null".to_string();
    };

    if origin == "null" || origin.starts_with("lingxia://") || origin.starts_with("lx://") {
        origin.to_string()
    } else {
        "null".to_string()
    }
}

struct AppleBridgeConnection {
    id: u64,
    writer: UnixStream,
}

struct AppleBridgeTransportState {
    queue: VecDeque<Vec<u8>>,
    connection: Option<AppleBridgeConnection>,
    next_connection_id: u64,
    shutdown: bool,
}

pub(super) struct AppleBridgeTransport {
    webtag: WebTag,
    state: Mutex<AppleBridgeTransportState>,
    signal: Condvar,
}

impl AppleBridgeTransport {
    pub(super) fn new(webtag: WebTag) -> Arc<Self> {
        let transport = Arc::new(Self {
            webtag,
            state: Mutex::new(AppleBridgeTransportState {
                queue: VecDeque::new(),
                connection: None,
                next_connection_id: 0,
                shutdown: false,
            }),
            signal: Condvar::new(),
        });
        let worker = Arc::clone(&transport);
        std::thread::spawn(move || worker.run_writer_loop());
        transport
    }

    pub(super) fn connect_downstream(&self) -> Result<SystemPipeReader, WebViewError> {
        let (read_end, write_end) = UnixStream::pair().map_err(|e| {
            WebViewError::WebView(format!(
                "Failed to create Apple bridge downstream pipe: {e}"
            ))
        })?;
        let read_fd = read_end.into_raw_fd();
        let reader = unsafe { SystemPipeReader::from_raw_fd(read_fd) };

        let (replaced_existing, dropped_queued_frames) = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            guard.next_connection_id += 1;
            let replaced = guard.connection.is_some();
            let dropped = guard.queue.len();
            guard.queue.clear();
            let id = guard.next_connection_id;
            guard.connection = Some(AppleBridgeConnection {
                id,
                writer: write_end,
            });
            (replaced, dropped)
        };
        self.signal.notify_all();
        let dropped_suffix = if dropped_queued_frames > 0 {
            format!(" (dropped {} stale queued frame(s))", dropped_queued_frames)
        } else {
            String::new()
        };
        log::info!(
            "Apple bridge downstream connected webtag={}{}{}",
            self.webtag,
            if replaced_existing {
                " (replaced existing stream)"
            } else {
                ""
            },
            dropped_suffix
        );
        Ok(reader)
    }

    pub(super) fn enqueue_message(&self, message: &str) -> Result<(), WebViewError> {
        let mut frame = Vec::with_capacity(message.len() + 1);
        frame.extend_from_slice(message.as_bytes());
        frame.push(b'\n');

        let queued_len = {
            let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if guard.shutdown {
                return Err(WebViewError::WebView(format!(
                    "Apple bridge downstream is closed for {}",
                    self.webtag
                )));
            }
            if guard.queue.len() >= APPLE_BRIDGE_QUEUE_LIMIT {
                return Err(WebViewError::WebView(format!(
                    "Apple bridge downstream queue overflow for {} (limit={})",
                    self.webtag, APPLE_BRIDGE_QUEUE_LIMIT
                )));
            }
            guard.queue.push_back(frame);
            guard.queue.len()
        };
        if queued_len > (APPLE_BRIDGE_QUEUE_LIMIT / 2) {
            log::warn!(
                "Apple bridge downstream backlog webtag={} queued={}",
                self.webtag,
                queued_len
            );
        }
        self.signal.notify_one();
        Ok(())
    }

    pub(super) fn shutdown(&self) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.shutdown = true;
        guard.connection = None;
        self.signal.notify_all();
    }

    fn run_writer_loop(self: Arc<Self>) {
        loop {
            let (connection_id, mut writer, frame) = {
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                while !guard.shutdown && (guard.connection.is_none() || guard.queue.is_empty()) {
                    guard = self.signal.wait(guard).unwrap_or_else(|e| e.into_inner());
                }
                if guard.shutdown {
                    return;
                }
                let Some(frame) = guard.queue.pop_front() else {
                    continue;
                };
                let Some(connection) = guard.connection.as_ref() else {
                    guard.queue.push_front(frame);
                    continue;
                };
                let writer = match connection.writer.try_clone() {
                    Ok(writer) => writer,
                    Err(e) => {
                        log::warn!(
                            "Apple bridge downstream clone failed webtag={}: {}",
                            self.webtag,
                            e
                        );
                        guard.queue.push_front(frame);
                        guard.connection = None;
                        continue;
                    }
                };
                (connection.id, writer, frame)
            };

            if let Err(e) = writer.write_all(&frame) {
                log::debug!(
                    "Apple bridge downstream write failed webtag={}: {}",
                    self.webtag,
                    e
                );
                let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
                guard.queue.push_front(frame);
                if guard
                    .connection
                    .as_ref()
                    .is_some_and(|connection| connection.id == connection_id)
                {
                    guard.connection = None;
                }
            }
        }
    }
}
