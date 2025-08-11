//! Module system for extending LxApp JavaScript contexts.
//!
//! This module provides a way for external crates to register custom functionality
//! that will be automatically available in the JavaScript environment of an LxApp.
//!
//! ## Example
//!
//! ```rust
//! use lingxia_lxapp::module::{LxAppModule, register_module};
//! use rong::{JSContext, JSFunc, JSResult};
//!
//! struct MyFeatureModule;
//!
//! impl LxAppModule for MyFeatureModule {
//!     fn init(&self, ctx: &JSContext) -> JSResult<()> {
//!         fn greet_from_rust(_ctx: JSContext) -> JSResult<String> {
//!             Ok("Hello from Rust Module!".to_string())
//!         }
//!
//!         let js_greet_func = JSFunc::new(greet_from_rust);
//!         if let Err(e) = ctx.global().set("greetFromRust", js_greet_func) {
//!             eprintln!("Failed to register 'greetFromRust': {}", e);
//!         }
//!         // Now, `greetFromRust()` will be available in the LxApp's JS logic.
//!     }
//! }
//!
//! //  Register the module during crate initialization
//! fn init_my_feature() {
//!     register_module(Box::new(MyFeatureModule));
//! }
//! ```
//!

use rong::{JSContext, JSResult};
use std::sync::{Mutex, OnceLock};

/// A trait for modules that extend LxApp's JavaScript capabilities.
///
/// Implementors define how their functionality is integrated into the JS environment
/// by interacting with the provided [`JSContext`] in the `init` method.
pub trait LxAppModule: Send + Sync {
    /// Initialize the module within the given JavaScript context.
    ///
    /// This method is called once per `LxApp` JS context creation, allowing the
    /// module to register functions, define classes, or perform any necessary setup
    /// within that specific context.
    ///
    /// # Arguments
    ///
    /// * `ctx`: A reference to the [`JSContext`] for the LxApp being initialized.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if initialization was successful.
    /// * `Err(JSResult)` if an error occurred during initialization. This will
    ///   typically prevent the LxApp context from being fully usable if a critical
    ///   module fails.
    fn init(&self, ctx: &JSContext) -> JSResult<()>;
}

// Type alias for convenience when handling boxed modules.
type BoxedModule = Box<dyn LxAppModule>;

// Global registry for LxApp modules. Initialized only once.
static MODULES: OnceLock<Mutex<Vec<BoxedModule>>> = OnceLock::new();

/// Registers a module to be initialized for LxApp JavaScript contexts.
///
/// This function should be called during the initialization phase of crates
/// that wish to extend LxApp functionality. The order of registration determines
/// the order of initialization.
///
/// # Arguments
///
/// * `module`: A boxed instance of a type implementing [`LxAppModule`].
///
/// # Panics
///
/// Panics if called after the module registry has been read for the first time
/// (i.e., after LxApp context creation has started). This is to prevent
/// modifications during runtime which could lead to inconsistencies.
pub fn register_module(module: BoxedModule) {
    // Get or initialize the Mutex<Vec>. `OnceLock::get_or_init` ensures
    // this happens only once and is thread-safe.
    let modules_mutex = MODULES.get_or_init(|| Mutex::new(Vec::new()));

    // Acquire the lock to access the Vec. Panics if the mutex is poisoned,
    // which typically indicates a panic occurred in another thread while
    // holding the lock.
    let mut modules = modules_mutex
        .lock()
        .expect("Module registry mutex is poisoned");

    // Add the new module to the list. This is where the actual registration happens.
    modules.push(module);
}

/// Executes a closure with access to the list of registered modules.
///
/// This function acquires a lock on the global module registry and passes
/// a reference to the `Vec<BoxedModule>` to the provided closure `f`.
/// This is the safe way to iterate over or inspect the registered modules
/// without needing to clone the list or the trait objects.
///
/// If no modules have been registered, the closure `f` is not called and
/// `None` is returned.
///
/// # Arguments
///
/// * `f`: A closure that takes `&Vec<BoxedModule>` and returns a value of type `R`.
///
/// # Returns
///
/// * `Some(R)` if modules were registered and the closure was executed.
/// * `None` if no modules were registered.
pub(crate) fn with_registered_modules<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Vec<BoxedModule>) -> R,
{
    MODULES.get().map(|modules_mutex| {
        let modules = modules_mutex
            .lock()
            .expect("Module registry mutex is poisoned");
        f(&modules)
    })
}
