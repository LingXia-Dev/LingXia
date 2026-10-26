//! Resumable, retrying transfer of update packages (host app and lxapp).

use lingxia_transfer::store::{self, FetchErrorKind, FetchRequest, Progress};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

/// TCP/TLS connect cap. Long enough for congested store DNS, short of the OS default.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// Whole-transfer ceiling for one attempt, body included.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60 * 60);
/// Packages are bounded only by what a store serves; the checksum guards content.
const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// First attempt plus two retries.
const ATTEMPTS: u32 = 3;
const RETRY_BACKOFF: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(5)];
/// Network owner for update traffic.
const OWNER: &str = "host:update";

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

/// Download `url` to `dest`, resuming from its staged `.part`, and verify
/// `checksum_sha256` when non-empty. The transfer never goes through the HTTP
/// store. On failure `dest` is removed; the `.part` survives only when it can
/// still be resumed. `progress` gets bytes on disk, resumed ones included.
///
/// Dropping the returned future aborts the transfer.
pub async fn download_package(
    url: &str,
    dest: &Path,
    checksum_sha256: &str,
    progress: Option<Progress<'_>>,
) -> Result<(), PackageDownloadError> {
    let mut request = FetchRequest::new(url, OWNER);
    request.max_bytes = MAX_BYTES;
    request.timeout = REQUEST_TIMEOUT;
    request.connect_timeout = Some(CONNECT_TIMEOUT);
    // Update URLs come from the host's own provider; the checksum guards the bytes.
    request.allow_http = true;
    request.cache = store::CacheMode::Bypass;
    let mut attempt = 0;
    loop {
        attempt += 1;
        let resumed = partial_download_len(dest);
        let retries_left = attempt < ATTEMPTS;

        let error = match store::download_to_path(&request, dest, true, progress).await {
            Ok(_) => match verify_sha256(dest, checksum_sha256) {
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
                match classify(error.kind) {
                    Failure::Transient if retries_left => {}
                    // The `.part` itself may be what the server rejects.
                    Failure::Permanent if resumed > 0 && retries_left => {
                        let _ = fs::remove_file(store::partial_path(dest));
                    }
                    _ => {
                        return Err(PackageDownloadError::Transfer(format!(
                            "download failed: {error}"
                        )));
                    }
                }
                error.to_string()
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
    store::partial_len(dest)
}

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Transient,
    Permanent,
}

/// Only network-level failures and overloaded servers are worth retrying.
fn classify(kind: FetchErrorKind) -> Failure {
    match kind {
        FetchErrorKind::Network | FetchErrorKind::Timeout => Failure::Transient,
        FetchErrorKind::Status(408 | 429 | 500..=599) => Failure::Transient,
        _ => Failure::Permanent,
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
        download_package(&url, &dest, &expected, None)
            .await
            .unwrap();

        let start = server.join().unwrap();
        assert!(start > 0 && start <= CUT);
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
        let download = download_package(&url, &dest, "", None);
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
        for kind in [
            FetchErrorKind::Network,
            FetchErrorKind::Timeout,
            FetchErrorKind::Status(503),
            FetchErrorKind::Status(429),
        ] {
            assert_eq!(classify(kind), Failure::Transient, "{kind:?}");
        }
        for kind in [
            FetchErrorKind::Status(404),
            FetchErrorKind::Status(403),
            FetchErrorKind::Status(206),
            FetchErrorKind::TooLarge,
            FetchErrorKind::Io,
            FetchErrorKind::InvalidUrl,
        ] {
            assert_eq!(classify(kind), Failure::Permanent, "{kind:?}");
        }
    }
}
