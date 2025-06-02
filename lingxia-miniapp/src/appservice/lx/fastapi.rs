use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

use crate::error::MiniAppError;
use crate::miniapp::MiniApp;

/// FastAPI handler trait
pub trait FastApiHandler: Send + Sync + 'static {
    fn call(&self, miniapp: Arc<MiniApp>, input: Option<&str>) -> Result<String, MiniAppError>;
}

/// FastAPI registry - stores FastAPI handlers
struct FastApiRegistry {
    handlers: HashMap<String, Arc<dyn FastApiHandler>>,
}

impl FastApiRegistry {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}

/// Global FastAPI registry instance
static GLOBAL_FAST_API_REGISTRY: OnceLock<Mutex<FastApiRegistry>> = OnceLock::new();

/// Get global FastAPI registry
fn get_fast_api_registry() -> &'static Mutex<FastApiRegistry> {
    GLOBAL_FAST_API_REGISTRY.get_or_init(|| Mutex::new(FastApiRegistry::new()))
}

/// Register FastAPI handler
pub fn register_fast_api(name: &str, handler: Arc<dyn FastApiHandler>) {
    let registry = get_fast_api_registry();
    registry
        .lock()
        .unwrap()
        .handlers
        .insert(name.to_string(), handler);
}

/// Check if FastAPI exists and return the handler if found
pub fn get_fast_api(name: &str) -> Option<Arc<dyn FastApiHandler>> {
    let registry = get_fast_api_registry();
    registry.lock().unwrap().handlers.get(name).cloned()
}

/// Macro: simplify FastAPI implementation
#[macro_export]
macro_rules! fast_api {
    // No parameter version
    ($name:ident, $output:ty, $body:expr) => {
        pub struct $name;

        impl $crate::appservice::lx::fastapi::FastApiHandler for $name {
            fn call(
                &self,
                miniapp: std::sync::Arc<$crate::miniapp::MiniApp>,
                _input: Option<&str>,
            ) -> Result<String, $crate::error::MiniAppError> {
                let result: $output = $body(miniapp)?;
                serde_json::to_string(&result)
                    .map_err(|e| $crate::error::MiniAppError::Bridge(e.to_string()))
            }
        }
    };

    // With parameter version
    ($name:ident, $input:ty, $output:ty, $body:expr) => {
        pub struct $name;

        impl $crate::appservice::lx::fastapi::FastApiHandler for $name {
            fn call(
                &self,
                miniapp: std::sync::Arc<$crate::miniapp::MiniApp>,
                input: Option<&str>,
            ) -> Result<String, $crate::error::MiniAppError> {
                let input_data: $input = match input {
                    Some(json) => serde_json::from_str(json).map_err(|e| {
                        $crate::error::MiniAppError::Bridge(format!("Invalid input: {}", e))
                    })?,
                    None => {
                        return Err($crate::error::MiniAppError::Bridge(
                            "Missing input".to_string(),
                        ));
                    }
                };
                let result: $output = $body(miniapp, input_data)?;
                serde_json::to_string(&result)
                    .map_err(|e| $crate::error::MiniAppError::Bridge(e.to_string()))
            }
        }
    };
}
