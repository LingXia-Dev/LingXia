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
| `animatorDurationScale` | `number` | The platform's animation duration multiplier (0..10, default 1); 0 also sets `reducedMotion`. |
| `direction` | `"ltr" \| "rtl"` | Inline direction derived from `displayLanguage`. |
| `canDock` | `boolean` | Whether the window can dock an `as: 'aside'` beside the main; false until the host reports it. |
| `runner` | `boolean` |  |
| `renderer?` | `Readonly<{ fallback: "driver" \| null }>` | `fallback: "driver"` when this device's GPU driver failed and pages render on the CPU. |
