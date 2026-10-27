_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Limits

Page-wide bounds the runtime and the native decoder both enforce. Exceeding one fails the frame
(`UI_LIMIT`, `UI_LIST`, `UI_TEXT`, `UI_ANIMATION`), except the image budgets noted below.

| Limit | Value | Meaning |
| --- | --- | --- |
| `nodes` | 4096 | Nodes in one rendered frame, list rows included. |
| `depth` | 128 | Nesting depth of the rendered tree. |
| `animationBytes` | 262144 (256 KiB) | Animation definitions per page, as UTF-8 JSON. |
| `itemKeyBytes` | 262144 (256 KiB) | All `itemKeys` of a page, as UTF-8 JSON. |
| `imageBytes` | 8388608 (8 MiB) | Encoded size of one image resource. |
| `sourceBytes` | 8192 (8 KiB) | Length of one image `src`. |
| `visibleImageBytes` | 67108864 (64 MiB) | Decoded image bytes one page paints at once; past it an image gets `onError` and a placeholder. |
| `decodedImageBytes` | 100663296 (96 MiB) | Decoded-image cache shared by every page; it evicts, never fails. |
| `runs` | 256 | Runs in one `<text>`: each `<span>`, `<link>` and the text between them. |
| `transitionMs` | 10000 | Longest transition duration or delay, in milliseconds. |
| `evalBudgetMs` | 100 | Wall-clock budget for one View render or callback; past it the page fails with UI_TIMEOUT. |
| `evalBudgetDebugMultiplier` | 8 | Multiplier on that budget in unoptimized (debug) builds. |
| `editorTextBytes` | 65536 (64 KiB) | Text one editor holds, as UTF-8. |
| `inputRequestBytes` | 524288 (512 KiB) | One input-method request from the platform, as UTF-8 JSON. |
| `inputBatchEdits` | 64 | Edits in one input-method batch. |
| `clipboardItems` | 128 | Clipboard items one paste reads. |
| `wireValues` | 100000 | Values in one encoded frame. |
| `textBytes` | 65536 (64 KiB) | Text of one node, as UTF-8. |
| `identifierBytes` | 256 | One `key` or `testId`, as UTF-8. |
| `listRows` | 100000 | Rows one `<list>` declares. |
| `gridTracks` | 64 | Tracks one grid template expands to. |
| `vectorSourceBytes` | 32768 (32 KiB) | One `<vector>` source (paths, gradients, viewBox) as JSON. |
| `vectorPathBytes` | 16384 (16 KiB) | One path `d` string. |
| `vectorPathWork` | 16384 | Path elements one `<vector>` draws, strokes outlined. |
| `vectorWork` | 32768 | Path elements all vectors of one frame draw. |
| `plotWork` | 16384 | Geometry all plots of one frame draw. |
| `shadowMaskBytes` | 16777216 (16 MiB) | Blurred shadow masks one frame rasterizes. |
| `compositeFrames` | 6 | Frame-sized working surfaces nested opacity, clip and backdrop layers may hold at once (at least 64 MiB); past it the deepest groups flatten. |
| `leaseFrames` | 6 | Frame-sized GPU textures a page may hold at once across prepared frames and kept animation layers (at least 64 MiB, under `compositeBytes`); past it the frame draws without the kept layer. |
| `compositeBytes` | 268435456 (256 MiB) | Absolute ceiling on those working surfaces, for very large windows. |

Wire ABI: 1. Highest field id: 183.
