//! Embedded `<webview>` document lifecycle for GPU pages across Android, Apple, and HarmonyOS.
#[cfg(target_os = "android")]
use lingxia_view_android::lingxia_view;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use lingxia_view_apple::lingxia_view;
#[cfg(target_env = "ohos")]
use lingxia_view_harmony::lingxia_view;

use super::{Token, page_of};
use lingxia_view::scene::webview::{WebViewInbound, WebViewMount};
use lxapp::{EmbeddedDocumentOptions, EmbeddedErrorCode, OpenEmbeddedError};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

static NEXT_DOC_SEQ: AtomicU64 = AtomicU64::new(1);

struct Entry {
    doc_id: String,
    revision: u64,
    src: String,
    opened: bool,
    ready: bool,
    mount: WebViewMount,
}

fn strip_fragment(s: &str) -> &str {
    s.split('#').next().unwrap_or(s)
}

#[derive(Default)]
struct PageWebViews {
    entries: BTreeMap<String, Entry>,
    order: Vec<String>,
}

impl PageWebViews {
    fn current_mounts(&self) -> Vec<WebViewMount> {
        self.order
            .iter()
            .filter_map(|id| self.entries.get(id).map(|e| e.mount.clone()))
            .collect()
    }
}

static WEB_REGISTRY: LazyLock<Mutex<HashMap<Token, PageWebViews>>> =
    LazyLock::new(Default::default);

fn web_registry() -> MutexGuard<'static, HashMap<Token, PageWebViews>> {
    WEB_REGISTRY.lock().unwrap_or_else(|p| p.into_inner())
}

// Monotonic child document suffix: ArkWeb keys native controllers globally by `webTag`, and
// re-entering the same page path reuses the parent page `webtag` while the outgoing `Web` node
// is still tearing down asynchronously.
fn next_doc_id(web_id: &str) -> String {
    let seq = NEXT_DOC_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{web_id}-{seq}")
}

fn doc_id_of(token: Token, web_id: &str) -> Option<String> {
    web_registry()
        .get(&token)
        .and_then(|p| p.entries.get(web_id))
        .map(|e| e.doc_id.clone())
}

pub(crate) fn sync_webviews(
    token: Token,
    mounts: &[WebViewMount],
    deliver: fn(Token, String, WebViewInbound),
    ui_sync: fn(Token, &[WebViewMount]),
) {
    let mut removed = Vec::new();
    let mut actions: Vec<(String, String, u64, String, bool)> = Vec::new();
    {
        let mut reg = web_registry();
        if mounts.is_empty() && !reg.contains_key(&token) {
            return;
        }
        let page_state = reg.entry(token).or_default();
        let next_order: Vec<String> = mounts.iter().map(|m| m.web_id.clone()).collect();
        let stale: Vec<String> = page_state
            .entries
            .keys()
            .filter(|id| !next_order.contains(id))
            .cloned()
            .collect();
        for id in stale {
            if let Some(old) = page_state.entries.remove(&id) {
                removed.push(old.doc_id);
            }
        }
        page_state.order = next_order;
        for mount in mounts {
            match page_state.entries.get_mut(&mount.web_id) {
                Some(entry) => {
                    entry.mount = mount.clone();
                    if entry.src != mount.src {
                        if strip_fragment(&entry.src) != strip_fragment(&mount.src) {
                            entry.ready = false;
                        }
                        entry.revision += 1;
                        entry.src = mount.src.clone();
                        actions.push((
                            mount.web_id.clone(),
                            entry.doc_id.clone(),
                            entry.revision,
                            mount.src.clone(),
                            entry.opened,
                        ));
                    }
                }
                None => {
                    let doc_id = next_doc_id(&mount.web_id);
                    page_state.entries.insert(
                        mount.web_id.clone(),
                        Entry {
                            doc_id: doc_id.clone(),
                            revision: 1,
                            src: mount.src.clone(),
                            opened: false,
                            ready: false,
                            mount: mount.clone(),
                        },
                    );
                    actions.push((mount.web_id.clone(), doc_id, 1, mount.src.clone(), false));
                }
            }
        }
        if page_state.entries.is_empty() {
            reg.remove(&token);
        }
    }

    ui_sync(token, mounts);

    if !removed.is_empty() {
        std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
            if let Some(page) = page_of(token) {
                for doc_id in removed {
                    page.close_embedded_document(&doc_id);
                }
            }
        }));
    }

    for (web_id, doc_id, revision, src, was_opened) in actions {
        std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
            reconcile_one(
                token, web_id, doc_id, revision, src, was_opened, deliver, ui_sync,
            )
            .await;
        }));
    }
}

