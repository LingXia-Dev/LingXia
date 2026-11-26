//! Test-only routes that open an embedded document under the calling page and
//! report what happened in it.
//!
//! Gated twice: the module exists only with the `automation` feature, which
//! product builds leave off, and every route refuses outside a dev session.
//! Audience is `AppSessionOnly`: an lxapp session's View, never a browser
//! document. The routes grant nothing a dev session's automation channel
//! does not already reach.

use super::HostCancel;
use crate::page::embedded_document::{BlockedLoad, EmbeddedDocument, EmbeddedDocumentOptions};
use crate::{LxAppError, PageInstance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

type BlockedLog = Arc<Mutex<Vec<BlockedLoad>>>;

// Blocked loads per child tag, kept until the probe closes the document.
fn blocked_logs() -> std::sync::MutexGuard<'static, HashMap<String, BlockedLog>> {
    static LOGS: OnceLock<Mutex<HashMap<String, BlockedLog>>> = OnceLock::new();
    LOGS.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn calling_page(invocation: &super::HostInvocationContext) -> Result<PageInstance, LxAppError> {
    if !crate::is_dev_session() {
        return Err(LxAppError::UnsupportedOperation(
            "embeddedDocumentProbe exists only in a dev session".to_string(),
        ));
    }
    invocation.page().ok_or_else(|| {
        LxAppError::InvalidParameter(
            "embeddedDocumentProbe must be called from a page View".to_string(),
        )
    })
}

fn document(page: &PageInstance, node: &str) -> Result<EmbeddedDocument, LxAppError> {
    page.embedded_document(node).ok_or_else(|| {
        LxAppError::InvalidParameter(format!("no embedded document {node:?} on this page"))
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OpenInput {
    node: String,
    url: String,
    hosts: Option<Vec<String>>,
}

#[derive(Serialize)]
struct Opened {
    webtag: String,
}

struct Open;

impl super::HostHandler for Open {
    fn call<'a>(
        &'a self,
        invocation: super::HostInvocationContext,
        input: Option<String>,
        _cancel: HostCancel,
    ) -> super::HostFuture<'a> {
        Box::pin(async move {
            let input: OpenInput = super::parse_input(input.as_deref())?;
            let page = calling_page(&invocation)?;
            let log = BlockedLog::default();
            let sink = log.clone();
            let options = EmbeddedDocumentOptions {
                hosts: input.hosts,
                on_blocked: Some(Arc::new(move |blocked| {
                    sink.lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(blocked);
                })),
                ..Default::default()
            };
            let result = page
                .open_embedded_document(&input.node, &input.url, options)
                .await
                .map(|document| {
                    let webtag = document.webtag().as_str().to_string();
                    blocked_logs().insert(webtag.clone(), log);
                    Opened { webtag }
                })
                .map_err(Into::into);
            super::serialize_result(result)
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvalInput {
    node: String,
    script: String,
}

struct Eval;

impl super::HostHandler for Eval {
    fn call<'a>(
        &'a self,
        invocation: super::HostInvocationContext,
        input: Option<String>,
        _cancel: HostCancel,
    ) -> super::HostFuture<'a> {
        Box::pin(async move {
            let input: EvalInput = super::parse_input(input.as_deref())?;
            let document = document(&calling_page(&invocation)?, &input.node)?;
            let result = document
                .webview()
                .evaluate_javascript(&input.script)
                .await
                .map_err(|error| LxAppError::Runtime(format!("{error:?}")));
            super::serialize_result(result)
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NodeInput {
    node: String,
}

struct Blocked;

impl super::HostHandler for Blocked {
    fn call<'a>(
        &'a self,
        invocation: super::HostInvocationContext,
        input: Option<String>,
        _cancel: HostCancel,
    ) -> super::HostFuture<'a> {
        Box::pin(async move {
            let input: NodeInput = super::parse_input(input.as_deref())?;
            let result = document(&calling_page(&invocation)?, &input.node).map(|document| {
                blocked_logs()
                    .get(document.webtag().as_str())
                    .map(|log| {
                        log.lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone()
                    })
                    .unwrap_or_default()
            });
            super::serialize_result(result)
        })
    }
}

struct Close;

impl super::HostHandler for Close {
    fn call<'a>(
        &'a self,
        invocation: super::HostInvocationContext,
        input: Option<String>,
        _cancel: HostCancel,
    ) -> super::HostFuture<'a> {
        Box::pin(async move {
            let input: NodeInput = super::parse_input(input.as_deref())?;
            let result = calling_page(&invocation).map(|page| {
                if let Some(document) = page.embedded_document(&input.node) {
                    blocked_logs().remove(document.webtag().as_str());
                }
                page.close_embedded_document(&input.node)
            });
            super::serialize_result(result)
        })
    }
}

pub(crate) fn register_all() {
    register_host_module!(
        "embeddedDocumentProbe",
        crate::host::RouteAudience::AppSessionOnly,
        {
            "open" => Arc::new(Open),
            "eval" => Arc::new(Eval),
            "blocked" => Arc::new(Blocked),
            "close" => Arc::new(Close)
        }
    );
}
