//! Platform-neutral semantic core for the LingXia host shell.
//!
//! This crate owns app-declared activators, user-owned Pins, validation,
//! versioned persistence, and deterministic state transitions. It deliberately
//! has no dependency on lxapps, the surface graph, browser bookmarks, Logic,
//! or platform UI. Host integration resolves metadata and executes activation;
//! platform skins only render resolved snapshots and report stable ids.

mod activator;
mod error;
mod manager;
mod pin;
mod runtime;
mod store;

pub use activator::{
    ActivatorCollection, ActivatorDeclaration, ActivatorKind, NativeShellCapability,
    ResolvedActivatorSnapshot, ResolvedShellActivator, ShellActivator, ShellActivatorTarget,
    ShellActivatorUpdate,
};
pub use error::{ShellError, ShellResult};
pub use manager::{ShellManager, ShellSnapshot};
pub use pin::{MAX_SHELL_PINS, PinCollection, PinMutation, ShellPin, ShellPinTarget};
pub use runtime::{
    ShellActivationIntent, ShellHost, activate, apply_current_activators, apply_current_pins,
    initialize, is_pinned, manager, pins, resolved_activator_snapshot, resolved_activators,
    set_pinned,
};
pub use store::{ACTIVATOR_STORE_FILE, PIN_STORE_FILE, ShellStore};
