//! Display lists and paint primitives shared by every renderer.
pub(crate) mod backdrop_region;
pub(crate) mod border;
mod build;
pub mod color_filter;
pub mod composite;
pub(crate) mod display;
pub(crate) mod gradient;
pub mod image;
#[cfg(test)]
mod image_semantics_tests;
pub(crate) mod node;
pub(crate) mod paint;
pub(crate) mod plot;
pub(crate) mod retained;
pub(crate) mod shadow;
pub(crate) mod slider;
#[cfg(test)]
pub(crate) mod test_prelude;
#[cfg(test)]
mod tests;
pub(crate) mod transform;
pub(crate) mod vector;
