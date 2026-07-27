// Structured HTTP/1.1 capture over decrypted streams (MITM or plain HTTP).
//
// Consumer API:
//   let mut rx = proxy.session_receiver();           // Receiver<Arc<CapturedSession>>
//   while let Ok(s) = rx.recv().await {
//       if s.host.ends_with("openai.com") { ... }   // one-line filter
//       let json = serde_json::to_value(&*s)?;       // ready for LLM / UI
//   }

use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast;

// ── Session counter ───────────────────────────────────────────────────────

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
fn next_id() -> u64 {
    SESSION_COUNTER.fetch_add(1, Ordering::Relaxed)
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ── Public types ──────────────────────────────────────────────────────────

/// One complete HTTP request/response exchange.
///
/// Serialises directly to JSON — pass `serde_json::to_value(&session)` to
/// any agent or LLM without further transformation.
#[derive(Debug, Clone, Serialize)]
pub struct CapturedSession {
    pub id: u64,
    pub host: String,
    pub port: u16,
    /// `true` when the bytes were originally TLS-encrypted (HTTPS MITM).
    pub decrypted: bool,
    pub request: CapturedRequest,
    pub response: Option<CapturedResponse>,
    pub timing: SessionTiming,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: CapturedBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapturedResponse {
    pub status: u16,
    pub reason: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub body: CapturedBody,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CapturedBody {
    Text {
        content: String,
    },
    /// Body was binary or exceeded `MAX_BODY`.  `preview_hex` = first 32 bytes.
    Binary {
        size: usize,
        preview_hex: String,
    },
    Empty,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionTiming {
    pub request_start_ms: u64,
    pub response_start_ms: Option<u64>,
    pub response_end_ms: Option<u64>,
}

// ── Constants ─────────────────────────────────────────────────────────────

const MAX_HEADER: usize = 64 * 1024; // 64 KB — plenty for any HTTP header set
const MAX_BODY: usize = 1024 * 1024; // 1 MB  — cap captured body size
type Headers = Vec<(String, String)>;
type ParsedRequestHead = (String, String, String, Headers);
type ParsedResponseHead = (u16, String, String, Headers);

// ── Main entry point ──────────────────────────────────────────────────────

/// Drive HTTP/1.1 session parsing on two already-decrypted async streams.
///
/// `client` — stream facing the browser.
/// `server` — stream facing the upstream server.
///
/// Both streams are used bidirectionally: requests are forwarded from client
/// to server, responses from server to client, while bytes are captured for
/// structured emission.
pub async fn handle_http_sessions<C, S>(
    host: String,
    port: u16,
    decrypted: bool,
    mut client: C,
    mut server: S,
    tx: broadcast::Sender<Arc<CapturedSession>>,
) -> std::io::Result<()>
where
    C: AsyncRead + AsyncWrite + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        let t_req_start = now_ms();

        // ── REQUEST ───────────────────────────────────────────────────────

        let raw_req = match read_until_crlf2(&mut client, MAX_HEADER).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };

        let (method, path, version, req_headers) = parse_request_head(&raw_req)?;

        server.write_all(&raw_req).await?;

        let mut req_body_raw = Vec::new();
        let req_framing = body_framing(&req_headers, false)?;
        let req_body_total = forward_body(
            &mut client,
            &mut server,
            req_framing,
            &mut req_body_raw,
            MAX_BODY,
        )
        .await?;

        let request = CapturedRequest {
            method: method.clone(),
            path,
            version,
            headers: req_headers.clone(),
            body: make_body(req_body_raw, req_body_total),
        };

        // ── RESPONSE ──────────────────────────────────────────────────────

        let raw_resp = match read_until_crlf2(&mut server, MAX_HEADER).await {
            Ok(b) => b,
            Err(e) => return Err(e),
        };
        let t_resp_start = now_ms();

        let (status, reason, resp_version, resp_headers) = parse_response_head(&raw_resp)?;

        client.write_all(&raw_resp).await?;

        let mut resp_body_raw = Vec::new();
        // RFC 7230 §3.3: no body for 1xx, 204, 304 or HEAD responses.
        let (resp_body_total, response_close_delimited) =
            if matches!(status, 100..=199 | 204 | 304) || method == "HEAD" {
                (0, false)
            } else {
                let framing = body_framing(&resp_headers, true)?;
                let close_delimited = framing == BodyFraming::CloseDelimited;
                let total = forward_body(
                    &mut server,
                    &mut client,
                    framing,
                    &mut resp_body_raw,
                    MAX_BODY,
                )
                .await?;
                (total, close_delimited)
            };
        let t_resp_end = now_ms();

        let response = CapturedResponse {
            status,
            reason,
            version: resp_version.clone(),
            headers: resp_headers.clone(),
            body: make_body(resp_body_raw, resp_body_total),
        };

        // ── EMIT ──────────────────────────────────────────────────────────

        if tx.receiver_count() > 0 {
            let _ = tx.send(Arc::new(CapturedSession {
                id: next_id(),
                host: host.clone(),
                port,
                decrypted,
                request,
                response: Some(response),
                timing: SessionTiming {
                    request_start_ms: t_req_start,
                    response_start_ms: Some(t_resp_start),
                    response_end_ms: Some(t_resp_end),
                },
            }));
        }

        // ── KEEP-ALIVE CHECK ──────────────────────────────────────────────

        let req_close = header_value(&req_headers, "connection").eq_ignore_ascii_case("close");
        let resp_close = header_value(&resp_headers, "connection").eq_ignore_ascii_case("close");
        let http10 = resp_version == "HTTP/1.0";

        if req_close || resp_close || http10 || response_close_delimited {
            break;
        }
    }

    Ok(())
}

// ── HTTP head parsers ─────────────────────────────────────────────────────

fn parse_request_head(buf: &[u8]) -> std::io::Result<ParsedRequestHead> {
    let mut storage = [httparse::EMPTY_HEADER; 96];
    let mut req = httparse::Request::new(&mut storage);
    req.parse(buf)
        .map_err(|e| invalid_data(format!("request parse: {e:?}")))?;

    let method = req.method.unwrap_or("").to_string();
    let path = req.path.unwrap_or("").to_string();
    let version = if req.version == Some(1) {
        "HTTP/1.1"
    } else {
        "HTTP/1.0"
    }
    .to_string();
    let headers = collect_headers(req.headers);

    Ok((method, path, version, headers))
}

fn parse_response_head(buf: &[u8]) -> std::io::Result<ParsedResponseHead> {
    let mut storage = [httparse::EMPTY_HEADER; 96];
    let mut resp = httparse::Response::new(&mut storage);
    resp.parse(buf)
        .map_err(|e| invalid_data(format!("response parse: {e:?}")))?;

    let status = resp.code.unwrap_or(0);
    let reason = resp.reason.unwrap_or("").to_string();
    let version = if resp.version == Some(1) {
        "HTTP/1.1"
    } else {
        "HTTP/1.0"
    }
    .to_string();
    let headers = collect_headers(resp.headers);

    Ok((status, reason, version, headers))
}

fn collect_headers(raw: &[httparse::Header<'_>]) -> Headers {
    raw.iter()
        .filter(|h| !h.name.is_empty())
        .map(|h| {
            (
                h.name.to_string(),
                String::from_utf8_lossy(h.value).into_owned(),
            )
        })
        .collect()
}

// ── Body I/O ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyFraming {
    None,
    Chunked,
    ContentLength(usize),
    CloseDelimited,
}

fn body_framing(
    headers: &[(String, String)],
    allow_close_delimited: bool,
) -> std::io::Result<BodyFraming> {
    let transfer_encodings: Vec<&str> = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("transfer-encoding"))
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    let content_lengths: Vec<usize> = headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| invalid_data(format!("invalid Content-Length: {value:?}")))
        })
        .collect::<std::io::Result<_>>()?;

    if !transfer_encodings.is_empty() {
        if !content_lengths.is_empty() {
            return Err(invalid_data(
                "message contains both Transfer-Encoding and Content-Length",
            ));
        }
        if transfer_encodings
            .last()
            .is_some_and(|encoding| encoding.eq_ignore_ascii_case("chunked"))
        {
            return Ok(BodyFraming::Chunked);
        }
        return allow_close_delimited
            .then_some(BodyFraming::CloseDelimited)
            .ok_or_else(|| invalid_data("request Transfer-Encoding must end in chunked"));
    }

    if let Some(length) = content_lengths.first().copied() {
        if content_lengths.iter().any(|candidate| *candidate != length) {
            return Err(invalid_data("conflicting Content-Length values"));
        }
        return Ok(BodyFraming::ContentLength(length));
    }

    Ok(if allow_close_delimited {
        BodyFraming::CloseDelimited
    } else {
        BodyFraming::None
    })
}

