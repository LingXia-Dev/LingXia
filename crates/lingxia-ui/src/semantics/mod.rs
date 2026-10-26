//! Shared accessibility projection; adapters map it to OS APIs only.
pub(crate) mod groups;
pub mod ids;
pub mod projection;
pub mod query;
#[cfg(test)]
mod query_tests;
#[cfg(test)]
mod roles_tests;
