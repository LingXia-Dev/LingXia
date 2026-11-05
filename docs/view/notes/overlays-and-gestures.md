# Design note: overlays, gestures and springs

Status: proposed. Extends the [gesture model](../guide/interaction.md) and [motion](../guide/motion.md);
builds on [presence](presence-and-layout-motion.md).

## Overlays

Two primitives, spelled as HTML spells them. A sheet and a menu are not primitives.

```tsx
<dialog open={open.value} onClose={() => open.set(false)} aria-label="Delete note?"
  className="m-auto rounded-2xl bg-surface-raised p-4 backdrop:bg-scrim transition duration-200 starting:opacity-0 exit:opacity-0">…</dialog>

<button popoverTarget="more" aria-label="More">⋯</button>
<view id="more" popover role="menu" className="position-area-bottom-span-end mt-1 rounded-xl bg-surface-raised">
  <button role="menuitem" onClick={rename}>Rename</button>
</view>
```

| Web | Here |
| --- | --- |
| `<dialog open>`, `close` event, `closedby="any\|closerequest\|none"`, `::backdrop` | `<dialog open onClose closedby>`, `backdrop:` (paint only). Default `closerequest`, as a modal's is |
| `popover` (`auto`, `manual`), `popovertarget`, `toggle` | `popover`, `popoverTarget`, `onToggle(open)`; `auto` behaves as `closedby="any"`, `manual` as `none` (no `closedby` on popovers, as in HTML); `id` is new, page-unique (`UI_ID`), read only by `popoverTarget` and `aria-actions` |
| `position-area`, `position-try-fallbacks`, `anchor-name` | `positionArea` (`top`/`bottom`, optionally `span-inline-start\|end`; style or `[position-area:…]`); flips and stays on screen always, 4 px from the anchor; the opening invoker carries the anchor name |
| `showPopover()` | `open` + `onToggle`: controlled exactly as `<Details>` is, uncontrolled without `open` |

**Departures, all deliberate:**
- `<dialog>` is always modal. HTML's non-modal form needs an imperative `showModal()` to tell them apart; a non-modal floating box is a `popover`. `popover` on `<dialog>` is `UI_PROP`.
- A tap outside a `popover="auto"` closes it and is swallowed, on every platform: a mouse click on macOS too, so no button fires under a closing menu. The web lets it through. The picker already behaves this way.
- A `role="menuitem"` closes its popover after `onClick`. The web has no shipped menu element; this is what the ARIA menu pattern prescribes, and otherwise every item repeats a close.
- `onToggle` takes a boolean, not the `toggle` event's `newState: "open"|"closed"`, matching `<Details>`.
- `role="dialog"` leaves the author's role list (`UI_PROP` names `<dialog>`). `role="menu"` and `menuitem` are added.

**Departures made while building it:** placement is the `positionArea` style (or `[position-area:…]`), not new
utilities; the popover sits a fixed 4 px from its anchor rather than taking the author's margin; `closedby` is
dialog-only, as in HTML; the opening invoker carries a CSS anchor name the popover sits against.

**Top layer.** An open dialog or popover lays out against the page's whole box, as an `absolute` child of `<page>` does (under the bars; `<page>`'s safe-area padding is not applied, so a sheet's background reaches the screen edge and the scrim covers everything), and paints above all page content in open order, ignoring ancestor clips and transforms. Its bottom edge rises with the keyboard, so a sheet holding an editor stays in view. On phones an open dialog also covers the host's navigation and tab bars (the page view spans the screen, the bars are chrome insets, and it rises above them while a dialog is open). On desktop the window's sidebar and title bar are not page chrome: a dialog covers the page's own box, as native macOS sheets leave the toolbar alone. This is native, not a JS hoist: `layout_parents` already sends `fixed` to the root; a top-layer flag does the same and `RetainedScene` appends those nodes to paint order. The tree stays intact, so identity, inherited typography and `<List>` rows work. A popover whose row is evicted, or whose anchor is gone, closes (`onToggle(false)`). Closed content does not exist, as in `<Details>`: `useLocal` inside it resets unless named above it.