/// Read the body declared by `framing` from `reader`, forward every byte to
/// `writer`, and accumulate up to `max_capture` bytes into `cap`.
/// Returns the total number of body bytes transferred.
async fn forward_body<R, W>(
    reader: &mut R,
    writer: &mut W,
    framing: BodyFraming,
    cap: &mut Vec<u8>,
    max_capture: usize,
) -> std::io::Result<usize>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    match framing {
        BodyFraming::None | BodyFraming::ContentLength(0) => Ok(0),
        BodyFraming::Chunked => forward_chunked(reader, writer, cap, max_capture).await,
        BodyFraming::ContentLength(length) => {
            forward_exact(reader, writer, length, cap, max_capture).await
        }
        BodyFraming::CloseDelimited => forward_to_eof(reader, writer, cap, max_capture).await,
    }
}

async fn forward_to_eof<R, W>(
    reader: &mut R,
    writer: &mut W,
    cap: &mut Vec<u8>,
    max_capture: usize,
) -> std::io::Result<usize>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = [0u8; 8192];
    let mut total = 0usize;
    loop {
        let read = reader.read(&mut buf).await?;
        if read == 0 {
            return Ok(total);
        }
        writer.write_all(&buf[..read]).await?;
        total = total
            .checked_add(read)
            .ok_or_else(|| invalid_data("HTTP body size overflow"))?;
        capture_bytes(cap, &buf[..read], max_capture);
    }
}

