# Theme system

How a theme pack becomes colours, lengths and timings on a GPU page. Read before changing
`lingxia-theme`, the `roles` / `scales` lists in `contracts/view/schema.json`, scale lowering
in the compiler, or the environment's `palette` / `scales` keys. Authoring rules for classes are in
[view-styling.md](view-styling.md).

## Invariants

- Pages name roles and scale steps, never values. A colour that is not a role fails the build
  (`UI_COLOR`); only renderer fixtures may set `literalColors`.
- `roles` and `scales` in the schema are append-only slot lists. Slot indices are generated into
  Rust and JS; reordering one breaks every shipped bundle.
- Every pack that ships passes the `lingxia-theme` check. No path renders an unchecked pack: the
  host refuses one with `UI_ENVIRONMENT: theme pack <name>: …`.
- A theme change never re-runs a View. Palette changes repaint committed records; scale changes
  re-resolve them and relayout.
- The wire never carries a design system's names (`text-footnote`, `rounded-control`); the compiler
  resolves them to slots.

## Pack

One JSON document: `appearance` (role colours for light and dark), `accent` (seed, derive), `type`
(scale, leading, family), `shape.radius`, `space.unit`, `motion` (fast, base, slow, easing, enter,
exit). Completion fills what a pack leaves out from the default pack built into
`lingxia-theme`.

## Roles

Each role carries a `tier`, generated into Rust and JS:

| Tier | Count | Meaning |
| --- | --- | --- |
| authored | 12 | surfaces, lines, text, `danger`/`success`/`warning`; what a pack writes with an accent seed |
| fixed | 6 | `scrim`, `shadow`, `media`, `on-media`, `knob`, `media-canvas`; written or taken from the pack's dark canvas / the default |
| derived | 52 | accent family incl. `*-solid` and its hover/pressed steps, `focus`, status on/soft/solid/mark, info, `hue-1..8` and their soft forms, `chart-1..8`, `selection`, `surface-overlay` |

`resolve` computes every derived role a pack does not write, in OKLCH, so each passes its pairs; a
derived hue avoids the chart colours and yields to a status colour it would read as. A role a pack
writes explicitly is used as written and held to the same pairs.

Add a role only when a component needs a colour no existing role carries with a passing pair.

## Resolution and proof

- `resolve(pack, prefs)`: the preferences are appearance (auto already resolved by the host),
  high contrast, optional accent (replaces the seed), text scale. High contrast is a boost applied
  here, not a second palette.
- `check(pack)`: every pair in the schema's `on` lists meets its minimum contrast, in both
  appearances and both contrast levels. Runs in `lingxia build` and `lingxia theme check`, `new`
  and `tokens` (all through the same pack loader), the crate tests, and the host before it applies a
  theme update. A missed pair names the nearest passing value of the colour to move: the foreground,
  or the ground when the foreground is derived.
- `advise(pack)`: non-failing warnings (status colour reading as a hue or the accent, accent text
  below 4.5:1 on common surfaces, solid fill below 3:1).

## Scales

`schema.json` `scales` lists 31 slots in seven families: `type`, `leading`, `radius`, `space`,
`duration`, `ease`, and `em`; each names its pack field. `em` has no pack field and no value: it
stays a reference until layout, which knows the node's font size.

**Compiler.** The `@lingxia/view` compiler's scale theme defines every Tailwind scale variable as
`scale(<slot>)` and is appended after the project and package tokens, so it always wins: a project
or package cannot pin a step to a number. A length that is `step`, `n × step` or `n × step + constant`
lowers to `[slot, times, plus]` (`plus` in points); same-step terms fold. Two steps, a step times or
over a step, or `min`/`max` over steps fail with `UI_SCALE`. Literals (`p-[13px]`, inline numbers)
stay numbers and do not follow the pack. Percentages lower to `widthPercent`.

**Decode.** Fields listed in the schema's `scaledFields` accept a reference when the page sets
`scaleRefs`. The reader checks the slot's family against the field (`UI_SCALE: paddingTop does not take radius.lg`)
and refuses a resolved value below zero where the field forbids it (`UI_STYLE`). References resolve
once at commit in `lingxia-view`; the record keeps the number for layout, paint and hit-testing
and the slot beside it.

**Change.** A new `scales` push re-resolves every committed record, clears the text
cache, drops the retained layout and invalidates list measurements: the same work as a text-scale
change. The device text scale applies after the type step. Transition and animation steps resolve when they
start, so a running one keeps its timing.

## Delivery

The environment carries two internal keys, taken out before the View sees the snapshot:

- `palette`: one resolved palette for the current look. The host keeps four (light, dark and their
  high-contrast forms) and pushes the one matching `theme` and `highContrast`.
- `scales`: `{ revision, values }`, one value per slot. Without a push, the built-in scale values.

Precedence: a value the platform's measured environment carries wins;
else the host theme update (a revision and a pack, checked, then resolved); else the lxapp's
pack seeded at session start. Older revisions are ignored.

## Ownership

The pack belongs to the lxapp. Its root `theme.json` is completed and checked by `lingxia build`;
opening a page's lease registers it under the app session's worker before the
page's host reads it. Leases count per worker: the pack stays registered while any page of that
worker is live and is cleared when the last lease is disposed, so a relaunch that replaces pages
keeps it. An lxapp without one
uses the host pack, else the default pack.

`themes/<id>.json` beside `lingxia.yaml` is the host pack. `theme_for(appid)` is the lxapp's own
pack, else the host's, else the default. There are no colour keys in `lingxia.yaml`, `lxapp.json`
or page config.

Host chrome around a page (navigation bar, tab bar, separator, window floor) takes `surface`,
`line`, `text`, `text-muted`, `accent` and `canvas` from `theme_for(appid)` in the page's
appearance, the same way on every host. Product chrome (desktop rows, aside) takes the host pack's
roles. WebView pages get the same roles as `--lx-<role>` from the same resolver before first
paint.

## Design-system names

Liuli states its names once in its core token-names list and as `var()` aliases in its core
`tokens.css` (`--text-footnote: var(--text-sm)`), which the compiler resolves to slots.
`lingxia theme tokens` writes `theme/tokens.css` from a theme for Web pages and editor completion;
given a design system's names list it also writes those names as `var()` aliases of the steps. It
does not decide a GPU page's values.
