_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Limits

Page-wide bounds the runtime and the native decoder both enforce. Exceeding one fails the frame
(`UI_LIMIT`, `UI_LIST`, `UI_TEXT`, `UI_ANIMATION`), except the image budgets noted below.

| Limit | Value | Meaning |
| --- | --- | --- |
| `nodes` | 4096 | Nodes in one rendered frame, list rows included. |
| `depth` | 128 | Nesting depth of the rendered tree. |
| `animationBytes` | 262144 (256 KiB) | Animation definitions per page, as UTF-8 JSON (gated motion). |
| `itemKeyBytes` | 262144 (256 KiB) | All `itemKeys` of a page, as UTF-8 JSON. |
| `imageBytes` | 8388608 (8 MiB) | Encoded size of one image resource. |
| `sourceBytes` | 8192 (8 KiB) | Length of one image `src`. |
| `visibleImageBytes` | 67108864 (64 MiB) | Decoded image bytes one page paints at once; past it an image gets `onError` and a placeholder. |
| `decodedImageBytes` | 100663296 (96 MiB) | Decoded-image cache shared by every page; it evicts, never fails. |
| `runs` | 256 | Runs in one `<text>`: each `<span>`, `<link>` and the text between them. |
| `transitionMs` | 10000 | Longest transition duration or delay, in milliseconds (gated motion). |
| `evalBudgetMs` | 100 | Wall-clock budget for one View render or callback; past it the page fails with UI_TIMEOUT. |
| `evalBudgetDebugMultiplier` | 8 | Multiplier on that budget in unoptimized (debug) builds. |

Wire ABI: 1. Highest field id: 177.
