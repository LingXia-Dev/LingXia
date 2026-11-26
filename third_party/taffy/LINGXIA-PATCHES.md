# taffy, vendored for LingXia

The crates.io package form (`cargo package`) of taffy 0.14.0 plus the fixes below, used through
`[patch.crates-io]` in the workspace `Cargo.toml` until upstream takes them. Delete this directory
and the patch entry once a taffy release carries them.

- Upstream base: DioxusLabs/taffy `77f38568` ("Prepare for v0.14.0 release (#1153)").
- Fork head: `5616126b` on the local `lingxia-0.14` branch.

## Patches

Both live in `src/compute/flexbox.rs`; `patches/` holds them as `git format-patch` output, with the
upstream tests they add (the tests are not vendored).

1. `96d9f832` fix(flexbox): cap a non-shrinking item's basis contribution by its max size.
2. `5616126b` fix(flexbox): transfer a definite max size through aspect-ratio to an auto axis.
3. Sizing keywords on `min_size`/`max_size` (not in upstream or `patches/`): the style type is
   `Dimension`, and flexbox, block and absolute layout measure the item under the keyword's
   constraint (`compute/common/sizing_keyword.rs`, tests alongside). Grid treats a keyword
   minimum as `auto` and a keyword maximum as `none`.
4. A sizing pass keeps no layout (`src/tree/taffy_tree.rs`, not in upstream or `patches/`): flexbox
   baseline alignment lays its children out even under `RunMode::ComputeSize`. `TaffyView` now
   drops the stored layouts and final-layout cache entries made inside a `ComputeSize` call, so an
   ancestor's later final-layout cache hit cannot leave a probe's geometry (a max-content or
   zero-width cell) in place.
