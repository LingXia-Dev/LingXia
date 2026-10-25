//! Backends that rasterize a shared scene.
pub mod cpu;
pub(crate) mod cpu_paint;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod sentinel;

#[cfg(test)]
pub(crate) mod backdrop_tests;
#[cfg(test)]
mod composite_tests;
