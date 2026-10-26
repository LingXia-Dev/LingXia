//! The one HTTP fetch path LingXia uses for its own bytes, behind a
//! content-addressed on-disk cache.
//!
//! Only `fetch_bytes` and `fetch_into` with [`CacheMode::Default`] or
//! [`CacheMode::Revalidate`] populate the store; `fetch_to_path` writes nothing
//! to it under [`CacheMode::Bypass`], so large transfers never churn the LRU.
//! The store does not authorize: callers check their domain grants before a
//! lookup and pass the same policy as `guard` for the network request.
use bytes::Bytes;
use dashmap::DashMap;
use http::{HeaderMap, Request as HttpRequest, StatusCode, header};
use http_body_util::{BodyExt, Empty};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use ring::digest::{SHA256, digest};
use rong_rt::http::{self as host_http, HttpBody, NetworkAccessGuard, RequestOptions};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::watch;

/// Bytes a store keeps before evicting least recently used entries.
pub const STORE_BUDGET_BYTES: u64 = 256 * 1024 * 1024;
/// Freshness inferred from `Last-Modified` alone never exceeds this.
const HEURISTIC_FRESHNESS_CAP: Duration = Duration::from_secs(24 * 60 * 60);
const ENTRIES: TableDefinition<&str, &[u8]> = TableDefinition::new("entries");
const LAYOUT: &str = "v1";

/// How a request uses the store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CacheMode {
    /// Serve a fresh entry without a request; revalidate a stale one.
    #[default]
    Default,
    /// Always ask the origin, conditionally when the entry has validators.
    Revalidate,
    /// Neither read nor write the store.
    Bypass,
}

/// One fetch. `partition` keeps callers' bytes apart: an appid for an lxapp,
/// `host:<subsystem>` for bytes shared on purpose (registry icons).
#[derive(Clone)]
pub struct FetchRequest {
    pub url: String,
    pub partition: String,
    pub headers: Vec<(String, String)>,
    pub max_bytes: u64,
    /// Bounds the whole request, body included.
    pub timeout: Duration,
    pub allow_http: bool,
    /// Bounds the socket connect phase alone.
    pub connect_timeout: Option<Duration>,
    pub cache: CacheMode,
    /// Serve a stale entry when the origin cannot be reached.
    pub stale_on_error: bool,
    /// The caller's network policy, applied to the request itself.
    pub guard: Option<Arc<dyn NetworkAccessGuard>>,
}

impl fmt::Debug for FetchRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FetchRequest")
            .field("url", &self.url)
            .field("partition", &self.partition)
            .field("max_bytes", &self.max_bytes)
            .field("timeout", &self.timeout)
            .field("cache", &self.cache)
            .finish_non_exhaustive()
    }
}

impl FetchRequest {
    pub fn new(url: impl Into<String>, partition: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            partition: partition.into(),
            headers: Vec::new(),
            max_bytes: 32 * 1024 * 1024,
            timeout: Duration::from_secs(30),
            allow_http: false,
            connect_timeout: None,
            cache: CacheMode::Default,
            stale_on_error: false,
            guard: None,
        }
    }
}

