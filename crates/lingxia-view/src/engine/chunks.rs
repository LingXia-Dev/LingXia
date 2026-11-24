//! Per-lxapp View chunk loading and specifier validation (`lx:view/<id>` and `lx:view/<id>/<lang>`).
//!
//! Only `lx:view/*` specifiers are accepted by the QuickJS module loader. Every other specifier
//! (relative paths, `..`, absolute paths, bare packages) is rejected before any file is touched.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

/// Why a chunk file could not be served from the lxapp's dist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkLoadError {
    /// The chunk file is absent from the lxapp.
    NotFound,
    /// The chunk file failed the lxapp's integrity check (`lxapp.integrity.json`).
    IntegrityMismatch,
}

/// Reads a dist-relative View chunk path (e.g. `_view/<hash>.js` or `_view/<hash>/<lang>.js`).
pub type ChunkLoader = Arc<dyn Fn(&str) -> Result<String, ChunkLoadError> + Send + Sync>;

static APP_CHUNKS: LazyLock<Mutex<HashMap<String, ChunkLoader>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Registers or clears the View chunk loader for a View worker key (`"{appid}:{session}"`).
pub fn set_app_chunks(worker_key: &str, loader: Option<ChunkLoader>) {
    let mut map = APP_CHUNKS.lock().unwrap_or_else(|p| p.into_inner());
    match loader {
        Some(loader) => {
            map.insert(worker_key.to_owned(), loader);
        }
        None => {
            map.remove(worker_key);
        }
    }
}

/// Looks up the View chunk loader registered for `worker_key`.
pub(crate) fn app_chunks(worker_key: &str) -> Option<ChunkLoader> {
    APP_CHUNKS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(worker_key)
        .cloned()
}

struct ActiveScope {
    loader: Option<ChunkLoader>,
    expected_dev_build_id: Option<String>,
    last_error: Option<String>,
}

thread_local! {
    static ACTIVE: RefCell<Option<ActiveScope>> = const { RefCell::new(None) };
}

/// Runs `f` with `loader` and optional dev `expected_dev_build_id` active for QuickJS's module
/// loader callback on the current thread.
pub(crate) fn with_loader<R>(
    loader: Option<ChunkLoader>,
    expected_dev_build_id: Option<String>,
    f: impl FnOnce() -> R,
) -> (R, Option<String>) {
    let previous = ACTIVE.with(|slot| {
        slot.borrow_mut().replace(ActiveScope {
            loader,
            expected_dev_build_id,
            last_error: None,
        })
    });
    let result = f();
    let error = ACTIVE.with(|slot| {
        let current = slot.borrow_mut().take();
        *slot.borrow_mut() = previous;
        current.and_then(|scope| scope.last_error)
    });
    (result, error)
}

/// Records the exact `UI_MODULE:` error from inside the QuickJS module loader callback.
pub(crate) fn record_error(message: String) {
    ACTIVE.with(|slot| {
        if let Some(scope) = slot.borrow_mut().as_mut()
            && scope.last_error.is_none()
        {
            scope.last_error = Some(message);
        }
    });
}