/// Read exactly `len` bytes, forwarding each chunk and accumulating into `cap`.
async fn forward_exact<R, W>(
    reader: &mut R,
    writer: &mut W,
    len: usize,
    cap: &mut Vec<u8>,
    max_capture: usize,
) -> std::io::Result<usize>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 8192.min(len)];
    let mut done = 0;

    while done < len {
        let want = (len - done).min(buf.len());
        let n = reader.read(&mut buf[..want]).await?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).await?;
        done += n;
        capture_bytes(cap, &buf[..n], max_capture);
    }

    Ok(done)
}

/// Read HTTP/1.1 chunked encoding, forward raw wire bytes, accumulate decoded
/// body data into `cap`.
async fn forward_chunked<R, W>(
    reader: &mut R,
    writer: &mut W,
    cap: &mut Vec<u8>,
    max_capture: usize,
) -> std::io::Result<usize>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut total = 0;

    loop {
        // Read chunk-size line (hex digits, optional extensions, terminated \r\n)
        let size_line = read_line(reader, 128).await?;
        writer.write_all(&size_line).await?;

        let hex = std::str::from_utf8(&size_line)
            .unwrap_or("")
            .trim()
            .split(';')
            .next()
            .unwrap_or("0")
            .trim();

        let chunk_size = usize::from_str_radix(hex, 16)
            .map_err(|_| invalid_data(format!("bad chunk size: {hex:?}")))?;

        if chunk_size == 0 {
            // trailing headers + final \r\n
            let trailer = read_until_crlf2(reader, 4096).await.unwrap_or_default();
            writer.write_all(&trailer).await?;
            break;
        }

        // Read chunk data
        total += forward_exact(reader, writer, chunk_size, cap, max_capture).await?;

        // Each chunk is followed by \r\n
        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf).await?;
        writer.write_all(&crlf).await?;
    }

    Ok(total)
}

