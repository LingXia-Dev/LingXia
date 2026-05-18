//! Resource generators wired into the `lingxia gen` subcommand.
//!
//! Lives inside the CLI crate (rather than a standalone `lingxia-gen` lib)
//! because the CLI is the only consumer — `lingxia gen i18n …` / `lingxia
//! gen icons …` is the public entry point, and CI / release scripts call
//! that, not the underlying functions directly.

pub mod i18n;
pub mod icons;
