//! `SelfInfo` — read-only introspection of the calling lxapp.
//!
//! MVP: `current`, `pages`. (`info` runtime summary is a follow-up.)

use crate::resolve::upgrade;
use lxapp::LxApp;
use rong::{HostError, IntoJSObj, JSContext, JSResult, js_class, js_export, js_method};
use std::sync::{Arc, Weak};

#[js_export]
pub(crate) struct JSSelfInfo {
    lxapp: Weak<LxApp>,
}

impl JSSelfInfo {
    pub(crate) fn new(lxapp: &Arc<LxApp>) -> Self {
        Self {
            lxapp: Arc::downgrade(lxapp),
        }
    }
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSLxAppSummary {
    appid: String,
    #[rename = "currentPage"]
    current_page: Option<String>,
}

#[derive(Debug, Clone, IntoJSObj)]
struct JSPageConfig {
    name: String,
    path: String,
}

#[js_class(rename = "SelfInfo")]
impl JSSelfInfo {
    #[js_method(constructor)]
    fn _ctor() -> JSResult<()> {
        Err(HostError::new(rong::error::E_ILLEGAL_CONSTRUCTOR, "Use lx.automation()").into())
    }

    #[js_method]
    async fn current(&self, _ctx: JSContext) -> JSResult<JSLxAppSummary> {
        let app = upgrade(&self.lxapp)?;
        let info = app.runtime_info();
        Ok(JSLxAppSummary {
            appid: info.appid,
            current_page: info.current_page,
        })
    }

    #[js_method]
    async fn pages(&self, _ctx: JSContext) -> JSResult<Vec<JSPageConfig>> {
        let app = upgrade(&self.lxapp)?;
        let pages = app
            .runtime_info()
            .page_entries
            .into_iter()
            .map(|entry| JSPageConfig {
                name: entry.name,
                path: entry.path,
            })
            .collect();
        Ok(pages)
    }
}
