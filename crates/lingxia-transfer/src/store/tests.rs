use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// What the fixture answers; `None` for a request the test wants left hanging.
type Respond = dyn Fn(&str, &HeaderMap) -> Option<Vec<u8>> + Send + Sync;

/// A local HTTP/1.1 origin that counts requests and closes each connection.
struct Origin {
    url: String,
    hits: Arc<AtomicUsize>,
    closed: Arc<tokio::sync::Notify>,
    task: tokio::task::JoinHandle<()>,
}

impl Origin {
    async fn start(
        respond: impl Fn(&str, &HeaderMap) -> Option<Vec<u8>> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(tokio::sync::Notify::new());
        let respond: Arc<Respond> = Arc::new(respond);
        let task = {
            let hits = hits.clone();
            let closed = closed.clone();
            tokio::spawn(async move {
                loop {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let hits = hits.clone();
                    let closed = closed.clone();
                    let respond = respond.clone();
                    tokio::spawn(async move {
                        let mut buffer = Vec::new();
                        let mut chunk = [0u8; 1024];
                        while !buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                            match socket.read(&mut chunk).await {
                                Ok(0) | Err(_) => return,
                                Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                            }
                        }
                        hits.fetch_add(1, Ordering::SeqCst);
                        let text = String::from_utf8_lossy(&buffer).to_string();
                        let mut lines = text.split("\r\n");
                        let path = lines
                            .next()
                            .and_then(|line| line.split(' ').nth(1))
                            .unwrap_or("/")
                            .to_string();
                        let mut headers = HeaderMap::new();
                        for line in lines {
                            if let Some((name, value)) = line.split_once(':')
                                && let (Ok(name), Ok(value)) = (
                                    header::HeaderName::from_bytes(name.trim().as_bytes()),
                                    header::HeaderValue::from_str(value.trim()),
                                )
                            {
                                headers.insert(name, value);
                            }
                        }
                        match respond(&path, &headers) {
                            Some(response) => {
                                let _ = socket.write_all(&response).await;
                                let _ = socket.shutdown().await;
                            }
                            None => {
                                // Hold the request until the client gives up.
                                let _ = socket.read(&mut chunk).await;
                                closed.notify_one();
                            }
                        }
                    });
                }
            })
        };
        Self {
            url,
            hits,
            closed,
            task,
        }
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

impl Drop for Origin {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn response(status: &str, headers: &[&str], body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for header in headers {
        out.push_str(header);
        out.push_str("\r\n");
    }
    out.push_str("\r\n");
    let mut out = out.into_bytes();
    out.extend_from_slice(body);
    out
}

fn request(origin: &Origin, path: &str) -> FetchRequest {
    let mut request = FetchRequest::new(format!("{}{path}", origin.url), "app");
    request.allow_http = true;
    request.timeout = Duration::from_secs(5);
    request
}

fn store(budget: u64) -> (tempfile_dir::Dir, Arc<Store>) {
    let dir = tempfile_dir::Dir::new();
    let store = Store::open(dir.path(), budget).unwrap();
    (dir, store)
}

/// A unique directory under the system temp dir, removed on drop.
mod tempfile_dir {
    pub struct Dir(std::path::PathBuf);
    impl Dir {
        pub fn new() -> Self {
            let path = std::env::temp_dir().join(format!("lx-store-{}", super::unique()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        pub fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[tokio::test]
async fn a_fresh_entry_is_served_without_a_request() {
    let origin =
        Origin::start(|_, _| Some(response("200 OK", &["Cache-Control: max-age=60"], b"fresh")))
            .await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    assert_eq!(
        store.fetch_bytes(request(&origin, "/a")).await.unwrap(),
        "fresh"
    );
    let mut sink = Vec::new();
    let meta = store
        .fetch_into(request(&origin, "/a"), &mut sink)
        .await
        .unwrap();
    assert_eq!(
        (sink.as_slice(), meta.source),
        (&b"fresh"[..], Source::Fresh)
    );
    assert_eq!(origin.hits(), 1);
}

#[tokio::test]
async fn a_stale_entry_revalidates_and_a_304_serves_the_stored_body() {
    let origin = Origin::start(|_, headers| {
        Some(
            if headers
                .get(header::IF_NONE_MATCH)
                .is_some_and(|v| v == "\"v1\"")
            {
                response(
                    "304 Not Modified",
                    &["ETag: \"v1\"", "Cache-Control: max-age=0"],
                    b"",
                )
            } else {
                response(
                    "200 OK",
                    &["ETag: \"v1\"", "Cache-Control: max-age=0"],
                    b"body",
                )
            },
        )
    })
    .await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    assert_eq!(
        store.fetch_bytes(request(&origin, "/a")).await.unwrap(),
        "body"
    );
    let mut sink = Vec::new();
    let meta = store
        .fetch_into(request(&origin, "/a"), &mut sink)
        .await
        .unwrap();
    assert_eq!(sink, b"body");
    assert_eq!(meta.source, Source::Revalidated);
    assert_eq!(origin.hits(), 2);
}

#[tokio::test]
async fn a_changed_resource_replaces_the_entry_on_revalidation() {
    let version = Arc::new(AtomicUsize::new(1));
    let served = version.clone();
    let origin = Origin::start(move |_, _| {
        let v = served.load(Ordering::SeqCst);
        Some(response(
            "200 OK",
            &[&format!("ETag: \"v{v}\""), "Cache-Control: max-age=60"],
            format!("v{v}").as_bytes(),
        ))
    })
    .await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    assert_eq!(
        store.fetch_bytes(request(&origin, "/a")).await.unwrap(),
        "v1"
    );
    version.store(2, Ordering::SeqCst);
    let mut revalidate = request(&origin, "/a");
    revalidate.cache = CacheMode::Revalidate;
    assert_eq!(store.fetch_bytes(revalidate).await.unwrap(), "v2");
    assert_eq!(
        store.fetch_bytes(request(&origin, "/a")).await.unwrap(),
        "v2"
    );
    assert_eq!(origin.hits(), 2);
}

#[tokio::test]
async fn the_least_recently_used_entries_go_first_and_never_the_one_just_written() {
    let origin = Origin::start(|path, _| {
        let body = if path == "/big" {
            vec![b'x'; 40]
        } else {
            vec![b'x'; 10]
        };
        Some(response("200 OK", &["Cache-Control: max-age=60"], &body))
    })
    .await;
    let (_dir, store) = store(25);
    for path in ["/a", "/b"] {
        store.fetch_bytes(request(&origin, path)).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1100)).await;
    }
    // Reading /a makes /b the oldest.
    store.fetch_bytes(request(&origin, "/a")).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1100)).await;
    store.fetch_bytes(request(&origin, "/c")).await.unwrap();
    assert_eq!(store.size().unwrap(), 20);
    assert_eq!(origin.hits(), 3);
    store.fetch_bytes(request(&origin, "/a")).await.unwrap();
    assert_eq!(origin.hits(), 3, "/a was kept");
    store.fetch_bytes(request(&origin, "/b")).await.unwrap();
    assert_eq!(origin.hits(), 4, "/b was evicted");
    // Larger than the whole budget: kept by its own write, everything older goes.
    store.fetch_bytes(request(&origin, "/big")).await.unwrap();
    assert_eq!(store.size().unwrap(), 40);
}

#[tokio::test]
async fn bodies_over_the_limit_fail_whether_declared_or_streamed() {
    let origin = Origin::start(|path, _| {
        Some(if path == "/declared" {
            response("200 OK", &[], &[0; 100])
        } else {
            // No Content-Length: the size is only known while streaming.
            let mut out = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
            out.extend_from_slice(&[0; 100]);
            out
        })
    })
    .await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    for path in ["/declared", "/streamed"] {
        let mut limited = request(&origin, path);
        limited.max_bytes = 50;
        let error = store.fetch_bytes(limited).await.unwrap_err();
        assert_eq!(error.kind, FetchErrorKind::TooLarge, "{path}");
    }
    assert_eq!(store.size().unwrap(), 0);
}

#[tokio::test]
async fn a_request_that_never_answers_times_out() {
    let origin = Origin::start(|_, _| None).await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    let mut slow = request(&origin, "/slow");
    slow.timeout = Duration::from_millis(300);
    let started = std::time::Instant::now();
    let error = store.fetch_bytes(slow).await.unwrap_err();
    assert_eq!(error.kind, FetchErrorKind::Timeout);
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn plain_http_needs_an_explicit_allow() {
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    let error = store
        .fetch_bytes(FetchRequest::new("http://example.invalid/a", "app"))
        .await
        .unwrap_err();
    assert_eq!(error.kind, FetchErrorKind::InsecureScheme);
}

#[tokio::test]
async fn identical_requests_share_one_transfer() {
    let origin = Origin::start(|_, _| {
        std::thread::sleep(Duration::from_millis(200));
        Some(response(
            "200 OK",
            &["Cache-Control: max-age=60"],
            b"shared",
        ))
    })
    .await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    let fetches: Vec<_> = (0..5)
        .map(|_| {
            let store = store.clone();
            let request = request(&origin, "/a");
            tokio::spawn(async move { store.fetch_bytes(request).await })
        })
        .collect();
    for fetch in fetches {
        assert_eq!(fetch.await.unwrap().unwrap(), "shared");
    }
    assert_eq!(origin.hits(), 1);
}

#[tokio::test]
async fn dropping_the_last_waiter_stops_the_transfer() {
    let origin = Origin::start(|_, _| None).await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    let fetch = {
        let store = store.clone();
        let request = request(&origin, "/hang");
        tokio::spawn(async move { store.fetch_bytes(request).await })
    };
    while origin.hits() == 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    fetch.abort();
    tokio::time::timeout(Duration::from_secs(3), origin.closed.notified())
        .await
        .expect("the connection closes once nobody waits");
    assert_eq!(store.size().unwrap(), 0);
}

#[tokio::test]
async fn a_stale_entry_serves_when_the_origin_is_gone_only_if_asked() {
    let origin = Origin::start(|_, _| {
        Some(response(
            "200 OK",
            &["ETag: \"v1\"", "Cache-Control: max-age=0"],
            b"kept",
        ))
    })
    .await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    let mut stale = request(&origin, "/a");
    store.fetch_bytes(stale.clone()).await.unwrap();
    drop(origin);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        store.fetch_bytes(stale.clone()).await.unwrap_err().kind,
        FetchErrorKind::Network
    );
    stale.stale_on_error = true;
    let mut sink = Vec::new();
    let meta = store.fetch_into(stale, &mut sink).await.unwrap();
    assert_eq!(
        (sink.as_slice(), meta.source),
        (&b"kept"[..], Source::StaleOnError)
    );
}

#[tokio::test]
async fn a_body_the_os_removed_is_a_miss_and_the_index_survives_corruption() {
    let origin =
        Origin::start(|_, _| Some(response("200 OK", &["Cache-Control: max-age=60"], b"body")))
            .await;
    let dir = tempfile_dir::Dir::new();
    let store = Store::open(dir.path(), STORE_BUDGET_BYTES).unwrap();
    store.fetch_bytes(request(&origin, "/a")).await.unwrap();
    std::fs::remove_dir_all(dir.path().join(LAYOUT)).unwrap();
    assert_eq!(
        store.fetch_bytes(request(&origin, "/a")).await.unwrap(),
        "body"
    );
    assert_eq!(origin.hits(), 2);
    drop(store);
    std::fs::write(dir.path().join("index.redb"), b"not a database").unwrap();
    let reopened = Store::open(dir.path(), STORE_BUDGET_BYTES).unwrap();
    assert_eq!(reopened.size().unwrap(), 0);
}

#[tokio::test]
async fn bypass_and_no_store_never_populate_the_store() {
    let origin = Origin::start(|path, _| {
        Some(if path == "/private" {
            response("200 OK", &["Cache-Control: no-store"], b"private")
        } else {
            response("200 OK", &["Cache-Control: max-age=60"], b"package")
        })
    })
    .await;
    let (dir, store) = store(STORE_BUDGET_BYTES);
    let target = dir.path().join("package.bin");
    let mut bypass = request(&origin, "/package");
    bypass.cache = CacheMode::Bypass;
    store.fetch_to_path(bypass, &target).await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"package");
    assert_eq!(
        store
            .fetch_bytes(request(&origin, "/private"))
            .await
            .unwrap(),
        "private"
    );
    assert_eq!(store.size().unwrap(), 0);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".part"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[tokio::test]
async fn partitions_never_share_bytes() {
    let origin =
        Origin::start(|_, _| Some(response("200 OK", &["Cache-Control: max-age=60"], b"body")))
            .await;
    let (_dir, store) = store(STORE_BUDGET_BYTES);
    store.fetch_bytes(request(&origin, "/a")).await.unwrap();
    let mut other = request(&origin, "/a");
    other.partition = "other".into();
    store.fetch_bytes(other).await.unwrap();
    assert_eq!(origin.hits(), 2);
}
