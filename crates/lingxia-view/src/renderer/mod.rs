//! Backends that rasterize a shared scene.
pub mod cpu;
pub(crate) mod cpu_paint;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod pool;
#[cfg(feature = "gpu")]
pub mod render_thread;
pub mod sentinel;
#[cfg(feature = "gpu")]
pub mod surfaces;

#[cfg(test)]
pub(crate) mod backdrop_tests;
#[cfg(test)]
mod composite_tests;
