//! Display lists and paint primitives shared by every renderer.
pub(crate) mod backdrop_region;
pub(crate) mod border;
pub mod color_filter;
pub mod composite;
pub(crate) mod gradient;
pub mod image;
#[cfg(test)]
mod image_semantics_tests;
pub(crate) mod plot;
pub(crate) mod render;
pub(crate) mod shadow;
pub(crate) mod slider;
pub(crate) mod vector;