/// Where a response came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Fresh,
    Revalidated,
    Network,
    StaleOnError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchMeta {
    pub status: u16,
    pub content_type: Option<String>,
    pub size: u64,
    pub source: Source,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchErrorKind {
    InvalidUrl,
    InsecureScheme,
    Status(u16),
    TooLarge,
    Timeout,
    Network,
    Io,
    Canceled,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct FetchError {
    pub kind: FetchErrorKind,
    pub message: String,
}

impl FetchError {
    fn new(kind: FetchErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
    fn io(error: std::io::Error) -> Self {
        Self::new(FetchErrorKind::Io, error.to_string())
    }
}

/// Index row for one cached body.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    url: String,
    etag: Option<String>,
    last_modified: Option<String>,
    /// Unix seconds until which the body is served without a request.
    fresh_until: u64,
    no_cache: bool,
    must_revalidate: bool,
    size: u64,
    content_type: Option<String>,
    last_access: u64,
}

/// A body that is not stored (`no-store`), removed once every waiter is done with it.
#[derive(Debug)]
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

type Outcome = Result<(Option<Arc<TempFile>>, FetchMeta), FetchError>;

/// A content-addressed HTTP cache in a caller-supplied directory. The OS may
/// clear the directory at any time: a row without its body is a miss, and an
/// unreadable index is rebuilt empty.
pub struct Store {
    dir: PathBuf,
    budget: u64,
    db: Database,
    inflight: DashMap<String, Arc<watch::Sender<Option<Outcome>>>>,
}

impl Store {
    pub fn open(dir: impl Into<PathBuf>, budget: u64) -> Result<Arc<Self>, FetchError> {
        let dir = dir.into();
        std::fs::create_dir_all(dir.join(LAYOUT)).map_err(FetchError::io)?;
        let index = dir.join("index.redb");
        let db = match Database::create(&index) {
            Ok(db) => db,
            Err(error) => {
                log::warn!("http store index unreadable, starting empty: {error}");
                let _ = std::fs::remove_file(&index);
                let _ = std::fs::remove_dir_all(dir.join(LAYOUT));
                std::fs::create_dir_all(dir.join(LAYOUT)).map_err(FetchError::io)?;
                Database::create(&index)
                    .map_err(|e| FetchError::new(FetchErrorKind::Io, e.to_string()))?
            }
        };
        let tx = db.begin_write().map_err(db_error)?;
        tx.open_table(ENTRIES).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(Arc::new(Self {
            dir,
            budget,
            db,
            inflight: DashMap::new(),
        }))
    }

    /// The response body, at most `max_bytes`.
    pub async fn fetch_bytes(self: &Arc<Self>, request: FetchRequest) -> Result<Bytes, FetchError> {
        let (body, _) = self.fetch_body(request).await?;
        let bytes = tokio::fs::read(body.path()).await.map_err(FetchError::io)?;
        Ok(Bytes::from(bytes))
    }

    /// Copy the response body into `writer`.
    pub async fn fetch_into<W: AsyncWrite + Unpin>(
        self: &Arc<Self>,
        request: FetchRequest,
        writer: &mut W,
    ) -> Result<FetchMeta, FetchError> {
        let (body, meta) = self.fetch_body(request).await?;
        let mut file = tokio::fs::File::open(body.path())
            .await
            .map_err(FetchError::io)?;
        tokio::io::copy(&mut file, writer)
            .await
            .map_err(FetchError::io)?;
        writer.flush().await.map_err(FetchError::io)?;
        Ok(meta)
    }

    /// Write the response body to `path`, atomically: a failed fetch leaves no file.
    /// With [`CacheMode::Bypass`] the body streams straight there and the store is untouched.
    pub async fn fetch_to_path(
        self: &Arc<Self>,
        request: FetchRequest,
        path: &Path,
    ) -> Result<FetchMeta, FetchError> {
        if request.cache == CacheMode::Bypass {
            return download_to_path(&request, path, false, None).await;
        }
        let staging = staging_path(path);
        let result = {
            match self.fetch_body(request).await {
                Ok((body, meta)) => tokio::fs::copy(body.path(), &staging)
                    .await
                    .map(|_| meta)
                    .map_err(FetchError::io),
                Err(error) => Err(error),
            }
        };
        match result {
            Ok(meta) => {
                tokio::fs::rename(&staging, path)
                    .await
                    .map_err(FetchError::io)?;
                Ok(meta)
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&staging).await;
                Err(error)
            }
        }
    }

    /// Evict down to `budget` bytes, least recently used first.
    pub fn trim(&self, budget: u64) -> Result<u64, FetchError> {
        self.evict(budget, None)
    }

    /// Remove every entry; returns the bytes removed.
    pub fn clear(&self) -> Result<u64, FetchError> {
        self.evict(0, None)
    }

    /// Bytes the index accounts for.
    pub fn size(&self) -> Result<u64, FetchError> {
        Ok(self.entries()?.iter().map(|(_, entry)| entry.size).sum())
    }

    async fn fetch_body(
        self: &Arc<Self>,
        request: FetchRequest,
    ) -> Result<(Body, FetchMeta), FetchError> {
        check_url(&request)?;
        let credentials = request.headers.iter().any(|(name, _)| {
            name.eq_ignore_ascii_case("cookie") || name.eq_ignore_ascii_case("authorization")
        });
        if request.cache == CacheMode::Bypass || credentials {
            let temp = self.dir.join(format!("bypass-{}.part", unique()));
            let (_, meta, _) = network(&request, None, Target::fresh(&temp))
                .await
                .inspect_err(|_| {
                    let _ = std::fs::remove_file(&temp);
                })?;
            return Ok((Body::Temp(temp), meta));
        }
        let key = key(&request.partition, &request.url);
        if request.cache == CacheMode::Default
            && let Some(entry) = self.lookup(&key)?
            && !entry.no_cache
            && entry.fresh_until > now()
        {
            self.touch(&key);
            return Ok((
                Body::Cached(self.body_path(&key)),
                meta_of(&entry, Source::Fresh),
            ));
        }
        // Waiters on one key share a transfer; it stops when the last one leaves.
        let flight = format!("{key}:{}", request.max_bytes);
        let mut receiver = match self.inflight.entry(flight.clone()) {
            dashmap::Entry::Occupied(pending) => pending.get().subscribe(),
            dashmap::Entry::Vacant(slot) => {
                let (sender, receiver) = watch::channel(None);
                let sender = Arc::new(sender);
                slot.insert(sender.clone());
                let store = self.clone();
                let request = request.clone();
                let key = key.clone();
                tokio::spawn(async move {
                    let outcome = tokio::select! {
                        outcome = store.refresh(&key, &request) => outcome,
                        _ = sender.closed() => Err(FetchError::new(FetchErrorKind::Canceled, "no waiters")),
                    };
                    store.inflight.remove(&flight);
                    let _ = sender.send(Some(outcome));
                });
                receiver
            }
        };
        let outcome = loop {
            if let Some(outcome) = receiver.borrow_and_update().clone() {
                break outcome;
            }
            if receiver.changed().await.is_err() {
                break Err(FetchError::new(FetchErrorKind::Canceled, "transfer ended"));
            }
        };
        let (temp, meta) = outcome?;
        Ok(match temp {
            Some(temp) => (Body::Shared(temp), meta),
            None => (Body::Cached(self.body_path(&key)), meta),
        })
    }

    /// Fetch or revalidate `key`; a stored body lands under its path, an unstorable one in a temp file.
    async fn refresh(&self, key: &str, request: &FetchRequest) -> Outcome {
        let cached = self.lookup(key)?;
        let temp = self.dir.join(format!("{key}-{}.part", unique()));
        let result = network(request, cached.as_ref(), Target::fresh(&temp)).await;
        match result {
            Ok((StatusCode::NOT_MODIFIED, _, headers)) => {
                let Some(mut entry) = cached else {
                    return Err(FetchError::new(
                        FetchErrorKind::Status(304),
                        "304 without a cached body",
                    ));
                };
                apply_freshness(&mut entry, &headers);
                entry.etag = header_string(&headers, header::ETAG).or(entry.etag);
                entry.last_modified =
                    header_string(&headers, header::LAST_MODIFIED).or(entry.last_modified);
                entry.last_access = now();
                self.put(key, &entry)?;
                Ok((None, meta_of(&entry, Source::Revalidated)))
            }
            Ok((status, meta, headers)) => {
                let directives = CacheControl::parse(&headers);
                if directives.no_store {
                    return Ok((Some(Arc::new(TempFile(temp))), meta));
                }
                let mut entry = Entry {
                    url: request.url.clone(),
                    etag: header_string(&headers, header::ETAG),
                    last_modified: header_string(&headers, header::LAST_MODIFIED),
                    fresh_until: 0,
                    no_cache: false,
                    must_revalidate: false,
                    size: meta.size,
                    content_type: meta.content_type.clone(),
                    last_access: now(),
                };
                apply_freshness(&mut entry, &headers);
                let body = self.body_path(key);
                if let Some(parent) = body.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(FetchError::io)?;
                }
                tokio::fs::rename(&temp, &body)
                    .await
                    .map_err(FetchError::io)?;
                self.put(key, &entry)?;
                self.evict(self.budget, Some(key))?;
                debug_assert!(status.is_success());
                Ok((None, meta))
            }
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp).await;
                match cached {
                    Some(entry)
                        if request.stale_on_error
                            && !entry.must_revalidate
                            && matches!(
                                error.kind,
                                FetchErrorKind::Network | FetchErrorKind::Timeout
                            )
                            && self.body_path(key).exists() =>
                    {
                        self.touch(key);
                        Ok((None, meta_of(&entry, Source::StaleOnError)))
                    }
                    _ => Err(error),
                }
            }
        }
    }

    fn body_path(&self, key: &str) -> PathBuf {
        self.dir
            .join(LAYOUT)
            .join(&key[..2])
            .join(format!("{key}.body"))
    }

    /// The row for `key`, if its body is still on disk.
    fn lookup(&self, key: &str) -> Result<Option<Entry>, FetchError> {
        let entry = {
            let tx = self.db.begin_read().map_err(db_error)?;
            let table = tx.open_table(ENTRIES).map_err(db_error)?;
            let Some(value) = table.get(key).map_err(db_error)? else {
                return Ok(None);
            };
            serde_json::from_slice::<Entry>(value.value()).ok()
        };
        match entry {
            Some(entry)
                if std::fs::metadata(self.body_path(key)).is_ok_and(|m| m.len() == entry.size) =>
            {
                Ok(Some(entry))
            }
            _ => {
                self.remove(key)?;
                Ok(None)
            }
        }
    }

    fn put(&self, key: &str, entry: &Entry) -> Result<(), FetchError> {
        let value = serde_json::to_vec(entry)
            .map_err(|e| FetchError::new(FetchErrorKind::Io, e.to_string()))?;
        let tx = self.db.begin_write().map_err(db_error)?;
        tx.open_table(ENTRIES)
            .map_err(db_error)?
            .insert(key, value.as_slice())
            .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }

    fn touch(&self, key: &str) {
        if let Ok(Some(mut entry)) = self.lookup(key) {
            entry.last_access = now();
            let _ = self.put(key, &entry);
        }
    }

    fn remove(&self, key: &str) -> Result<(), FetchError> {
        let _ = std::fs::remove_file(self.body_path(key));
        let tx = self.db.begin_write().map_err(db_error)?;
        tx.open_table(ENTRIES)
            .map_err(db_error)?
            .remove(key)
            .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }

    fn entries(&self) -> Result<Vec<(String, Entry)>, FetchError> {
        let tx = self.db.begin_read().map_err(db_error)?;
        let table = tx.open_table(ENTRIES).map_err(db_error)?;
        let mut entries = Vec::new();
        for row in table.iter().map_err(db_error)? {
            let (key, value) = row.map_err(db_error)?;
            if let Ok(entry) = serde_json::from_slice::<Entry>(value.value()) {
                entries.push((key.value().to_string(), entry));
            }
        }
        Ok(entries)
    }

    /// Remove least recently used entries until at most `budget` bytes remain;
    /// `keep` (the entry just written) is never removed by its own write.
    fn evict(&self, budget: u64, keep: Option<&str>) -> Result<u64, FetchError> {
        let mut entries = self.entries()?;
        let mut total: u64 = entries.iter().map(|(_, entry)| entry.size).sum();
        entries.sort_by_key(|(_, entry)| entry.last_access);
        let mut freed = 0;
        for (key, entry) in entries {
            if total <= budget {
                break;
            }
            if Some(key.as_str()) == keep {
                continue;
            }
            self.remove(&key)?;
            total -= entry.size;
            freed += entry.size;
        }
        Ok(freed)
    }
}

