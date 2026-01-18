use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

use crate::error::LxAppError;
use crate::lxapp::LxApp;

/// Host API handler trait - for page layer to call host app capabilities directly
pub trait HostHandler: Send + Sync + 'static {
    fn call(&self, lxapp: Arc<LxApp>, input: Option<&str>) -> Result<String, LxAppError>;
}

/// Host API registry - stores host handlers
struct HostRegistry {
    handlers: HashMap<String, Arc<dyn HostHandler>>,
}

impl HostRegistry {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}

/// Global host API registry instance
static GLOBAL_HOST_REGISTRY: OnceLock<Mutex<HostRegistry>> = OnceLock::new();

/// Get global host API registry
fn get_host_registry() -> &'static Mutex<HostRegistry> {
    GLOBAL_HOST_REGISTRY.get_or_init(|| Mutex::new(HostRegistry::new()))
}

/// Register host API handler
pub fn register_host(name: &str, handler: Arc<dyn HostHandler>) {
    let registry = get_host_registry();
    registry
        .lock()
        .unwrap()
        .handlers
        .insert(name.to_string(), handler);
}

/// Check if host API exists and return the handler if found
pub(crate) fn get_host(name: &str) -> Option<Arc<dyn HostHandler>> {
    let registry = get_host_registry();
    registry.lock().unwrap().handlers.get(name).cloned()
}

/// Macro: simplify host API implementation
#[macro_export]
macro_rules! host_api {
    // No parameter version
    ($name:ident, $output:ty, $body:expr) => {
        pub struct $name;

        impl $crate::host::HostHandler for $name {
            fn call(
                &self,
                lxapp: std::sync::Arc<$crate::LxApp>,
                _input: Option<&str>,
            ) -> Result<String, $crate::LxAppError> {
                let result: $output = $body(lxapp)?;
                serde_json::to_string(&result)
                    .map_err(|e| $crate::LxAppError::Bridge(e.to_string()))
            }
        }
    };

    // With parameter version
    ($name:ident, $input:ty, $output:ty, $body:expr) => {
        pub struct $name;

        impl $crate::host::HostHandler for $name {
            fn call(
                &self,
                lxapp: std::sync::Arc<$crate::LxApp>,
                input: Option<&str>,
            ) -> Result<String, $crate::LxAppError> {
                let input_data: $input = match input {
                    Some(json) => serde_json::from_str(json)
                        .map_err(|e| $crate::LxAppError::Bridge(format!("Invalid input: {}", e)))?,
                    None => {
                        return Err($crate::LxAppError::Bridge("Missing input".to_string()));
                    }
                };
                let result: $output = $body(lxapp, input_data)?;
                serde_json::to_string(&result)
                    .map_err(|e| $crate::LxAppError::Bridge(e.to_string()))
            }
        }
    };
}
