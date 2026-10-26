//! Text shaping caches, system text scaling and editor buffers.
/// Parley brush: the 1-based run a glyph belongs to, or 0 outside rich text.
pub type RunBrush = u16;

pub(crate) mod cache;
pub(crate) mod editing;
pub mod fonts;
pub mod input;
pub(crate) mod runs;
pub(crate) mod scale;
pub(crate) mod shape;
pub(crate) mod style;
#[cfg(test)]
mod tests;