/// A body to read once: cached bodies stay, private ones go when read.
enum Body {
    Cached(PathBuf),
    Shared(Arc<TempFile>),
    Temp(PathBuf),
}

impl Body {
    fn path(&self) -> &Path {
        match self {
            Self::Cached(path) | Self::Temp(path) => path,
            Self::Shared(temp) => &temp.0,
        }
    }
}

impl Drop for Body {
    fn drop(&mut self) {
        if let Self::Temp(path) = self {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn check_url(request: &FetchRequest) -> Result<(), FetchError> {
    let uri: http::Uri = request
        .url
        .parse()
        .map_err(|_| FetchError::new(FetchErrorKind::InvalidUrl, request.url.clone()))?;
    match uri.scheme_str() {
        Some("https") if uri.host().is_some() => Ok(()),
        Some("http") if uri.host().is_some() && request.allow_http => Ok(()),
        Some("http") => Err(FetchError::new(
            FetchErrorKind::InsecureScheme,
            request.url.clone(),
        )),
        _ => Err(FetchError::new(
            FetchErrorKind::InvalidUrl,
            request.url.clone(),
        )),
    }
}

/// Bytes so far (resumed ones included) and the total when known.
pub type Progress<'a> = &'a (dyn Fn(u64, Option<u64>) + Send + Sync);

/// Where a response body goes.
struct Target<'a> {
    path: &'a Path,
    /// Bytes already in `path` to continue from with a `Range` request.
    resume_from: u64,
    /// Leave a partial body for a later resume instead of removing it.
    keep_partial: bool,
    progress: Option<Progress<'a>>,
}

impl<'a> Target<'a> {
    fn fresh(path: &'a Path) -> Self {
        Self {
            path,
            resume_from: 0,
            keep_partial: false,
            progress: None,
        }
    }
}

/// One GET into `target`, conditional on `cached`. A 304 writes nothing.
async fn network(
    request: &FetchRequest,
    cached: Option<&Entry>,
    target: Target<'_>,
) -> Result<(StatusCode, FetchMeta, HeaderMap), FetchError> {
    let mut builder = HttpRequest::builder()
        .uri(&request.url)
        .header(header::USER_AGENT, rong_rt::get_user_agent());
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    if target.resume_from > 0 {
        builder = builder.header(header::RANGE, format!("bytes={}-", target.resume_from));
    }
    if let Some(entry) = cached {
        if let Some(etag) = &entry.etag {
            builder = builder.header(header::IF_NONE_MATCH, etag);
        }
        if let Some(modified) = &entry.last_modified {
            builder = builder.header(header::IF_MODIFIED_SINCE, modified);
        }
    }
    let mut http_request = builder
        .body(
            Empty::<Bytes>::new()
                .map_err(|never| match never {})
                .boxed(),
        )
        .map_err(|e| FetchError::new(FetchErrorKind::InvalidUrl, e.to_string()))?;
    crate::tag_network_kind(&mut http_request, crate::TransferKind::Download);
    // Dropping this future drops the sender, which aborts the transport too.
    let (_abort, abort) = tokio::sync::oneshot::channel::<()>();
    // `timeout` below bounds the whole exchange, body included.
    let mut options = RequestOptions::new().with_abort(abort);
    if let Some(connect) = request.connect_timeout {
        options = options.with_connect_timeout(connect);
    }
    if let Some(guard) = &request.guard {
        options = options.with_network_access_guard(guard.clone());
    }
    let transfer = async {
        let response = host_http::send_stream(http_request, options)
            .await
            .map_err(|e| FetchError::new(FetchErrorKind::Network, e.to_string()))?;
        let status = response.status;
        if status == StatusCode::NOT_MODIFIED && cached.is_some() {
            let meta = FetchMeta {
                status: 304,
                content_type: None,
                size: 0,
                source: Source::Revalidated,
            };
            return Ok((status, meta, response.headers));
        }
        if !status.is_success() {
            return Err(FetchError::new(
                FetchErrorKind::Status(status.as_u16()),
                format!("HTTP {status}"),
            ));
        }
        // A 206 continues the partial body; a 200 to a range request starts over.
        let mut size = if status == StatusCode::PARTIAL_CONTENT {
            let start = header_string(&response.headers, header::CONTENT_RANGE).and_then(|range| {
                range
                    .strip_prefix("bytes ")?
                    .split('-')
                    .next()?
                    .parse::<u64>()
                    .ok()
            });
            if start != Some(target.resume_from) {
                return Err(FetchError::new(
                    FetchErrorKind::Status(206),
                    format!(
                        "resume range starts at {start:?}, expected {}",
                        target.resume_from
                    ),
                ));
            }
            target.resume_from
        } else {
            0
        };
        let declared = response
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok()?.parse::<u64>().ok())
            .map(|length| size + length);
        if declared.is_some_and(|length| length > request.max_bytes) {
            return Err(FetchError::new(
                FetchErrorKind::TooLarge,
                "declared length over the limit",
            ));
        }
        let mut file = if size > 0 {
            tokio::fs::OpenOptions::new()
                .append(true)
                .open(target.path)
                .await
        } else {
            tokio::fs::File::create(target.path).await
        }
        .map_err(FetchError::io)?;
        let mut chunks = match response.body {
            HttpBody::Empty => None,
            HttpBody::Small(bytes) => Some(Err(bytes)),
            HttpBody::Stream(chunks) => Some(Ok(chunks)),
        };
        loop {
            let chunk = match &mut chunks {
                None => break,
                Some(Err(bytes)) => {
                    let bytes = std::mem::take(bytes);
                    chunks = None;
                    bytes
                }
                Some(Ok(stream)) => match stream.recv().await {
                    Some(chunk) => {
                        chunk.map_err(|e| FetchError::new(FetchErrorKind::Network, e))?
                    }
                    None => break,
                },
            };
            size += chunk.len() as u64;
            if size > request.max_bytes {
                return Err(FetchError::new(
                    FetchErrorKind::TooLarge,
                    "body over the limit",
                ));
            }
            file.write_all(&chunk).await.map_err(FetchError::io)?;
            if let Some(progress) = target.progress {
                progress(size, declared);
            }
        }
        file.flush().await.map_err(FetchError::io)?;
        file.sync_all().await.map_err(FetchError::io)?;
        let meta = FetchMeta {
            status: status.as_u16(),
            content_type: header_string(&response.headers, header::CONTENT_TYPE),
            size,
            source: Source::Network,
        };
        Ok((status, meta, response.headers))
    };
    let result = crate::scope_network_owner(&request.partition, async {
        tokio::time::timeout(request.timeout, transfer)
            .await
            .unwrap_or_else(|_| {
                Err(FetchError::new(
                    FetchErrorKind::Timeout,
                    "request timed out",
                ))
            })
    })
    .await;
    if result.is_err() && !target.keep_partial {
        let _ = tokio::fs::remove_file(target.path).await;
    }
    result
}

/// The staging file a resumable download of `path` continues from.
pub fn partial_path(path: &Path) -> PathBuf {
    path.with_extension("part")
}

/// Bytes already staged for `path`.
pub fn partial_len(path: &Path) -> u64 {
    std::fs::metadata(partial_path(path))
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .unwrap_or(0)
}

/// Download straight to `path`, never through a store, staging in
/// [`partial_path`]: with `resume`, a staged partial body continues with a `Range`
/// request and a failure keeps it for the next call. `path` appears only complete.
pub async fn download_to_path(
    request: &FetchRequest,
    path: &Path,
    resume: bool,
    progress: Option<Progress<'_>>,
) -> Result<FetchMeta, FetchError> {
    check_url(request)?;
    let staging = partial_path(path);
    let resume_from = if resume { partial_len(path) } else { 0 };
    let target = Target {
        path: &staging,
        resume_from,
        keep_partial: resume,
        progress,
    };
    let (_, meta, _) = network(request, None, target).await?;
    tokio::fs::rename(&staging, path)
        .await
        .map_err(FetchError::io)?;
    Ok(meta)
}

#[derive(Default)]
struct CacheControl {
    no_store: bool,
    no_cache: bool,
    must_revalidate: bool,
    max_age: Option<u64>,
}

impl CacheControl {
    fn parse(headers: &HeaderMap) -> Self {
        let mut directives = Self::default();
        for value in headers.get_all(header::CACHE_CONTROL) {
            let Ok(value) = value.to_str() else { continue };
            for directive in value.split(',').map(|d| d.trim().to_ascii_lowercase()) {
                match directive.split_once('=') {
                    Some(("max-age", seconds)) => {
                        directives.max_age = seconds.trim_matches('"').parse().ok();
                    }
                    _ if directive == "no-store" => directives.no_store = true,
                    _ if directive == "no-cache" => directives.no_cache = true,
                    _ if directive == "must-revalidate" => directives.must_revalidate = true,
                    _ => {}
                }
            }
        }
        directives
    }
}

/// `max-age`, else `Expires`, else a tenth of the time since `Last-Modified` (capped).
fn apply_freshness(entry: &mut Entry, headers: &HeaderMap) {
    let directives = CacheControl::parse(headers);
    let date = header_time(headers, header::DATE).unwrap_or_else(SystemTime::now);
    let lifetime = if let Some(seconds) = directives.max_age {
        Duration::from_secs(seconds)
    } else if let Some(expires) = header_time(headers, header::EXPIRES) {
        expires.duration_since(date).unwrap_or_default()
    } else if let Some(modified) = header_time(headers, header::LAST_MODIFIED).or_else(|| {
        entry
            .last_modified
            .as_deref()
            .and_then(|v| httpdate::parse_http_date(v).ok())
    }) {
        (date.duration_since(modified).unwrap_or_default() / 10).min(HEURISTIC_FRESHNESS_CAP)
    } else {
        Duration::ZERO
    };
    entry.no_cache = directives.no_cache;
    entry.must_revalidate = directives.must_revalidate;
    entry.fresh_until = now() + lifetime.as_secs();
}

fn header_string(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers.get(name)?.to_str().ok().map(str::to_string)
}

fn header_time(headers: &HeaderMap, name: header::HeaderName) -> Option<SystemTime> {
    httpdate::parse_http_date(headers.get(name)?.to_str().ok()?).ok()
}

fn meta_of(entry: &Entry, source: Source) -> FetchMeta {
    FetchMeta {
        status: 200,
        content_type: entry.content_type.clone(),
        size: entry.size,
        source,
    }
}

/// Partitions never share keys: `partition` and the URL are hashed together.
fn key(partition: &str, url: &str) -> String {
    let input = format!("{partition}\0{url}");
    digest(&SHA256, input.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn staging_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.part", unique()));
    path.with_file_name(name)
}

fn unique() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn db_error(error: impl fmt::Display) -> FetchError {
    FetchError::new(FetchErrorKind::Io, format!("http store index: {error}"))
}

static SHARED: std::sync::OnceLock<Arc<Store>> = std::sync::OnceLock::new();

/// Install the host's store; the first call wins.
pub fn install(store: Arc<Store>) {
    let _ = SHARED.set(store);
}

/// The host's store, once installed.
pub fn shared() -> Option<Arc<Store>> {
    SHARED.get().cloned()
}

#[cfg(test)]
#[path = "store/tests.rs"]
mod tests;