fn valid_segment(seg: &str) -> bool {
    !seg.is_empty()
        && seg != "."
        && seg != ".."
        && !seg.contains("..")
        && seg
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Validates a module specifier against the `lx:view/<id>` or `lx:view/<id>/<lang>` grammar
/// and maps it to its dist-relative path (`_view/<id>.js` or `_view/<id>/<lang>.js`).
pub(crate) fn specifier_to_dist_path(specifier: &str) -> Result<String, String> {
    let Some(rest) = specifier.strip_prefix("lx:view/") else {
        return Err(format!(
            "UI_MODULE: a View page can import only its lxapp's View chunk (got '{specifier}')"
        ));
    };
    let mut parts = rest.split('/');
    let Some(id) = parts.next() else {
        return Err(format!(
            "UI_MODULE: a View page can import only its lxapp's View chunk (got '{specifier}')"
        ));
    };
    if !valid_segment(id) {
        return Err(format!(
            "UI_MODULE: a View page can import only its lxapp's View chunk (got '{specifier}')"
        ));
    }
    match (parts.next(), parts.next()) {
        (None, None) => Ok(format!("_view/{id}.js")),
        (Some(lang), None) if valid_segment(lang) => Ok(format!("_view/{id}/{lang}.js")),
        _ => Err(format!(
            "UI_MODULE: a View page can import only its lxapp's View chunk (got '{specifier}')"
        )),
    }
}

/// Validates `specifier`, loads the chunk source through `loader`, checks its compatibility
/// header, and verifies `chunkBuildId` in dev.
pub(crate) fn load_chunk(
    specifier: &str,
    loader: Option<&ChunkLoader>,
    expected_dev_build_id: Option<&str>,
) -> Result<String, String> {
    let dist_path = specifier_to_dist_path(specifier)?;
    let Some(loader) = loader else {
        return Err(format!(
            "UI_MODULE: the View chunk {specifier} is not in this lxapp; rebuild it"
        ));
    };
    let source = match loader(&dist_path) {
        Ok(source) => source,
        Err(ChunkLoadError::NotFound) => {
            return Err(format!(
                "UI_MODULE: the View chunk {specifier} is not in this lxapp; rebuild it"
            ));
        }
        Err(ChunkLoadError::IntegrityMismatch) => {
            return Err(format!(
                "UI_MODULE: the View chunk {specifier} does not match this lxapp's build"
            ));
        }
    };
    crate::engine::compatibility::validate(&source)?;
    if let Some(expected) = expected_dev_build_id {
        let actual = crate::engine::compatibility::parse_manifest(&source)?
            .and_then(|manifest| manifest.chunk_build_id);
        if let Some(actual) = actual
            && actual != expected
        {
            return Err(format!(
                "UI_MODULE: this page was built against View chunk {expected}, the lxapp has {actual}; rebuild the page"
            ));
        }
    }
    Ok(source)
}

/// Resolves and loads `specifier` using the current thread's active chunk scope.
pub(crate) fn load_active(specifier: &str) -> Result<String, String> {
    let (loader, expected) = ACTIVE.with(|slot| {
        let guard = slot.borrow();
        match guard.as_ref() {
            Some(scope) => (scope.loader.clone(), scope.expected_dev_build_id.clone()),
            None => (None, None),
        }
    });
    load_chunk(specifier, loader.as_ref(), expected.as_deref())
}

/// Attempts to load a per-language catalogue module (`<prefix>/<lang>`), trying exact tag first
/// and then its primary subtag (e.g. `zh-CN` -> `zh`). Returns `Ok(None)` when absent.
pub(crate) fn load_language_module(
    prefix: &str,
    language: &str,
    loader: &ChunkLoader,
    expected_dev_build_id: Option<&str>,
) -> Result<Option<(String, String)>, String> {
    let trimmed = language.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let mut candidates = vec![trimmed.to_owned()];
    if let Some((primary, _)) = trimmed.split_once(['-', '_'])
        && !primary.is_empty()
        && primary != trimmed
    {
        candidates.push(primary.to_owned());
    }
    for tag in candidates {
        if !valid_segment(&tag) {
            continue;
        }
        let specifier = format!("{prefix}/{tag}");
        let dist_path = specifier_to_dist_path(&specifier)?;
        match loader(&dist_path) {
            Ok(source) => {
                crate::engine::compatibility::validate(&source)?;
                if let Some(expected) = expected_dev_build_id {
                    let actual = crate::engine::compatibility::parse_manifest(&source)?
                        .and_then(|m| m.chunk_build_id);
                    if let Some(actual) = actual
                        && actual != expected
                    {
                        return Err(format!(
                            "UI_MODULE: this page was built against View chunk {expected}, the lxapp has {actual}; rebuild the page"
                        ));
                    }
                }
                return Ok(Some((specifier, source)));
            }
            Err(ChunkLoadError::NotFound) => continue,
            Err(ChunkLoadError::IntegrityMismatch) => {
                return Err(format!(
                    "UI_MODULE: the View chunk {specifier} does not match this lxapp's build"
                ));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ViewApp, ViewSession, engine::bytecode, session::tests::source};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn chunk_with_runtime(extra: &str, build_id: Option<&str>) -> String {
        let mut manifest = serde_json::json!({
            "uiAbi": crate::protocol::UI_ABI,
            "minField": 102,
            "requiredFeatures": ["candidate-transactions", "bounded-jobs"],
            "compiler": "0.30.0",
        });
        if let Some(id) = build_id {
            manifest["chunkBuildId"] = serde_json::json!(id);
        }
        format!(
            "// LingXia UI module; {manifest}\n{}\n{extra}\nexport const runtimeReady = true;\n",
            source("function render(){return h('text',{},'unused')}")
                .replace("globalThis.__lingxiaRender = render;", "")
        )
    }

    fn page_importing_chunk(
        specifier: &str,
        body: &str,
        build_id: Option<&str>,
        catalogue_chunk: Option<&str>,
    ) -> String {
        let mut manifest = serde_json::json!({
            "uiAbi": crate::protocol::UI_ABI,
            "minField": 102,
            "requiredFeatures": ["candidate-transactions", "bounded-jobs", "view-chunks"],
            "compiler": "0.30.0",
        });
        if let Some(id) = build_id {
            manifest["chunkBuildId"] = serde_json::json!(id);
        }
        if let Some(cat) = catalogue_chunk {
            manifest["catalogueChunk"] = serde_json::json!(cat);
        }
        format!(
            "// LingXia UI module; {manifest}\nimport {{ runtimeReady }} from '{specifier}';\nif (!runtimeReady) throw Error('missing chunk');\n{body}\n"
        )
    }

    #[test]
    fn loader_refuses_every_specifier_outside_lx_view() {
        let loader: ChunkLoader = Arc::new(|_| Err(ChunkLoadError::NotFound));
        for bad in [
            "./local.js",
            "../escape.js",
            "/etc/passwd",
            "lodash",
            "lx:view/",
            "lx:view/..",
            "lx:view/abc/../def",
            "lx:view/a/b/c",
        ] {
            let page = format!("import '{bad}';\n");
            let err = ViewSession::new_with_chunks(&page, false, loader.clone())
                .err()
                .expect(bad);
            assert_eq!(
                err,
                format!(
                    "UI_MODULE: a View page can import only its lxapp's View chunk (got '{bad}')"
                ),
                "specifier: {bad}"
            );
        }
    }

    #[test]
    fn missing_integrity_and_stale_dev_chunk_return_exact_diagnostics() {
        let missing: ChunkLoader = Arc::new(|_| Err(ChunkLoadError::NotFound));
        let page = page_importing_chunk(
            "lx:view/abc123",
            "globalThis.__lingxiaRender = () => h('text', {}, 'hi');",
            None,
            None,
        );
        assert_eq!(
            ViewSession::new_with_chunks(&page, false, missing)
                .err()
                .unwrap(),
            "UI_MODULE: the View chunk lx:view/abc123 is not in this lxapp; rebuild it"
        );

        let tampered: ChunkLoader = Arc::new(|_| Err(ChunkLoadError::IntegrityMismatch));
        assert_eq!(
            ViewSession::new_with_chunks(&page, false, tampered)
                .err()
                .unwrap(),
            "UI_MODULE: the View chunk lx:view/abc123 does not match this lxapp's build"
        );

        let chunk_v2 = chunk_with_runtime("", Some("build-b"));
        let stale_loader: ChunkLoader = Arc::new(move |path| {
            if path == "_view/dev.js" {
                Ok(chunk_v2.clone())
            } else {
                Err(ChunkLoadError::NotFound)
            }
        });
        let stale_page = page_importing_chunk(
            "lx:view/dev",
            "globalThis.__lingxiaRender = () => h('text', {}, 'hi');",
            Some("build-a"),
            None,
        );
        assert_eq!(
            ViewSession::new_with_chunks(&stale_page, false, stale_loader)
                .err()
                .unwrap(),
            "UI_MODULE: this page was built against View chunk build-a, the lxapp has build-b; rebuild the page"
        );
    }

    #[test]
    fn shared_chunk_caches_bytecode_per_module_across_pages_and_edits() {
        let chunk = chunk_with_runtime("export const tag = 'shared-cache-v1';", None);
        let chunk_key = bytecode::key(&chunk);
        bytecode::forget(&chunk_key);
        let loads = Arc::new(AtomicUsize::new(0));
        let loads_ref = loads.clone();
        let chunk_ref = chunk.clone();
        let loader: ChunkLoader = Arc::new(move |path| {
            if path == "_view/cachetest1.js" {
                loads_ref.fetch_add(1, Ordering::Relaxed);
                Ok(chunk_ref.clone())
            } else {
                Err(ChunkLoadError::NotFound)
            }
        });

        let page_a = page_importing_chunk(
            "lx:view/cachetest1",
            "globalThis.__lingxiaRender = () => h('text', {}, 'Page A');",
            None,
            None,
        );
        let page_b = page_importing_chunk(
            "lx:view/cachetest1",
            "globalThis.__lingxiaRender = () => h('text', {}, 'Page B');",
            None,
            None,
        );
        let page_a_key = bytecode::key(&page_a);
        let page_b_key = bytecode::key(&page_b);
        assert_ne!(page_a_key, page_b_key);
        assert_ne!(chunk_key, page_a_key);

        let mut app = ViewApp::new(2).unwrap();
        app.set_chunks(Some(loader));
        let a = app.create(&page_a, false).unwrap();
        assert!(
            bytecode::load(&chunk_key).is_some(),
            "chunk cached after page A"
        );
        let b = app.create(&page_b, false).unwrap();
        assert_eq!(loads.load(Ordering::Relaxed), 2);
        assert_eq!(
            app.page(a).unwrap().layout(320, 640, 1.0).unwrap().nodes[0].text,
            "Page A"
        );
        assert_eq!(
            app.page(b).unwrap().layout(320, 640, 1.0).unwrap().nodes[0].text,
            "Page B"
        );
    }

    #[test]
    fn per_language_catalogue_module_loads_synchronously_on_language_switch() {
        let chunk = chunk_with_runtime(
            "globalThis.__cat = {}; globalThis.__lingxiaRegisterCatalogue = (tag, msgs) => { globalThis.__cat[tag] = msgs; };",
            None,
        );
        let en = "// LingXia UI module; {\"uiAbi\":1,\"minField\":102,\"requiredFeatures\":[\"candidate-transactions\",\"bounded-jobs\"],\"compiler\":\"0.30.0\"}\nglobalThis.__lingxiaRegisterCatalogue('en-US', { greet: 'Hello' });\n";
        let zh = "// LingXia UI module; {\"uiAbi\":1,\"minField\":102,\"requiredFeatures\":[\"candidate-transactions\",\"bounded-jobs\"],\"compiler\":\"0.30.0\"}\nglobalThis.__lingxiaRegisterCatalogue('zh-CN', { greet: '你好' });\n";
        let loader: ChunkLoader = Arc::new(move |path| match path {
            "_view/i18ntest.js" => Ok(chunk.clone()),
            "_view/i18ntest/en-US.js" => Ok(en.to_owned()),
            "_view/i18ntest/zh-CN.js" => Ok(zh.to_owned()),
            _ => Err(ChunkLoadError::NotFound),
        });
        let page = page_importing_chunk(
            "lx:view/i18ntest",
            "globalThis.__lingxiaRender = ({ env }) => h('text', {}, (globalThis.__cat[env.language]?.greet) || 'missing');",
            None,
            Some("lx:view/i18ntest"),
        );

        let mut session = ViewSession::new_with_chunks(&page, false, loader).unwrap();
        let env = |lang: &str| {
            serde_json::json!({
                "theme": "light",
                "language": lang,
                "insets": { "top": 0, "right": 0, "bottom": 0, "left": 0 },
                "keyboard": { "height": 0 },
                "os": "android",
                "formFactor": "phone",
                "canDock": false
            })
        };
        session.set_environment(env("en-US")).unwrap();
        assert_eq!(
            session.layout(320, 640, 1.0).unwrap().nodes[0].text,
            "Hello"
        );

        session.set_environment(env("zh-CN")).unwrap();
        assert_eq!(session.layout(320, 640, 1.0).unwrap().nodes[0].text, "你好");
    }
}