// ── Low-level readers ─────────────────────────────────────────────────────

/// Read bytes until `\r\n\r\n` (inclusive).  Returns the full header block.
pub(crate) async fn read_until_crlf2<R: AsyncRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(512);
    let mut byte = [0u8; 1];

    loop {
        reader.read_exact(&mut byte).await?;
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            return Ok(buf);
        }
        if buf.len() >= max {
            return Err(invalid_data("HTTP headers too large"));
        }
    }
}

/// Read bytes until `\r\n` (inclusive), up to `max` bytes.
async fn read_line<R: AsyncRead + Unpin>(reader: &mut R, max: usize) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(32);
    let mut byte = [0u8; 1];

    loop {
        reader.read_exact(&mut byte).await?;
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n") {
            return Ok(buf);
        }
        if buf.len() >= max {
            return Err(invalid_data("chunk-size line too long"));
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> &'a str {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn capture_bytes(cap: &mut Vec<u8>, data: &[u8], max: usize) {
    if cap.len() < max {
        let room = max - cap.len();
        cap.extend_from_slice(&data[..data.len().min(room)]);
    }
}

fn make_body(data: Vec<u8>, total: usize) -> CapturedBody {
    if total == 0 {
        return CapturedBody::Empty;
    }

    // If we captured the whole body, try UTF-8.
    if data.len() == total {
        match String::from_utf8(data) {
            Ok(s) => return CapturedBody::Text { content: s },
            Err(e) => {
                let raw = e.into_bytes();
                let preview_hex = raw
                    .iter()
                    .take(32)
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                return CapturedBody::Binary {
                    size: total,
                    preview_hex,
                };
            }
        }
    }

    // Truncated capture.
    let preview_hex = data
        .iter()
        .take(32)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    CapturedBody::Binary {
        size: total,
        preview_hex,
    }
}

fn invalid_data(msg: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg.into())
}

#[cfg(test)]
mod tests {
    use super::{BodyFraming, body_framing, forward_body};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn headers(values: &[(&str, &str)]) -> Vec<(String, String)> {
        values
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn framing_rejects_invalid_or_conflicting_content_lengths() {
        assert!(body_framing(&headers(&[("Content-Length", "wat")]), true).is_err());
        assert!(
            body_framing(
                &headers(&[("Content-Length", "4"), ("Content-Length", "5")]),
                true,
            )
            .is_err()
        );
        assert_eq!(
            body_framing(&headers(&[("Content-Length", "4, 4")]), true).unwrap(),
            BodyFraming::ContentLength(4)
        );
    }

    #[test]
    fn only_responses_without_length_are_close_delimited() {
        assert_eq!(body_framing(&[], false).unwrap(), BodyFraming::None);
        assert_eq!(
            body_framing(&[], true).unwrap(),
            BodyFraming::CloseDelimited
        );
    }

    #[tokio::test]
    async fn close_delimited_body_is_forwarded_until_eof() {
        let (mut source_writer, mut source_reader) = tokio::io::duplex(64);
        source_writer.write_all(b"complete body").await.unwrap();
        source_writer.shutdown().await.unwrap();
        let (mut destination_reader, mut destination_writer) = tokio::io::duplex(64);
        let mut captured = Vec::new();

        let total = forward_body(
            &mut source_reader,
            &mut destination_writer,
            BodyFraming::CloseDelimited,
            &mut captured,
            1024,
        )
        .await
        .unwrap();
        destination_writer.shutdown().await.unwrap();
        let mut forwarded = Vec::new();
        destination_reader
            .read_to_end(&mut forwarded)
            .await
            .unwrap();

        assert_eq!(total, 13);
        assert_eq!(captured, b"complete body");
        assert_eq!(forwarded, b"complete body");
    }
}
