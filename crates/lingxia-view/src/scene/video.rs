//! `<video>` sources: the host resolves an authored `src` to the URL its player opens.
use std::collections::HashMap;
use std::sync::Arc;

/// Resolves an authored source under the page's authority; runs on the View worker.
pub type VideoResolver = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

/// Past this many sources the cache starts over rather than grow with a feed.
const CACHED: usize = 256;

#[derive(Default)]
pub(crate) struct Sources {
    resolver: Option<VideoResolver>,
    resolved: HashMap<String, Option<String>>,
}

impl Sources {
    pub(crate) fn has_resolver(&self) -> bool {
        self.resolver.is_some()
    }

    pub(crate) fn set_resolver(&mut self, resolver: VideoResolver) {
        self.resolver = Some(resolver);
        self.resolved.clear();
    }

    /// The player URL for `src`, or `None` when it cannot play; without a host resolver the
    /// source is the URL.
    pub(crate) fn url(&mut self, src: &str) -> Option<String> {
        if let Some(url) = self.resolved.get(src) {
            return url.clone();
        }
        let url = match &self.resolver {
            None => Some(src.to_owned()),
            Some(resolve) => resolve(src)
                .map_err(
                    |error| log::warn!(target: "lingxia_view", "<video src=\"{src}\">: {error}"),
                )
                .ok(),
        };
        if self.resolved.len() >= CACHED {
            self.resolved.clear();
        }
        self.resolved.insert(src.to_owned(), url.clone());
        url
    }
}
