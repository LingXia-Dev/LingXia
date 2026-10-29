_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# View engine limits

Wire and renderer bounds behind the public [limits](../view/reference/limits.md): they keep a frame
decodable and its GPU memory bounded, and page code cannot act on them.

| Limit | Value | Meaning |
| --- | --- | --- |
| `decodedImageBytes` | 100663296 (96 MiB) | Decoded-image cache shared by every page; it evicts, never fails. |
| `evalBudgetDebugMultiplier` | 8 | Multiplier on that budget in unoptimized (debug) builds. |
| `inputRequestBytes` | 524288 (512 KiB) | One input-method request from the platform, as UTF-8 JSON. |
| `inputBatchEdits` | 64 | Edits in one input-method batch. |
| `clipboardItems` | 128 | Clipboard items one paste reads. |
| `wireValues` | 100000 | Values in one encoded frame. |
| `shadowMaskBytes` | 16777216 (16 MiB) | Blurred shadow masks one frame rasterizes. |
| `compositeFrames` | 6 | Frame-sized working surfaces nested opacity, clip and backdrop layers may hold at once (at least 64 MiB); past it the deepest groups flatten. |
| `leaseFrames` | 6 | Frame-sized GPU textures a page may hold at once across prepared frames and kept animation layers (at least 64 MiB, under `compositeBytes`); past it the frame draws without the kept layer. |
| `compositeBytes` | 268435456 (256 MiB) | Absolute ceiling on those working surfaces, for very large windows. |
| `outlineGlyphs` | 4096 | Glyphs the platform outlines (Apple's PingFang UI faces) in one frame, each filled as a path; past it frames slow down. |
| `ghosts` | 64 | Removed elements still drawing their exit at once; past it further removals vanish at once. |
| `layoutMotions` | 256 | Elements moving to a new layout position under `transition-layout` at once; past it further moves land at once. |

Wire ABI: 1. Highest field id: 195.