**Anchoring.** The invoker that opened a popover anchors it through the existing `anchorTo`; `scene/anchor.rs` gains `positionArea` and RTL and follows scrolling without a render. A popover opened with no invoker is placed by its classes, centred by default.

**Dismissal.** One close-request stack, topmost first: Escape, Android back, VoiceOver's escape gesture, TalkBack's back. Opening a `popover="auto"` closes other autos unless it is nested in one. `manual` has no light dismiss.

**Semantics and focus.** A dialog hides the rest of the page from assistive tech, traps focus (the existing `active_dialog`, now top-layer aware) and returns it on close. It needs a name (`UI_A11Y`). An invoker gets `aria-expanded` and `aria-haspopup` for free. A menu takes arrows, Home/End and type-ahead (the picker's logic, shared). Automation: `click` on an invoker opens; the "page is behind an open dialog" refusal covers `<dialog>`; a click outside a popover dismisses, as a finger would.

**Internals move onto it.** `overlay.js`: modal and action sheet become `<dialog>`, toast a `popover="manual"` with `role="status"`. `picker.js`: popover with a mouse, sheet dialog on touch. `zIndex: 10000`, the private anchor plumbing and the separate Escape path go; `lx-*` test ids stay.

## Gestures

Three cases, and scroll already is one of them.

**Pager = scroll.** `<scroll bind={index} className="snap-x snap-always">`. CSS's `scroll-snap-stop: always` (`snap-always`) stops a fling skipping a child. `bind` on a snapping scroll is the settled child index, two-way: setting it springs there. That also answers "no scroll-to" for snapping regions only. `onScrollSnapChange` is not added; `bind` covers it.

**Everything else = one binding on one element.** Framer Motion's names, since the web has none (`draggable` is HTML drag-and-drop, a different thing):

```tsx
// Sheet: placed by margin, as a web dialog is. No <sheet>.
<dialog open={open.value} onClose={close} closedby="any" aria-label="Filters"
  drag="y" dragSnapPoints={['0%', '50%', '100%']} bind={detent}
  className="mt-auto w-full h-[90dvh] rounded-t-2xl bg-surface-raised backdrop:bg-scrim
             transition-transform duration-300 ease-spring starting:translate-y-full exit:translate-y-full">…</dialog>

// Swipe row: actions sit behind; a full swipe is one more point.
<stack>
  <row className="absolute inset-0 justify-end"><button id={`del-${r.id}`} onClick={remove}>Delete</button></row>
  <button drag="x" dragSnapPoints={[0, -88, '-100%']} bind={swipe} aria-actions={`del-${r.id}`} onClick={open}>…</button>
</stack>
```

- `drag`: `'x'` or `'y'`. No free 2D drag, no reordering in this step.
- `dragSnapPoints`: up to 8 offsets from the laid-out box, px or % of its own size. They are also the constraints; there is no `dragConstraints`. `x` offsets are inline-direction and mirror in RTL, like `start`/`end`.
- Past the ends the element rubber-bands. `overscroll-none` makes it stop hard. There is no `dragElastic`.
- Release timing is the element's transition timing (`transition-transform duration-* ease-spring*`; default 300 ms, bounce 0). There is no `dragTransition`.

**Outcomes that reach JS, all discrete, through the existing dispatch queue:**

| Outcome | Delivered as |
| --- | --- |
| drag claimed (past slop, axis locked) | `onDragStart()`, e.g. to close another open row |
| released | `onDragEnd({ index, velocity })`: the point it is heading for, px/s |
| settled | `bind` takes the index. A full swipe is this, or `onDragEnd` for an earlier start |
| dragged off | a `<dialog>` settling at an offset that hides it wholly calls `onClose`. By then it is off screen, so `exit:` has nothing left to play |
| page changed | a snapping scroll's `bind` |

A `bind` value from JS during a gesture waits for release: the finger wins, as scroll offsets' revisions do.

**Where it runs.** A `DragModel` beside `ScrollModel` on the input path: `ViewHost::pointer` moves the offset and returns `Effect::Redraw`, and composition applies it with the same translation sticky and anchor use (`RetainedScene`). No layout, no JS. Hit testing, semantics bounds and IME rects use composed bounds, so they follow. A release projects `offset + velocity × 0.15 s` to the nearest point; past `min_fling` it goes at least one point in that direction.

**Arbitration.** At slop the axis locks for the gesture (as scroll does today), then the chain from the innermost node decides. `gesture.target` becomes this chain:

1. A `<slider>` or text selection keeps what it has today, as does a scroll or `drag` on the other axis, which is not in the chain.
2. Dragging toward *expansion* inside a `drag` holding a scroll: the `drag` moves until it is at its most expanded point, then the scroll takes over. Dragging toward *collapse*: the scroll moves until its edge, then the `drag`. Handoff is continuous within one gesture, so a sheet's inner scroll at top hands the drag to the sheet with no re-touch. `overscroll-contain` on the scroll stops the chain.
3. Otherwise innermost first; leftover distance passes outward, as now.
4. A press inside a `drag` is cancelled at slop, as in a scroll. `UI_BUTTON`'s ban on scrolls in buttons stands, which is why swipe rows are `drag` and not nested scrolls: a row is a button, and a scroll per row would also cost a clip region and a semantic scroll node per row.

Considered and rejected: sheets as a scroll with a transparent top region. It needs hit-through on part of a scroll, drags the sheet from the scrim, and has the wrong order for rule 2.

A scroll inside a sheet at a partial detent extends its `max_offset` by the part off screen, or its end could never be reached.

**Accessibility: every gesture has an equivalent, enforced at build (`UI_A11Y`).** A `drag` needs one of:
- `aria-label`: adjustable. Increment and decrement move between points in index order; the value is a range from 1 to the number of points, read as a number. Keyboard arrows on the element's axis, Home and End do the same when focused. A `<dialog>` is named already.
- `aria-actions` (ARIA 1.3 draft, attribute shipping in Chromium; IDs of the elements behind): the row speaks those as custom actions, and they leave linear navigation while hidden.

Increment and decrement are the `scroll-forward`/`scroll-backward` pair sliders already use on both platforms, not a new pair; `Dismiss` is new, and the custom-action projection takes a label. Automation sees the same states: `drag: { axis, index, count }` in the semantic tree, the actions via `locator`, and `t.app.view.drag` for the pointer path.

## Springs

`ease-spring`, `ease-spring-bouncy` (bounce 0.25) and `ease-[spring(0.4)]`: bounce 0–0.9, after CSS's `spring()` proposal and SwiftUI. `duration-*` stays the one duration knob and means perceptual duration, so there are no stiffness or damping props. With ω = 2π/duration and ζ = 1 − bounce, each channel is closed-form, so sampling stays a pure function of time as today and corpus runs stay deterministic:

`x(t) = to + (x₀ − to)·A(t) + v₀·B(t)`

- **Interruption.** A retarget samples value and velocity (`A′`, `B′`) and starts a new spring from them. Bezier transitions keep their value-only continuation; `transition.rs` gains a per-channel velocity for translate, rotate, scale and layout offset.
- **Overshoot** applies to transform and layout offset. Opacity and colours clamp to their endpoints.
- **Drag.** Release velocity is `v₀`. A touch on a moving element samples its value and takes over.
- **Settling.** Finished when the envelope is under half a device pixel, capped at `transitionMs`.
- **Reduced motion.** Land at once; a finger still tracks, and release lands. `animatorDurationScale` scales duration; 0 reads as reduced.
- **Scroll snap** moves from `session/host/snap.rs`'s timed ease to this solver, keeping the fling's velocity. `snap-always` releases into the spring, skipping `Effect::Fling`.
- `@keyframes` refuses spring (`UI_ANIMATION`): a keyframe has no velocity to preserve.

**A limit that must move.** A transform of a subtree holding an editor, scroll or slider lands at once (`Resolved::transform = None`), which would make every real sheet jump in. A pure-translate `starting:`, `exit:` or `transition-layout` on such a subtree goes through composition translation instead, as a drag does. Scale and rotate keep the rule.

## Adapters

**Core does the physics.** No adapter gets per-frame gesture code; `Pointer.velocity` and vsync requests already exist.

**iOS (`Platform/iOS/View`):**
- `Effect::Claim` is a no-op in `crates/lingxia-view-apple/src/adapter.rs`. It must hold off the guest edge-back recognizer and any enclosing `UIScrollView` for the gesture.
- `GpuAccessibility`: `accessibilityViewIsModal` for the dialog subtree, `accessibilityPerformEscape` → `Dismiss`, `accessibilityDecrement` beside the existing increment, the custom-action allowlist takes labelled actions, `.screenChanged` on open and close, traits for `menu`/`menuitem`. **Roles assert when unmapped.**
- `GpuTextInput` and `GpuSelectionHandles`: check caret and handles follow a composed move on `Redraw`.
- Regenerated `GpuFrame` decoders.

**macOS:** the mouse drags through `pointer` already; `accessibilityPerformCancel`, role map. Wheel does not move a `drag`.

**Android:** `Claim` works already; back joins the close-request stack before the leave guard; TalkBack custom actions and adjustable; **`GpuAccessibility` throws on an unmapped role**, so `menu`/`menuitem` land with the schema change on all three.

## Wire, budgets, verification

**Schema, append-only, with a contract feature:** elements `dialog`; props `id`, `open`, `closedby`, `popover`, `popoverTarget`, `drag`, `dragSnapPoints`, `aria-actions`; styles `positionArea`, `snapStop`, the `backdrop` state style, a spring form of `Ease`; `bind` on `<scroll>` and on `drag` elements. Handlers stay JS-side.

**Limits (schema, `UI_BUDGET` past them):** 16 open top-layer nodes, 8 snap points, 256 `drag` nodes laid out per page. A 17th open is refused with `UI_BUDGET` and stays closed; it never opened, so no `onToggle(false)` or `onClose` fires. `FrameMetrics` gains `drags` and `springs`.

**Tests.**
- Host (`session/tests`): the arbitration table as pointer sequences, handoff in one gesture, RTL, row eviction, value and velocity continuous across a retarget, reduced motion, close-request order, focus return, modal semantics.
- Corpus (`--visual`): `overlay-dialog`, `overlay-menu`, `sheet-detents`, `swipe-row`, `pager`, spring at time zero and midpoint.
- `@lingxia/test` specs in showcase-gpu driving each by pointer and by accessibility action, on macOS by me.

**iPhone checks, one list per round:**
- Overlays: menu opens at a button in a scrolled list and follows; tap outside closes without activating; VoiceOver stays inside a dialog, two-finger scrub closes it.
- Motion: sheet enters with a scroll inside and does not jump.
- Gestures: drag a sheet with a slow and a fast flick, grab it mid-flight; inner scroll at top hands over to dismiss, no re-touch; swipe a row, full swipe, then scroll the list diagonally; VoiceOver's action rotor on the row; pager flick stops at one page; edge back still works from a page with a swipe row.

## Order

1. Top layer, `<dialog>`, `popover`, menu, a static sheet (`mt-auto`, `starting:`/`exit:` translate); translate of live subtrees, so that sheet holds a scroll without jumping; internals migrated. iOS check.
2. Spring solver; scroll snap on it.
3. `DragModel` and arbitration, then sheet detents, then swipe rows with `aria-actions`, then `bind` and `snap-always` pagers. iOS check after each of the first and last.

## Not in this step

Free 2D drag and reordering, pinch and rotate, a pointer-handler API, shared-element transitions, values driven by drag other than the backdrop's opacity (a dialog's scrim fades with how much of the dialog is on screen), `anchor-size()`, `popover="hint"`, components (`<Sheet>`, `<Menu>`).
