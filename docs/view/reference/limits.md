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
| `runs` | 256 | Runs in one `<text>`: each `<span>`, `<link>` and the text between them. |
| `transitionMs` | 10000 | Longest transition duration or delay, in milliseconds. |
| `evalBudgetMs` | 100 | CPU-time budget for one View render or callback (a page's first render gets twice it); past it, or past ten times it in wall time, the page fails with UI_TIMEOUT. |
| `editorTextBytes` | 65536 (64 KiB) | Text one editor holds, as UTF-8. |
| `textBytes` | 65536 (64 KiB) | Text of one node, as UTF-8. |
| `identifierBytes` | 256 | One `key` or `data-testid`, as UTF-8. |
| `listRows` | 100000 | Rows one `<List>` holds. |
| `gridTracks` | 64 | Tracks one grid template expands to. |
| `vectorSourceBytes` | 32768 (32 KiB) | One `<vector>` source (paths, gradients, viewBox) as JSON. |
| `vectorPathBytes` | 16384 (16 KiB) | One path `d` string. |
| `vectorPathWork` | 16384 | Path elements one `<vector>` draws, strokes outlined. |
| `vectorWork` | 32768 | Path elements all vectors of one frame draw. |
| `plotWork` | 16384 | Geometry all plots of one frame draw. |
| `dataAttributeBytes` | 65536 (64 KiB) | `data-*` props of a page, as UTF-8 JSON; automation reads them, nothing draws them. |
| `blurLayers` | 8 | Elements a frame draws with `blur-*`; past it further ones draw sharp. |
| `topLayer` | 16 | Dialogs and popovers open at once; a further one stays closed (`UI_BUDGET`) and fires no `onClose` or `onToggle`. |
