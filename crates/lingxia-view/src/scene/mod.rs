//! Display lists and paint primitives shared by every renderer.
pub(crate) mod anchor;
pub(crate) mod backdrop_region;
pub(crate) mod border;
pub(crate) mod build;
pub(crate) mod chrome_material;
pub mod color_filter;
pub mod composite;
pub mod cost;
pub(crate) mod display;
pub(crate) mod gradient;
pub(crate) mod hud;
pub mod image;
#[cfg(test)]
mod image_semantics_tests;
pub(crate) mod mask;
pub(crate) mod node;
pub(crate) mod paint;
pub(crate) mod plot;
pub(crate) mod retained;
pub(crate) mod ring;
pub(crate) mod shadow;
pub(crate) mod slider;
pub(crate) mod sticky;
#[cfg(test)]
pub(crate) mod test_prelude;
#[cfg(test)]
mod tests;
pub(crate) mod transform;
pub(crate) mod vector;
pub mod video;
pub mod webview;