#[allow(clippy::too_many_arguments)]
async fn reconcile_one(
    token: Token,
    web_id: String,
    doc_id: String,
    revision: u64,
    src: String,
    was_opened: bool,
    deliver: fn(Token, String, WebViewInbound),
    ui_sync: fn(Token, &[WebViewMount]),
) {
    let Some(page) = page_of(token) else {
        return;
    };
    let still_current = web_registry()
        .get(&token)
        .and_then(|p| p.entries.get(&web_id))
        .is_some_and(|e| e.doc_id == doc_id && e.revision == revision);
    if !still_current {
        return;
    }

    if was_opened && let Some(doc) = page.embedded_document(&doc_id) {
        if let Err(err) = doc.set_source(&src) {
            let mounts = {
                let mut reg = web_registry();
                if let Some(entry) = reg
                    .get_mut(&token)
                    .and_then(|p| p.entries.get_mut(&web_id))
                    .filter(|e| e.doc_id == doc_id)
                {
                    entry.ready = false;
                }
                reg.get(&token)
                    .map(PageWebViews::current_mounts)
                    .unwrap_or_default()
            };
            deliver(token, web_id.clone(), WebViewInbound::Ready(false));
            deliver(
                token,
                web_id,
                WebViewInbound::Error {
                    code: err.code.as_str().to_owned(),
                    message: err.message,
                },
            );
            if !mounts.is_empty() {
                ui_sync(token, &mounts);
            }
        }
        return;
    }

    let id_load = web_id.clone();
    let doc_load = doc_id.clone();
    let id_msg = web_id.clone();
    let id_blk = web_id.clone();
    let id_err = web_id.clone();
    let doc_err = doc_id.clone();
    let id_fs = web_id.clone();

    let options = EmbeddedDocumentOptions {
        hosts: None,
        on_load: Some(Arc::new(move |url, title| {
            let mounts = {
                let mut reg = web_registry();
                if let Some(entry) = reg
                    .get_mut(&token)
                    .and_then(|p| p.entries.get_mut(&id_load))
                    .filter(|e| e.doc_id == doc_load)
                {
                    entry.ready = true;
                }
                reg.get(&token)
                    .map(PageWebViews::current_mounts)
                    .unwrap_or_default()
            };
            deliver(token, id_load.clone(), WebViewInbound::Ready(true));
            deliver(token, id_load.clone(), WebViewInbound::Load { url, title });
            if !mounts.is_empty() {
                ui_sync(token, &mounts);
            }
        })),
        on_message: Some(Arc::new(move |msg| {
            deliver(
                token,
                id_msg.clone(),
                WebViewInbound::Message {
                    json: msg.json,
                    origin: msg.origin,
                    queued: msg.queued,
                },
            );
        })),
        on_blocked: Some(Arc::new(move |blocked| {
            deliver(
                token,
                id_blk.clone(),
                WebViewInbound::Blocked { url: blocked.url },
            );
        })),
        on_error: Some(Arc::new(move |code, message| {
            let mut sync_mounts = Vec::new();
            if matches!(
                code,
                EmbeddedErrorCode::Crashed
                    | EmbeddedErrorCode::Source
                    | EmbeddedErrorCode::NotGranted
            ) {
                {
                    let mut reg = web_registry();
                    if let Some(entry) = reg
                        .get_mut(&token)
                        .and_then(|p| p.entries.get_mut(&id_err))
                        .filter(|e| e.doc_id == doc_err)
                    {
                        entry.ready = false;
                    }
                    sync_mounts = reg
                        .get(&token)
                        .map(PageWebViews::current_mounts)
                        .unwrap_or_default();
                }
                deliver(token, id_err.clone(), WebViewInbound::Ready(false));
            }
            deliver(
                token,
                id_err.clone(),
                WebViewInbound::Error {
                    code: code.as_str().to_owned(),
                    message,
                },
            );
            if !sync_mounts.is_empty() {
                ui_sync(token, &sync_mounts);
            }
        })),
        on_fullscreen: Some(Arc::new(move |fullscreen| {
            deliver(token, id_fs.clone(), WebViewInbound::Fullscreen(fullscreen));
        })),
    };

    match page
        .open_embedded_document_typed(&doc_id, &src, options)
        .await
    {
        Ok(doc) => {
            let post_open = {
                let mut reg = web_registry();
                match reg
                    .get_mut(&token)
                    .and_then(|p| p.entries.get_mut(&web_id))
                    .filter(|e| e.doc_id == doc_id)
                {
                    None => None,
                    Some(entry) => {
                        entry.opened = true;
                        let newer = (entry.revision != revision).then(|| entry.src.clone());
                        let mounts = reg
                            .get(&token)
                            .map(PageWebViews::current_mounts)
                            .unwrap_or_default();
                        Some((mounts, newer))
                    }
                }
            };
            match post_open {
                None => {
                    page.close_embedded_document(&doc_id);
                }
                Some((mounts, newer_src)) => {
                    ui_sync(token, &mounts);
                    if let Some(newer_src) = newer_src
                        && let Err(err) = doc.set_source(&newer_src)
                    {
                        deliver(token, web_id.clone(), WebViewInbound::Ready(false));
                        deliver(
                            token,
                            web_id,
                            WebViewInbound::Error {
                                code: err.code.as_str().to_owned(),
                                message: err.message,
                            },
                        );
                    }
                }
            }
        }
        Err(error) => {
            let still_current = web_registry()
                .get(&token)
                .and_then(|p| p.entries.get(&web_id))
                .is_some_and(|e| e.doc_id == doc_id && e.revision == revision);
            if !still_current {
                return;
            }
            let (code, message) = match error {
                OpenEmbeddedError::Source(err) => (err.code.as_str().to_owned(), err.message),
                OpenEmbeddedError::Runtime(err) => ("WEBVIEW_NETWORK".to_owned(), err.to_string()),
            };
            deliver(token, web_id.clone(), WebViewInbound::Ready(false));
            deliver(token, web_id, WebViewInbound::Error { code, message });
        }
    }
}

