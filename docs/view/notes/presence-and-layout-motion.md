# Design note: presence (enter/exit) and layout motion (FLIP)

Status: implemented. Builds on `transition-*` ([Motion](../guide/motion.md)) and on
`childSlot` identity, which keeps a conditional child's identity stable while it comes and goes.

## Authoring (Tailwind dialect)

| Utility | Meaning |
| --- | --- |
| `starting:<paint>` (Tailwind's `@starting-style` variant) | the values an element enters from, e.g. `starting:opacity-0 starting:translate-y-2` |
| `exit:<paint>` (LingXia variant) | the values a removed element leaves toward, e.g. `exit:opacity-0 exit:scale-95` |
| `transition-layout` (LingXia utility) | animate this element's position when layout moves it (FLIP) |

`starting:`/`exit:` take paint properties only (opacity, transform, colours, shadow), like
`active:`. They animate with the element's own `transition-*` timing; without a transition they
do nothing. `transition-all` does not include `layout`; it is opted into by name.

```tsx
{open.value && <view className="transition duration-200 starting:opacity-0 starting:translate-y-2 exit:opacity-0" />}
<List items={rows} getKey={r => r.id} renderItem={r => <row className="transition-layout duration-200">…</row>} />
```

## Enter

A node seen for the first time with `starting:` styles starts a transition from those values to
its resolved ones, at its first presentation. Today a first-seen node never animates; that stays
the rule when it has no `starting:` values.

## Exit

When a View update removes a node whose subtree has `exit:` styles (and a transition), the host
keeps a **ghost**: the node's last presented paint (its committed display commands and layer), at
its last position, animating toward the `exit:` values for the transition's duration, then
dropped.

- The ghost takes no layout: siblings reflow at once (combine with `transition-layout` on them to
  close the gap smoothly).
- It is not hit-testable and leaves the semantics tree immediately; focus moves as for any removal.
- Only the outermost removed node ghosts; its subtree is frozen inside it (no state, no handlers).
- A node that comes back (same identity) before its exit ends cancels the ghost and enters again
  from the ghost's current values.
- Evicted virtual-list rows are not removed and never ghost.
- The ghost keeps the ancestors it had at removal: it scrolls with their scrollers and is clipped
  by the same clip chain, so an exiting list row never draws outside its scroller.
- A ghost on a hidden or parked page is dropped at the next present, never resumed.

## Layout motion (FLIP)

Each frame, a node with `transition-layout` whose laid-out origin differs from the one last
presented for its identity gets an inverse translation (old origin − new origin) that animates to
zero. Position only: size changes land at once, because scaling text and borders would distort
them. Hit testing follows the presented transform, as for transitions today. A reorder in a keyed
`<List>` or keyed children moves every displaced row together.

## Motion preference and scale

`env.reducedMotion` completes enters, exits and FLIP at once (exits drop the ghost on the next
frame); `animatorDurationScale` multiplies their durations, and 0 reads as reduced motion. Hidden
pages park them like other motion.

## Budgets and metrics

- At most 64 live ghosts and 256 FLIP nodes per page; beyond that new ones land at once and a
  `UI_BUDGET` warning names the limit (both limits move into the schema).
- `FrameMetrics.ghosts` and `FrameMetrics.layoutMotions` count the live ones; `FrameCost` counts
  ghost paints like other paints.
- Corpus: one page with a conditional panel and a keyed list reorder, rendered at time zero and
  at the transition midpoint, so the deterministic counters cover ghosts and FLIP.

## Wire and engine

- `starting:`/`exit:` lower to two new state-style fields, as `hoverStyle`/`pressedStyle` do; the
  transition field gains `layout` in its property set.
- `lingxia_ui::animation` owns ghosts (retained command ranges keyed by identity, reusing
  `animated_spans`) and FLIP offsets; adapters change nothing.

## Not in this step

Shared-element transitions across pages, size/clip (FLIP scale) animation, exit for elements
without a transition, and JS-visible animation events.
