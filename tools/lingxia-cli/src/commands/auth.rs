//! Authentication command facade.
//!
//! Platform-specific auth implementations live in submodules.

mod apple;
mod harmony;

pub use apple::{
    AppleLoginOptions, apple_import_developer_id, apple_login, apple_logout, apple_status,
};
pub use harmony::{HarmonyLoginOptions, harmony_login, harmony_logout, harmony_status};
