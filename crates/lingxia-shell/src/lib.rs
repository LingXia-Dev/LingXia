//! Platform-neutral semantic core for the LingXia host shell.
//!
//! This crate owns app-declared sidebar actions, user-owned Pins, validation,
//! Pin persistence, and deterministic state transitions. It deliberately
//! has no dependency on lxapps, the surface graph, browser bookmarks, Logic,
//! or platform UI. Host integration resolves metadata and executes activation;
//! platform skins only render resolved snapshots and report stable ids.

mod error;
mod manager;
mod pin;
mod runtime;
mod sidebar_action;
mod store;
mod surface_menu;

pub use error::{ShellError, ShellResult};
pub use manager::{ShellManager, ShellSnapshot};
pub use pin::{MAX_SHELL_PINS, PinCollection, PinMutation, ShellPin, ShellPinTarget};
pub use runtime::{
    ShellHost, SidebarActionIntent, activate_sidebar_action, apply_current_pins,
    apply_current_sidebar_actions, initialize, is_pinned, manager, pins, resolved_sidebar_actions,
    set_pinned,
};
pub use sidebar_action::{
    MAX_HEADER_SIDEBAR_ACTIONS, ResolvedShellSidebarAction, ShellSidebarAction,
    ShellSidebarActionUpdate, SidebarActionCollection, SidebarActionPlacement,
};
pub use store::{PIN_STORE_FILE, ShellStore};
pub use surface_menu::{
    LxappSurfaceMenuAction, SurfaceMenuAction, SurfaceMenuBuiltinAction, SurfaceMenuContext,
    SurfaceMenuIntent, SurfaceMenuItem, SurfaceMenuItemRole, SurfaceMenuSection,
    SurfaceMenuSectionKind, SurfaceMenuSnapshot, compose_surface_menu,
};
