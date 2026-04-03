use crate::LxApp;
use std::sync::{Arc, OnceLock, RwLock};

type Resolver = Arc<dyn Fn(&LxApp, &str) -> Option<String> + Send + Sync>;

static PAGE_DEFINITION_RESOLVERS: OnceLock<RwLock<Vec<Resolver>>> = OnceLock::new();

fn resolvers() -> &'static RwLock<Vec<Resolver>> {
    PAGE_DEFINITION_RESOLVERS.get_or_init(|| RwLock::new(Vec::new()))
}

pub fn register_page_resolver<F>(resolver: F)
where
    F: Fn(&LxApp, &str) -> Option<String> + Send + Sync + 'static,
{
    let mut guard = resolvers().write().unwrap_or_else(|err| err.into_inner());
    guard.push(Arc::new(resolver));
}

pub fn resolve_page_path(lxapp: &LxApp, path: &str) -> Option<String> {
    let guard = resolvers().read().unwrap_or_else(|err| err.into_inner());
    for resolver in guard.iter().rev() {
        if let Some(resolved) = resolver(lxapp, path) {
            return Some(resolved);
        }
    }
    None
}
