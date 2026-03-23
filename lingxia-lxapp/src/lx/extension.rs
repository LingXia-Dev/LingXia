//! Extension system for extending LxApp JavaScript contexts.
//!
//! This module provides a way for external crates to register custom functionality
//! that will be automatically available in the JavaScript environment of an LxApp.
//!
//! ## Example
//!
//! ```rust
//! use lingxia_lxapp::lx::extension::{LxLogicExtension, register_logic_extension};
//! use rong::{JSContext, JSFunc, JSResult};
//!
//! struct MyFeatureExtension;
//!
//! impl LxLogicExtension for MyFeatureExtension {
//!     fn init(&self, ctx: &JSContext) -> JSResult<()> {
//!         fn greet_from_rust(_ctx: JSContext) -> JSResult<String> {
//!             Ok("Hello from Rust Extension!".to_string())
//!         }
//!
//!         let js_greet_func = JSFunc::new(ctx, greet_from_rust)?;
//!         ctx.global().set("greetFromRust", js_greet_func)?;
//!         // Now, `greetFromRust()` will be available in the LxApp's JS logic.
//!         Ok(())
//!     }
//! }
//!
//! //  Register the extension during crate initialization
//! fn init_my_feature() {
//!     register_logic_extension(Box::new(MyFeatureExtension));
//! }
//! ```
//!

use rong::{JSContext, JSResult};
use std::sync::{Mutex, OnceLock};

/// A trait for extensions that extend LxApp's JavaScript capabilities.
///
/// Implementors define how their functionality is integrated into the JS environment
/// by interacting with the provided [`JSContext`] in the `init` method.
pub trait LxLogicExtension: Send + Sync {
    /// Initialize the extension within the given JavaScript context.
    ///
    /// This method is called once per `LxApp` JS context creation, allowing the
    /// extension to register functions, define classes, or perform any necessary setup
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
    ///   extension fails.
    fn init(&self, ctx: &JSContext) -> JSResult<()>;
}

// Type alias for convenience when handling boxed extensions.
type BoxedExtension = Box<dyn LxLogicExtension>;

// Global registry for LxApp extensions. Initialized only once.
static EXTENSIONS: OnceLock<Mutex<Vec<BoxedExtension>>> = OnceLock::new();

/// Registers an extension to be initialized for LxApp JavaScript contexts.
///
/// This function should be called during the initialization phase of crates
/// that wish to extend LxApp functionality. The order of registration determines
/// the order of initialization.
///
/// # Arguments
///
/// * `extension`: A boxed instance of a type implementing [`LxLogicExtension`].
///
/// # Panics
///
/// Panics if called after the extension registry has been read for the first time
/// (i.e., after LxApp context creation has started). This is to prevent
/// modifications during runtime which could lead to inconsistencies.
pub fn register_logic_extension(extension: BoxedExtension) {
    // Get or initialize the Mutex<Vec>. `OnceLock::get_or_init` ensures
    // this happens only once and is thread-safe.
    let extensions_mutex = EXTENSIONS.get_or_init(|| Mutex::new(Vec::new()));

    // Acquire the lock to access the Vec. Panics if the mutex is poisoned,
    // which typically indicates a panic occurred in another thread while
    // holding the lock.
    let mut extensions = extensions_mutex
        .lock()
        .expect("Extension registry mutex is poisoned");

    // Add the new extension to the list. This is where the actual registration happens.
    extensions.push(extension);
}

/// Executes a closure with access to the list of registered extensions.
///
/// This function acquires a lock on the global extension registry and passes
/// a reference to the `Vec<BoxedExtension>` to the provided closure `f`.
/// This is the safe way to iterate over or inspect the registered extensions
/// without needing to clone the list or the trait objects.
///
/// If no extensions have been registered, the closure `f` is not called and
/// `None` is returned.
///
/// # Arguments
///
/// * `f`: A closure that takes `&Vec<BoxedExtension>` and returns a value of type `R`.
///
/// # Returns
///
/// * `Some(R)` if extensions were registered and the closure was executed.
/// * `None` if no extensions were registered.
pub(crate) fn with_registered_extensions<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Vec<BoxedExtension>) -> R,
{
    EXTENSIONS.get().map(|extensions_mutex| {
        let extensions = extensions_mutex
            .lock()
            .expect("Extension registry mutex is poisoned");
        f(&extensions)
    })
}
