//! Resumable, retrying transfer of update packages (host app and lxapp).

use rong_rt::download::{self as service_executor, BodySink};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// TCP/TLS connect cap. Long enough for congested store DNS, short of the OS default.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Whole-transfer ceiling. rong_rt's default request timeout (60s) bounds the
/// body too, so a slow link would fail while bytes are still arriving. rong_rt
/// still aborts a socket that delivers nothing for 120s.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// First attempt plus two retries.
const ATTEMPTS: u32 = 3;
const RETRY_BACKOFF: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(5)];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageDownloadError {
    /// The transfer finished but the bytes do not match the expected sha256.
    Checksum(String),
    Transfer(String),
}

impl fmt::Display for PackageDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Checksum(message) | Self::Transfer(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for PackageDownloadError {}

/// Download `url` to `dest`, resuming from `dest.part`, and verify `checksum_sha256`
/// when non-empty. On failure `dest` is removed; `.part` survives only when it can
/// still be resumed. `make_sink` receives the byte count already on disk.
///
/// Dropping the returned future aborts the transfer. Otherwise a cancelled open
/// could leave a task appending to `.part` for up to an hour while a retry
/// appends to the same file.
pub async fn download_package(
    url: &str,
    dest: &Path,
    checksum_sha256: &str,
    mut make_sink: impl FnMut(u64) -> Option<Box<dyn BodySink>>,
) -> Result<(), PackageDownloadError> {
    let part = partial_path(dest);
    let mut attempt = 0;
    loop {
        attempt += 1;
        let resumed = partial_download_len(dest);
        let retries_left = attempt < ATTEMPTS;

        let mut options = service_executor::DownloadOptions::new(url, dest.to_path_buf())
            .with_resume()
            .with_connect_timeout(CONNECT_TIMEOUT)
            .with_request_timeout(REQUEST_TIMEOUT);
        if let Some(sink) = make_sink(resumed) {
            options = options.with_sink(sink);
        }
        // rong_rt aborts when the sender is dropped, so holding it here ties the
        // transfer to this future.
        let (_abort_tx, abort_rx) = tokio::sync::oneshot::channel::<()>();
        let result = service_executor::spawn_download(options, Some(abort_rx))
            .map_err(|error| {
                PackageDownloadError::Transfer(format!("failed to start download: {error}"))
            })?
            .await
            .map_err(|_| PackageDownloadError::Transfer("download task cancelled".to_string()))?;

        let error = match result {
            Ok(()) => match verify_sha256(dest, checksum_sha256) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let _ = fs::remove_file(dest);
                    // A resumed file may splice stale bytes; one fresh transfer can fix it.
                    if resumed == 0 || !retries_left {
                        return Err(error);
                    }
                    error.to_string()
                }
            },
            Err(error) => {
                let _ = fs::remove_file(dest);
                match classify(&error) {
                    Failure::Transient if retries_left => {}
                    // The `.part` itself may be what the server rejects.
                    Failure::Permanent if resumed > 0 && retries_left => {
                        let _ = fs::remove_file(&part);
                    }
                    _ => {
                        return Err(PackageDownloadError::Transfer(format!(
                            "download failed: {error}"
                        )));
                    }
                }
                error
            }
        };

        log::warn!("Update package download attempt {attempt}/{ATTEMPTS} failed: {error}");
        let backoff = RETRY_BACKOFF[(attempt as usize - 1).min(RETRY_BACKOFF.len() - 1)];
        // Sleep on rong_rt's runtime: callers may poll from a runtime without timers.
        let _ = rong_rt::RongExecutor::global()
            .spawn(tokio::time::sleep(backoff))
            .await;
    }
}

/// Bytes already on disk for an interrupted download of `dest`.
pub fn partial_download_len(dest: &Path) -> u64 {
    fs::metadata(partial_path(dest))
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .unwrap_or(0)
}

/// Must match rong_rt's resume file naming.
fn partial_path(dest: &Path) -> PathBuf {
    dest.with_extension("part")
}

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Transient,
    Permanent,
}

/// rong_rt reports failures as strings; only network-level ones are worth retrying.
fn classify(error: &str) -> Failure {
    if let Some(status) = error.strip_prefix("http status: ") {
        let code = status.get(..3).and_then(|code| code.parse::<u16>().ok());
        return match code {
            Some(408 | 429) | Some(500..=599) => Failure::Transient,
            _ => Failure::Permanent,
        };
    }
    const TRANSIENT: [&str; 5] = [
        "request failed",
        "request timeout",
        "read timeout",
        "read frame",
        "stream task",
    ];
    if TRANSIENT.iter().any(|prefix| error.starts_with(prefix)) {
        Failure::Transient
    } else {
        Failure::Permanent
    }
}

fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), PackageDownloadError> {
    if expected_hex.is_empty() {
        return Ok(());
    }
    let actual = sha256_hex(path).map_err(|error| {
        PackageDownloadError::Transfer(format!("failed to hash download: {error}"))
    })?;
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(PackageDownloadError::Checksum(format!(
            "checksum mismatch: expected {expected_hex}, got {actual}"
        )))
    }
}

pub(crate) fn sha256_hex(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let mut hex = String::with_capacity(64);
    for b in hasher.finalize() {
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    // rong_rt buffers small bodies whole and coalesces 512 KiB before writing
    // `.part`, so the cut must leave at least one coalesced chunk on disk.
    const LEN: usize = 3 * 1024 * 1024;
    const CUT: usize = 2 * 1024 * 1024;

    fn body() -> Vec<u8> {
        (0..LEN).map(|i| (i % 251) as u8).collect()
    }

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut byte = [0u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap() == 0 {
                break;
            }
            request.push(byte[0]);
        }
        String::from_utf8(request).unwrap().to_ascii_lowercase()
    }

    #[tokio::test]
    async fn dropped_transfer_resumes_and_verifies() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pkg.zip", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let body = body();
            let (mut first, _) = listener.accept().unwrap();
            read_request(&mut first);
            write!(first, "HTTP/1.1 200 OK\r\nContent-Length: {LEN}\r\n\r\n").unwrap();
            first.write_all(&body[..CUT]).unwrap();
            drop(first);

            let (mut second, _) = listener.accept().unwrap();
            let request = read_request(&mut second);
            let start: usize = request
                .split("range: bytes=")
                .nth(1)
                .and_then(|rest| rest.split('-').next())
                .and_then(|start| start.parse().ok())
                .expect("second attempt must resume");
            write!(
                second,
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{LEN}\r\nContent-Length: {}\r\n\r\n",
                LEN - 1,
                LEN - start
            )
            .unwrap();
            second.write_all(&body[start..]).unwrap();
            start
        });

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("pkg.zip");
        let expected = {
            use sha2::{Digest, Sha256};
            Sha256::digest(body())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        let mut resumed_from = Vec::new();
        download_package(&url, &dest, &expected, |resumed| {
            resumed_from.push(resumed);
            None
        })
        .await
        .unwrap();

        let start = server.join().unwrap();
        assert!(start > 0 && start <= CUT);
        assert_eq!(resumed_from, [0, start as u64]);
        assert_eq!(fs::read(&dest).unwrap(), body());
        assert!(!dest.with_extension("part").exists());
    }

    #[tokio::test]
    async fn dropping_the_future_aborts_the_transfer() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/pkg.zip", listener.local_addr().unwrap());
        let (closed_tx, closed_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {LEN}\r\n\r\n").unwrap();
            let _ = stream.write_all(&body()[..64 * 1024]);
            // The client closes the socket once the download future is dropped.
            let mut probe = [0u8; 1];
            let closed = stream.read(&mut probe).map_or(true, |n| n == 0);
            let _ = closed_tx.send(closed);
        });

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("pkg.zip");
        let download = download_package(&url, &dest, "", |_| None);
        let aborted = tokio::time::timeout(Duration::from_millis(500), download).await;
        assert!(aborted.is_err(), "the transfer must still be in flight");

        // Well under rong_rt's 120s idle read timeout, which would also close it.
        let closed = closed_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("server socket should close promptly after the drop");
        assert!(closed);
        assert!(!dest.exists());
    }

    #[test]
    fn partial_download_len_reads_the_resume_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("app_1.2.3_package.apk");
        assert_eq!(partial_download_len(&dest), 0);

        fs::write(dest.with_extension("part"), b"partial").unwrap();
        assert_eq!(partial_download_len(&dest), b"partial".len() as u64);
    }

    #[test]
    fn only_network_failures_are_transient() {
        for error in [
            "request failed: client error (Connect)",
            "request timeout",
            "read timeout",
            "read frame: connection reset",
            "http status: 503 Service Unavailable",
            "http status: 429 Too Many Requests",
        ] {
            assert_eq!(classify(error), Failure::Transient, "{error}");
        }
        for error in [
            "http status: 404 Not Found",
            "http status: 403 Forbidden",
            "resume content range starts at 0, expected 10",
            "range not satisfiable; stale `.part` discarded",
            "write chunk: No space left on device",
            "too many redirects",
        ] {
            assert_eq!(classify(error), Failure::Permanent, "{error}");
        }
    }
}
