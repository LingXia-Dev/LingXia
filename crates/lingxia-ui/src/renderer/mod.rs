//! Backends that rasterize a shared scene.
pub mod cpu;
#[cfg(feature = "gpu")]
pub mod gpu;

#[cfg(test)]
pub(crate) mod backdrop_tests;
#[cfg(test)]
mod composite_tests;
