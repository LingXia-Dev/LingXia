# taffy, vendored for LingXia

The crates.io package form (`cargo package`) of taffy 0.14.0 plus two flexbox fixes, used through
`[patch.crates-io]` in the workspace `Cargo.toml` until upstream takes them. Delete this directory
and the patch entry once a taffy release carries both.

- Upstream base: DioxusLabs/taffy `77f38568` ("Prepare for v0.14.0 release (#1153)").
- Fork head: `5616126b` on the local `lingxia-0.14` branch.

## Patches

Both live in `src/compute/flexbox.rs`; `patches/` holds them as `git format-patch` output, with the
upstream tests they add (the tests are not vendored).

1. `96d9f832` fix(flexbox): cap a non-shrinking item's basis contribution by its max size.
2. `5616126b` fix(flexbox): transfer a definite max size through aspect-ratio to an auto axis.
