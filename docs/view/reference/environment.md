_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Environment

`view.env` (from `render(view)` or `useView()`, or `useEnv()` in a component): read-only, and a change re-renders the page.

| Field | Type | Notes |
| --- | --- | --- |
| `theme` | `"light" \| "dark"` |  |
| `viewport` | `Readonly<{ width: number; height: number }>` | The page surface in logical pixels. |
| `insets` | `Readonly<Edges>` | System bars and host chrome over the surface, logical pixels. |
| `safeArea` | `Readonly<Edges>` | The system bars alone (status bar, cutout, home indicator), as CSS `env(safe-area-inset-*)`; a dialog covering host chrome pads by these, logical pixels. |
| `hinge` | `Readonly<HingeRect> \| null` | Physical display hinge/crease intersecting this page canvas in local logical pixels; null when absent. |
| `keyboard` | `Readonly<{ height: number }>` |  |
| `capsule` | `Readonly<{ x: number; y: number; width: number; height: number }> \| null` | The host's capsule (the ··· and close control) where it floats over the page, in page coordinates, logical pixels; `null` when the host shows none or it sits in a nav bar. The same box Web pages read as `--lx-page-chrome-capsule-*`. |
| `pointer` | `"fine" \| "coarse"` | `fine` pointers (mouse, trackpad) hover; `coarse` ones (touch) do not. |
| `platform` | `"apple" \| "android" \| "harmony" \| "windows" \| "web"` | Host platform family (for rendering platform shortcut symbols only; never for layout branching). |
| `os` | `string` |  |
| `formFactor` | `string` | `phone` or `tablet` from the smallest screen width; other hosts may report their own. |
| `sizeClass` | `"compact" \| "regular"` | `compact` below 600 logical pixels of page width, else `regular`. |
| `language` | `string` | The language the page's text is in: the host's display language narrowed to the `languages` its lxapp.json lists. `t`, built-in words, direction and glyph forms follow it. |
| `reducedMotion` | `boolean` | The system asks for reduced motion; `motion-reduce:`/`motion-safe:` follow it. |
| `highContrast` | `boolean` | The system asks for increased contrast; the palette resolves its high-contrast variant. |
| `animatorDurationScale` | `number` | The platform's animation duration multiplier (0..10, default 1); 0 also sets `reducedMotion`. |
| `hourCycle` | `"h12" \| "h23"` | 12- or 24-hour clock: the user's system setting, else the custom of the system language. |
| `firstWeekday` | `Weekday` | The first day of the week, a `Weekday` (1 = Monday … 7 = Sunday): the user's system setting, else the custom of the system language. |
| `direction` | `"ltr" \| "rtl"` | Inline direction derived from `language`. |
| `canDock` | `boolean` | Whether the window can dock an `as: 'aside'` beside the main; false until the host reports it. |
| `renderer?` | `Readonly<{ fallback: "driver" \| null }>` | `fallback: "driver"` when this device's GPU driver failed and pages render on the CPU. |
| `textScale` | `number` | How much the system's text size grows body text: 1 at the default size, 2 when it is twice as large, after the lxapp's `maxTextScale`. Sizes already follow it; read it only for a layout that must change shape, such as fewer columns. |
