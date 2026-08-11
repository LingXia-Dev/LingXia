//! The command surface a LingXia product exposes, independent of who mounts it.
//!
//! `lxdev` is a development tool and always will be — it dials a `lingxia dev`
//! session over the network to drive a phone. But the commands it offers are
//! not its own: they belong to the platform, and a shipped product that wants
//! a command line or agent skills needs the same ones. Keeping one definition
//! here is what stops the two from drifting the moment a flag is added to one.
//!
//! Not every command needs a transport. `desktop` automates the local OS
//! through `lingxia-device-io` and never talks to a running app, so it
//! works in any binary that links it. The namespaces that do need a live app
//! reach it over the dev websocket in `lxdev` and over the product's local
//! control socket in a shipped binary.

pub mod app;
pub mod browser;
mod console;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod entry;
pub mod guard;
pub mod output;
pub mod skills;
pub mod transport;
