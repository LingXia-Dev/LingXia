<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Split

The main content and a companion beside it: side by side on a wide screen, one above the other on a half-open fold, stacked or in a sheet on a phone. Status: preview.

```ts
import { Split } from '@liulidesign/core/Split';
```

Split pairs the main content (a video, a document, a basket) with a companion `aside` (a live chat, comments, an inspector, a summary). On a wide screen the aside sits at the end, `asideWidth` wide, and at a fold that crosses the page it takes the end leaf. On a foldable stood half open like a laptop, the main content takes the upper half and the aside the lower. On a phone, `compact="stack"` puts the aside under the main content, and `compact="sheet"` puts it in a bottom sheet the page opens.

## Examples

### A live stream and its chat

```tsx
import { Split } from '@liulidesign/core/Split';

/** Primary + companion split: side-by-side on regular, upper/lower leaves on half-opened tabletop folds, stacked on compact. */
export function LiveStreamSplit() {
  return (
    <view className="h-[320px] overflow-hidden rounded-container border border-text/12">
      <Split
        asideWidth={240}
        asideLabel="Live chat"
        aside={
          <view className="p-3 gap-2">
            <text role="heading" headingLevel={2} className="text-footnote font-semibold text-text">Live chat</text>
            <text className="text-caption text-muted">Messages stay beside video on wide screens and below in compact or tabletop.</text>
          </view>
        }
        data-testid="live-split"
      >
        <view className="grow items-center justify-center bg-surface-sunken p-4">
          <text className="text-footnote font-semibold text-text">Video Canvas</text>
        </view>
      </Split>
    </view>
  );
}
```

### A document and its comments

```tsx
import { useLocal, useView } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Split } from '@liulidesign/core/Split';

/** Document + comments sheet: docked aside on regular, draggable BottomSheet on compact. */
export function DocumentSheetSplit() {
  const { env } = useView();
  const commentsOpen = useLocal('comments-open', false);
  const isCompact = env.sizeClass !== 'regular';
  return (
    <view className="h-[320px] overflow-hidden rounded-container border border-text/12">
      <Split
        compact="sheet"
        bind={commentsOpen}
        asideLabel="Comments"
        asideWidth={240}
        aside={
          <view className="p-3 gap-2">
            <text role="heading" headingLevel={2} className="text-footnote font-semibold text-text">Comments</text>
            <text className="text-caption text-muted">Review notes and inline discussion.</text>
          </view>
        }
        data-testid="doc-split"
      >
        <view className="grow p-4 gap-3">
          <text className="text-body font-semibold text-text">Architecture Spec</text>
          {isCompact && (
            <Button
              label="Comments"
              variant="secondary"
              width="content"
              onClick={() => commentsOpen.set(true)}
              data-testid="doc-open-comments"
            />
          )}
        </view>
      </Split>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `compact` | `"sheet" \| "stack"` | `'stack'` | `stack` puts the aside under the main content; `sheet` puts it in a bottom sheet the page opens. |
| `bind` | `Local<boolean>` |  | A View-local cell: whether the sheet is open. |
| `isOpen` | `boolean` |  | Whether the sheet is open. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  | Runs with `false` when the sheet is dismissed. |
| `children` | `Child` |  | The main content: a video, a document, a canvas. |
| `aside` (required) | `Child` |  | The companion beside it: a live chat, comments, an inspector. |
| `asideLabel` (required) | `string` |  | Names the companion for a screen reader in every form, beside, below or in a sheet: "Live chat", "Comments". Not drawn; start `aside` with its own heading. |
| `asideWidth` | `number` | `320` | The aside's width in pt beside the main content, when no fold crosses the page. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the root; the panes get `<testid>-primary` and `<testid>-aside`, the sheet `<testid>-sheet`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | Fills its page: a row on a wide screen, a column on a phone or a half-open fold. |
| primary | `<view>` | yes | The main content; what is inside it sizes itself to the pane's own width, not the screen's. |
| aside | `<view>` | yes | The companion, a group named by `asideLabel`; on a phone with `compact="sheet"`, the body of the sheet. |
| hingeGap | `<view>` | no | An empty strip as wide as the fold's crease, so nothing is drawn across it. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `splitRecipe` from `@liulidesign/core/Split/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- Colour roles it paints with, set by the theme pack: `canvas`, `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- The aside is a group named by `asideLabel`, so a screen reader can move to it; on a phone its sheet takes the same name, and Escape or Back closes it.
- The strip over a fold's crease is hidden from assistive tech.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use `compact="stack"` when both must stay in view on a phone, such as a live video above its chat.
- Do: Use `compact="sheet"`, with `bind` or `isOpen` and `onOpenChange`, and a button that shows only on a phone, when the companion can wait (comments, filters).
- Do: Start the companion with its own heading; `asideLabel` names it for a screen reader and the opening button, and is not drawn.
- Do: Let it fill the page to its bottom edge and across its width: the seam is placed from those edges, so it meets the crease whatever sits above.
- Don't: Read the fold or the platform in page code to move the video and the chat; Split does that.

Related: ListDetail, BottomSheet, Drawer.