pub(crate) fn exit_webview_fullscreen(token: Token, web_id: &str) {
    let Some(doc_id) = doc_id_of(token, web_id) else {
        return;
    };
    std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
        if let Some(page) = page_of(token)
            && let Some(doc) = page.embedded_document(&doc_id)
        {
            let _ = doc.exit_fullscreen();
        }
    }));
}

pub(crate) fn clear_webviews(token: Token, ui_clear: Option<fn(Token)>) {
    let had = web_registry().remove(&token).is_some();
    if let Some(clear) = ui_clear {
        clear(token);
    }
    if had && let Some(page) = page_of(token) {
        std::mem::drop(rong_rt::RongExecutor::global().spawn(async move {
            page.close_all_embedded_documents();
        }));
    }
}

#[cfg(target_os = "android")]
pub(crate) fn is_ready(token: Token, web_id: &str) -> bool {
    web_registry()
        .get(&token)
        .and_then(|p| p.entries.get(web_id))
        .is_some_and(|e| e.ready)
}

#[cfg(target_os = "android")]
pub(crate) fn java_webview<'a>(
    env: &mut jni::Env<'a>,
    token: Token,
    web_id: &str,
) -> Result<jni::objects::JObject<'a>, jni::errors::Error> {
    let Some(doc_id) = doc_id_of(token, web_id) else {
        return Ok(jni::objects::JObject::null());
    };
    let Some(doc) = page_of(token).and_then(|page| page.embedded_document(&doc_id)) else {
        return Ok(jni::objects::JObject::null());
    };
    env.new_local_ref(doc.webview().get_java_webview())
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn apple_webview_ptr(token: Token, web_id: &str) -> usize {
    let Some(doc_id) = doc_id_of(token, web_id) else {
        return 0;
    };
    page_of(token)
        .and_then(|page| page.embedded_document(&doc_id))
        .map(|doc| doc.webview().get_swift_webview_ptr())
        .unwrap_or(0)
}

#[cfg(target_env = "ohos")]
pub(crate) fn has_webviews(token: Token) -> bool {
    web_registry()
        .get(&token)
        .is_some_and(|p| !p.entries.is_empty())
}

#[cfg(target_env = "ohos")]
pub(crate) fn child_webtag(token: Token, web_id: &str) -> Option<String> {
    let doc_id = doc_id_of(token, web_id)?;
    page_of(token)?
        .webtag()
        .child(&doc_id)
        .map(|tag| tag.as_str().to_owned())
}
