# glifo, vendored for LingXia

The crates.io package of glifo 0.4.0 (linebender/vello `f3000c8d`, `glifo/`) plus one fix, used
through `[patch.crates-io]` in the workspace `Cargo.toml` and in each app workspace that builds the
View (`examples/lingxia-showcase-gpu/native/Cargo.toml`), until upstream takes it
(proposed as linebender/vello#2067). Delete this
directory and the patch entries once a glifo release carries it.

## Patches

`patches/` holds each as a diff against the package's `src/`.

1. `0001-keep-decoded-bitmap-strikes.patch` (`src/glyph.rs`): a bitmap glyph's PNG strike was
   decoded on every draw, before its atlas slot was looked up, because the decoded size places the
   glyph. Decoded strikes are now kept per font, glyph and strike ppem (up to 512, then cleared).
   Upstream proposal linebender/vello#2067 looks the atlas slot up before decoding instead, placing
   a cached glyph by its slot's height.
