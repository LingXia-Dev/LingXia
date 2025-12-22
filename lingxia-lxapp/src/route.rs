use crate::error::LxAppError;
use crate::lxapp::LxApp;
use crate::lxapp::uri as lx_uri;
use crate::{plugin, startup};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RouteTarget {
    Normal { path: String },
    Plugin { name: String, path: String },
}

impl RouteTarget {
    pub(crate) fn internal_path(&self) -> String {
        match self {
            RouteTarget::Normal { path } => path.clone(),
            RouteTarget::Plugin { name, path } => plugin::build_plugin_page_path(name, path),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedRoute {
    pub(crate) original: String,
    pub(crate) query: Option<String>,
    pub(crate) target: RouteTarget,
}

impl ResolvedRoute {
    pub(crate) fn internal_path(&self) -> String {
        self.target.internal_path()
    }
}

pub(crate) fn resolve_route(lxapp: &LxApp, url: &str) -> Result<ResolvedRoute, LxAppError> {
    let original = url.to_string();
    let (path, query) = startup::split_path_query(url);

    if let Some((appid, page_path)) = lx_uri::parse_lxapp_url(&path) {
        if appid != lxapp.appid {
            return Err(LxAppError::ResourceNotFound(path));
        }

        return Ok(ResolvedRoute {
            original,
            query,
            target: RouteTarget::Normal { path: page_path },
        });
    }

    // Try lx://plugin scheme first, then plugin/ prefix
    let plugin_info =
        plugin::parse_plugin_url(&path).or_else(|| plugin::parse_plugin_page_path(&path));

    if let Some((plugin_name, page_path)) = plugin_info {
        let resolved_path = plugin::resolve_plugin_page(
            &lxapp.runtime,
            &lxapp.config.plugins,
            &plugin_name,
            &page_path,
        )?;
        return Ok(ResolvedRoute {
            original,
            query,
            target: RouteTarget::Plugin {
                name: plugin_name,
                path: resolved_path,
            },
        });
    }

    Ok(ResolvedRoute {
        original,
        query,
        target: RouteTarget::Normal { path },
    })
}
