# taffy, vendored for LingXia

The crates.io package form (`cargo package`) of taffy 0.16.0 plus the patches below, used through
`[patch.crates-io]` in the workspace `Cargo.toml` until upstream takes them. Delete this directory
and the patch entry once a taffy release carries them.

- Upstream base: DioxusLabs/taffy `497de49c` ("Prepare for Taffy v0.16.0 release (#1420)").
- `src/` is the base with `patches/0001`–`0006` applied, formatted with upstream's `rustfmt.toml`.
  Each patch file is `git format-patch` output against the base, with the tests it adds (the
  `tests/` directory is not vendored; the unit tests in `src/` are). `patches/ISSUES.md` holds a
  draft upstream issue for each. Nothing has been filed.

## Patches

1. `0001` fix(flexbox): cap a non-shrinking item's basis contribution by its max size.
2. `0002` fix(flexbox): transfer a definite max size through aspect-ratio to an auto axis.
3. `0003` feat: resolve sizing keywords on min and max sizes. The style type of
   `min_size`/`max_size` is `Dimension`; flexbox, block and absolutely positioned layout measure
   the item under the keyword's constraint (`compute/common/sizing_keyword.rs`, unit tests
   alongside). Grid treats a keyword minimum as `auto` and a keyword maximum as `none`.
4. `0004` feat(tree): `TaffyTree::intrinsic_widths_with_measure`, a node's min-content and
   max-content widths after layout (`useBox`/`onLayout` content widths).
5. `0005` fix(tree): keep no layout from a sizing pass. Flexbox baseline alignment lays a row's
   children out even under `RunMode::ComputeSize`; `TaffyView` drops the stored layouts and
   final-layout cache entries made inside a `ComputeSize` call, so an ancestor's later
   final-layout cache hit cannot leave a probe's geometry in place. 0.15 skips the baseline pass
   when only the main-axis size is asked for, but a cross-axis or two-axis sizing call still runs
   it.
6. `0006` fix(tree): clear a node's hoisted children when it stops laying them out. A node that
   becomes a leaf, `display: none`, or part of a hidden subtree kept its old hoisted list, so
   `round_layout` visited a removed box and panicked (a dialog leaving, a WebView replaced).
   Upstream issue #1354; open PR #1425 covers the leaf case only.
