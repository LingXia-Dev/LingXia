<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# WebView

An embedded web page, inline or expanded into a 70% sheet or full screen without reloading. Status: preview.

```ts
import { WebView } from '@liulidesign/core/WebView';
```

WebView frames an untrusted or packaged HTML document (`<webview>`) and optionally adds an expand control over its corner. Expanding opens a 70% bottom sheet or full-screen dialog holding the same `useWebView` session, so maps, previews and third-party widgets never reload or lose state when opened or dismissed.

## Examples

### Inline card that expands into a 70% sheet

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { WebView } from '@liulidesign/core/WebView';

const map = 'public/web/map.html';

/** An inline card with a corner expand button that opens the same document in a 70% bottom sheet. */
export function ExpandableSheet() {
  const hash = useLocal('section', '#overview');
  return (
    <view className="gap-2">
      <WebView
        src={`${map}${hash.value}`}
        label="Delivery area map"
        expand="sheet"
        expandLabel="Expand map"
        closeLabel="Close map"
        data-testid="sheet-web"
      />
      <Button
        label={hash.value === '#overview' ? 'Center on pin' : 'Show overview'}
        size="sm"
        width="content"
        onClick={() => hash.set(hash.value === '#overview' ? '#pin' : '#overview')}
        data-testid="toggle-hash"
      />
    </view>
  );
}
```

### Full-screen expansion with a footer action

```tsx
import { useLocal, useWebView } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { WebView } from '@liulidesign/core/WebView';

const map = 'public/web/map.html';

/** Controlled full-screen expansion with a native confirmation footer below the web document. */
export function FullScreenWithActions() {
  const web = useWebView('liuli-web-picker');
  const open = useLocal('picker-open', false);
  return (
    <WebView
      web={web}
      src={`${map}#picker`}
      label="Pick a meeting point"
      expand="full"
      expandLabel="Expand the map"
      closeLabel="Close the map"
      isOpen={open.value}
      onOpenChange={next => open.set(next)}
      actions={<Button label="Confirm location" variant="primary" onClick={() => open.set(false)} data-testid="confirm" />}
      data-testid="full-web"
    />
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `expand` | `"full" \| "none" \| "sheet"` | `'none'` | Stays inline. Adds an expand button over the frame that opens the same live document in a 70% `'sheet'` or a `'full'` dialog without reloading. |
| `expandLabel` | `string` |  | The expand button's name: "Expand map". |
| `closeLabel` | `string` |  | The close button's name in the expanded header: "Close map". |
| `isOpen` | `boolean` |  | Whether the expanded sheet or dialog is open; leave it out to let the expand and close buttons own it. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `dismissOn` | `DialogDismissOn` | `'any'` | Which requests close the expanded sheet or dialog (`'any'` includes a backdrop tap). |
| `actions` | `Child` |  | Footer inside the expanded sheet or dialog, such as a confirmation bar under a map. |
| `src` | `string` |  | Address or packaged path of the document. Optional when another `<webview>` already gave `web` a source. |
| `web` | `WebView` |  | Shared `useWebView` handle; when omitted, `WebView` creates its own so expanding still keeps one live document. |
| `label` (required) | `string` |  | What the embedded page shows, its accessible name and expanded heading. |
| `onLoad` | `((event: WebViewLoad) => void)` |  | Runs when a top-level document finishes loading. |
| `onMessage` | `((message: WebViewMessage) => void)` |  | Runs when the page calls `window.lingxia.postMessage`. |
| `onBlocked` | `((event: WebViewBlocked) => void)` |  | Runs when the host blocks an off-origin or disallowed navigation. |
| `onError` | `((error: WebViewError) => void)` |  | Runs when loading fails or a queued message is dropped. |
| `className` | `string` |  | Classes for the inline frame, such as its height or aspect ratio. |
| `data-testid` | `string` |  | On the inline `<webview>`; the expand button takes `<id>-expand` and the expanded dialog `<id>-sheet`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The rounded inline frame that clips the embedded document. |
| webview | `<webview>` | yes | The inline `<webview>` surface. |
| expand | `<button>` | no | Media-docked corner button shown when `expand` is `sheet` or `full`, named by `expandLabel`. |
| panel | `<dialog>` | no | The 70% bottom sheet or full-screen `<dialog>` shown while expanded. |
| header | `<view>` | no | Header row in the expanded sheet or dialog with document title and close button. |
| stage | `<webview>` | no | The expanded `<webview>` sharing the same `useWebView` session. |
| footer | `<view>` | no | Optional action bar below the expanded document, such as an address confirmation button. |

## What an app may override

- `className`: Classes for the inline frame, such as its height or aspect ratio.
- The recipe, `webViewRecipe` from `@liulidesign/core/WebView/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-title`, `--shadow-modal`, `--radius-container`, `--radius-dialog`.
- Colour roles it paints with, set by the theme pack: `canvas`, `focus`, `media`, `on-media`, `surface`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Escape: Closes the expanded sheet or full-screen dialog when `dismissOn` is `any` or `closeRequest`.
- Both the inline `<webview>` and the expanded dialog are named by `label`; while expanded, the page behind is inert to assistive tech.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| expand button | `on-media` on `media` | 4.5:1 | 7.94:1 | 7.94:1 |
| sheet title | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.11:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use `expand="sheet"` for interactive cards like maps or previews where 70% of the screen gives enough room while keeping page context.
- Do: Pass `actions` to place native confirmation controls below an expanded map or picker.
- Do: Update only the `#hash` of `src` when passing coordinates or state into an already-loaded document without reloading it.
- Don't: Nest `WebView` inside a pressable control (`Button`, `Item` with `onClick`).
- Don't: Mount two separate `WebView` components without sharing a `useWebView` handle when moving the same document into a custom `BottomSheet` or `Dialog`.

Related: BottomSheet, Dialog, VideoPlayer.
