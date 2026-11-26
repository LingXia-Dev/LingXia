# Draft upstream issues

Drafts only, one per patch in this directory; whether any is filed is the owner's call.

## 0001 Flexbox: a non-shrinking item's basis contribution ignores its max size

An item with `flex-shrink: 0` contributes its flex basis as a minimum to its container's
intrinsic main size, even when the basis exceeds the item's max size. An auto-height column
around such an item grows to the unclamped basis while the item itself is laid out at its max,
leaving a gap. Chrome and Firefox clamp the hypothetical main size by the max first. Repro and
fix: `0001`, test `a_non_shrinking_items_basis_contribution_is_capped_by_its_max_size`.

## 0002 Flexbox: a definite max size does not transfer through aspect-ratio to an auto axis

An item with `aspect-ratio: 1`, `width: 100%`, `height: auto` and `max-width: 240px` is laid out
240×240, but its flex base size and hypothetical main size use the untransferred height, so an
auto-height column around it is sized from the item's full width. CSS Sizing 4 transfers a
definite max size through the preferred aspect ratio when the other axis is auto, as browsers
do. Repro and fix: `0002`, test `max_width_transfers_through_aspect_ratio_to_the_auto_height`.

## 0003 Sizing keywords on `min-*` and `max-*` sizes

`min-width: min-content`, `max-height: fit-content` and the other sizing keywords are valid on
the min/max size properties, but `Style::min_size`/`max_size` are `LengthPercentageAuto`, so
they cannot be expressed. Proposal and implementation: `0003` makes them `Size<Dimension>` and
resolves the keywords by measuring the item under the keyword's constraint in flexbox (both
axes, stretched items included), block layout and absolute positioning; grid treats a keyword
minimum as `auto` and a keyword maximum as `none` for now. Unit tests in
`compute/common/sizing_keyword.rs`. This changes a public field type, so it needs a minor
release.

## 0004 An API for a node's min-content and max-content widths

A host that reports a box's content widths to its page (to choose between stacked and
side-by-side presentation) needs a node's min-content and max-content widths after layout,
through the same measure function. `0004` adds `TaffyTree::intrinsic_widths_with_measure`,
returning `[min, max]` per node. Test: `a_box_reports_its_content_widths_with_padding_and_border`.

## 0005 A sizing pass leaves its layout in the tree

Flexbox baseline alignment (`calculate_children_base_lines`) lays a row's children out with
`RunMode::PerformLayout` even when the row is only being sized. Since 0.15 this is skipped when
only the main-axis size is requested, but a cross-axis or two-axis `ComputeSize`, such as a column
measuring a row's height at its min-content width, still runs it. Those layouts are stored
(`set_unrounded_layout`, and each child's final-layout cache entry). On a later layout where
only a sibling changed, the row's final-layout cache hits and its subtree is skipped, so
rounding publishes the sizing pass's geometry: a text laid out on one line at max-content inside
a narrower cell, or a zero-basis cell at its min-content width. Repro and fix: `0005`, test
`a_baseline_rows_children_keep_their_layout_after_an_ancestor_is_sized_again`. The fix counts
the enclosing `ComputeSize` calls in `TaffyView` and stores nothing inside them. A custom
`LayoutPartialTree` would need the same rule; an alternative is to run the baseline pass in a
mode that returns baselines without storing.

## 0006 Stale hoisted children on leaves and hidden nodes

Already reported upstream as #1354; PR #1425 (open) clears the list when a node has no
positioning area, which covers a leaf and a node that is itself `display: none`, but not a node
inside a hidden ancestor: hidden layout returns before `compute_oof_layout` runs. Draft comment
for #1425: the same stale list survives under a hidden ancestor; `0006` also clears it in
`TaffyView`'s `PerformHiddenLayout` branch. Test:
`removing_a_hoisted_box_leaves_no_stale_hoisted_child` (leaf, hidden, hidden ancestor; each case
panics in `round_layout` without the fix).
