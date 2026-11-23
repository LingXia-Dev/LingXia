<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Split

Primary canvas with a companion aside: side-by-side on regular, upper/lower on tabletop folds, stack or sheet on compact. Status: preview.

```ts
import { Split } from '@liulidesign/core/Split';
```

Split pairs primary content (video, document, checkout items) with a companion `aside` pane (live chat, comments, inspector, summary). On flat regular screens (`>= 600 dp`) the companion docks at the inline end and snaps to any vertical fold hinge; when a foldable stands half-opened horizontally (`hinge.posture === "half-opened"`), primary automatically takes the upper leaf and `aside` takes the lower leaf. On compact screens (`< 600 dp`), `compact="stack"` places `aside` underneath while `compact="sheet"` turns it into a draggable `<BottomSheet>`.

## Examples

### Live stream with companion chat

```tsx
import { Split } from '@liulidesign/core/Split';

/** Primary + companion split: side-by-side on regular, upper/lower leaves on half-opened tabletop folds, stacked on compact. */
export function LiveStreamSplit() {
  return (
    <view className="h-[320px] overflow-hidden rounded-container border border-text/12">
      <Split
        asideWidth={240}
        aside={
          <view className="p-3 gap-2">
            <text className="text-footnote font-semibold text-text">Live Chat</text>
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

### Document with comments sheet

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
        bindOpen={commentsOpen}
        asideTitle="Comments"
        asideWidth={240}
        aside={
          <view className="p-3 gap-2">
            <text className="text-caption text-muted">Review notes and inline discussion.</text>
          </view>
        }
        data-testid="doc-split"
      >
        <view className="grow p-4 gap-3">
          <text className="text-body font-semibold text-text">Architecture Spec</text>
          {isCompact && (
            <Button
              label="Open Comments"
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
| `children` | `Child` |  | Primary content (video, document, canvas). |
| `aside` (required) | `Child` |  | Companion aside content (live chat, comments, inspector). |
| `asideWidth` | `number` | `320` | Aside pane width in `regular` flat mode when no vertical hinge is present. |
| `asideTitle` | `string` | `'Details'` | Accessible title for the aside sheet when `compact="sheet"`. |
| `compact` | `SplitCompactMode` | `'stack'` | How `aside` presents on `compact` screens: vertical stack or draggable BottomSheet. |
| `isOpen` | `boolean` | `false` | Controlled open state for `compact="sheet"`. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  | Called when the compact sheet opens or closes. |
| `bindOpen` | `Local<boolean>` |  | Uncontrolled local open state cell for `compact="sheet"`. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | Row in regular flat mode; column in compact or half-opened tabletop posture. |
| primary | `<view>` | yes | The main content region; scoped by `<SizeClassScope>` to its own resolved column width. |
| aside | `<view>` | yes | The companion pane (or the body of the compact `<BottomSheet>` when `compact="sheet"`). |
| hingeGap | `<view>` | no | Decorative spacer matching a physical display crease so content never crosses the hinge. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `splitRecipe` from `@liulidesign/core/Split/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- It reads no Liuli scale token.
- Colour roles it paints with, set by the theme pack: `canvas`, `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- In `compact="sheet"` mode, `asideTitle` names the modal `<BottomSheet>` dialog and Escape or Back closes it.
- Physical hinge spacers are marked `aria-hidden` so assistive navigation skips them.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use `compact="stack"` when both panes must remain visible simultaneously on phones (such as live video above chat).
- Do: Use `compact="sheet"` with `bindOpen` and a compact-only trigger button when the companion pane is secondary (such as document comments or filters).
- Do: Let `<Split>` fill the page to its bottom edge and span its width: the seam is placed from those edges, so it meets the fold crease whatever sits above.
- Don't: Check `env.platform` or fold posture manually in page code to rearrange video and chat; `<Split>` handles tabletop folds automatically.

Related: ListDetail, BottomSheet, SplitView.
