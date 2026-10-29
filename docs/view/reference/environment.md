_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Environment

`view.env` (from `render(view)` or `useView()`): read-only, and a change re-renders the page.

| Field | Type | Notes |
| --- | --- | --- |
| `theme` | `"light" \| "dark"` |  |
| `viewport` | `Readonly<{ width: number; height: number }>` | The page surface in logical pixels. |
| `insets` | `Readonly<{ top: number; right: number; bottom: number; left: number }>` | System bars and host chrome over the surface, logical pixels. |
| `keyboard` | `Readonly<{ height: number }>` |  |
| `pointer` | `"fine" \| "coarse"` | `fine` pointers (mouse, trackpad) hover; `coarse` ones (touch) do not. |
| `os` | `string` |  |
| `formFactor` | `string` | `phone` or `tablet` from the smallest screen width; other hosts may report their own. |
| `sizeClass` | `"compact" \| "regular"` | `compact` below 600 logical pixels of page width, else `regular`. |
| `displayLanguage` | `string` |  |
| `reducedMotion` | `boolean` | The system asks for reduced motion; `motion-reduce:`/`motion-safe:` follow it. |
| `highContrast` | `boolean` | The system asks for increased contrast; the palette resolves its high-contrast variant. |
| `animatorDurationScale` | `number` | The platform's animation duration multiplier (0..10, default 1); 0 also sets `reducedMotion`. |
| `hourCycle` | `"h12" \| "h23"` | 12- or 24-hour clock: the user's system setting, else the custom of `displayLanguage`. |
| `firstWeekday` | `number` | The first day of the week, 1 = Sunday to 7 = Saturday: the user's system setting, else the custom of `displayLanguage`. |
| `direction` | `"ltr" \| "rtl"` | Inline direction derived from `displayLanguage`. |
| `canDock` | `boolean` | Whether the window can dock an `as: 'aside'` beside the main; false until the host reports it. |
| `renderer?` | `Readonly<{ fallback: "driver" \| null }>` | `fallback: "driver"` when this device's GPU driver failed and pages render on the CPU. |
