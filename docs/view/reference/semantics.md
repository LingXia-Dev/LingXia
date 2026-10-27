_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Semantics

The accessibility tree the platform adapter exposes. Elements get their role from what they are;
the `role` prop sets one of the authored roles on any non-control element.

## Authored roles

| `role` | Exposed as |
| --- | --- |
| `heading` | `heading` |
| `list` | `list` |
| `listitem` | `list-item` |
| `dialog` | `dialog` |
| `alert` | `alert` |
| `status` | `status` |

`headingLevel` (1..6) applies only to `role="heading"`.

## All exposed roles

`button`, `check-box`, `switch`, `radio-group`, `radio-button`, `combo-box`, `text-field`, `scroll-view`, `horizontal-scroll-view`, `image`, `slider`, `text`, `link`, `heading`, `list`, `list-item`, `dialog`, `alert`, `status`

## Live regions

`accessibilityLiveRegion`: `off`, `polite`, `assertive`. Announced politeness: `polite`, `assertive`.

## Text actions

Selectable text supports `copy`, `cut`, `paste`, `set-selection`.
