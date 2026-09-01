//! Provider-registered product commands.
//!
//! Built-in commands (`browser`, `app`, `computer`) live in this crate. A
//! host-linked provider can add more without this crate naming them. The
//! running app still has to register the matching socket handler.

use crate::transport::Transport;
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

/// One extra top-level command on the product executable.
#[derive(Clone, Copy)]
pub struct ExtraProductCommand {
    /// argv[1], e.g. the namespace the provider owns.
    pub name: &'static str,
    pub about: &'static str,
    /// Execute with the provider arguments after the top-level command name.
    /// The agent integration's private discriminator has already been removed.
    pub execute: fn(&dyn Transport, &[OsString]) -> i32,
}

fn registrations() -> &'static Mutex<Vec<ExtraProductCommand>> {
    static REGISTRATIONS: OnceLock<Mutex<Vec<ExtraProductCommand>>> = OnceLock::new();
    REGISTRATIONS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Publish a top-level product command. Registering twice for the same name
/// replaces the previous entry.
pub fn register_extra_product_command(command: ExtraProductCommand) {
    let mut registrations = registrations()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(existing) = registrations
        .iter_mut()
        .find(|registered| registered.name == command.name)
    {
        *existing = command;
        return;
    }
    registrations.push(command);
}

pub(crate) fn get(name: &str) -> Option<ExtraProductCommand> {
    registrations()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .copied()
        .find(|command| command.name == name)
}

pub(crate) fn all() -> Vec<ExtraProductCommand> {
    registrations()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

pub(crate) fn is_registered(name: &str) -> bool {
    registrations()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .any(|command| command.name == name)
}
