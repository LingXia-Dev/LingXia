# Limits and compatibility

```tsx
import { List, useView } from '@lingxia/ui';

const rows = Array.from({ length: 10000 }, (_, i) => ({ id: `r${i}`, label: `Row ${i + 1}` }));

export function render() {
  const { env } = useView();
  return (
    <page>
      {env.renderer?.fallback === 'driver' && (
        <text role="status" className="px-4 py-2 text-xs bg-warning-soft">Rendering without the GPU on this device.</text>
      )}
      <List items={rows} getKey={row => row.id} renderItem={row => <text className="px-4 py-3 text-sm">{row.label}</text>}
        getItemHeight={() => 44} />
    </page>
  );
}
```

## Page limits

Both the View runtime and the host enforce fixed bounds on every frame; the current values are in
[limits](../reference/limits.md). The ones you meet first:

- **Nodes per frame.** Every rendered element counts, list rows included. A long list rendered with
  `map` inside a `<scroll>` hits it; a `<List>` renders only the rows near the viewport, so 10 000 items
  cost a screenful of nodes.
- **Depth.** Deeply nested wrapper components add depth without adding pixels; flatten them.
- **Runs per `<text>`.** Split very long rich text into several `<text>` paragraphs.
- **Item keys.** All `itemKeys` of a page share one byte budget; keep keys short ids, not content.
- **Images.** One resource has an encoded-size cap; a page has a budget of decoded image bytes painted at
  once, past which images get `onError` instead of failing the frame.
- **JavaScript time.** One render or callback may use a fixed amount of CPU time on the View's thread
  (a page's first render twice that); past it the page stops with `UI_TIMEOUT`. A busy machine does not
  count against it, a render that computes too much does.

Exceeding a limit fails the frame with `UI_LIMIT`, `UI_LIST` or `UI_TEXT`, and the previous frame stays.
Per-element bounds (options, plot values, vector paths, gradient stops, grid tracks, shadow layers) are
stated in each chapter and checked the same way.

## Host compatibility

A compiled page starts with a banner naming the contract it needs: the wire ABI, the highest field id
its runtime can emit, and the engine features it requires. The host checks the banner before running
any of the page's code:

- A host at least as new as the page runs it. Element, prop and style ids are only ever appended, so new
  hosts keep running old pages unchanged.
- A host older than the page refuses it with `UI_COMPAT` instead of guessing at fields it does not know.
  Update the host app or SDK; the page itself is fine.
- The page needs an engine that can interrupt runaway View code. Every host that renders native pages
  has one.

## Renderer fallback

Some GPU drivers crash while compiling the renderer's pipelines. The host records the device's adapter
before its first GPU frame; if that process dies, the adapter is never used again and pages render
through another GPU backend or on the CPU. `env.renderer.fallback` is `'driver'` while such a fallback
is in effect. The CPU path draws the same frames.

## Platform status

| Platform | Native pages |
| --- | --- |
| Android, iOS, macOS | yes |
| HarmonyOS, Windows | not yet; use WebView pages |
