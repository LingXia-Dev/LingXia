<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Skeleton

Quiet shapes that stand in for content while it loads. Status: preview.

```ts
import { Skeleton, SkeletonGroup } from '@liulidesign/core/Skeleton';
```

Skeleton draws faint lines, blocks and circles in the shape of content that is on its way, so the page keeps its layout and does not jump when the content lands. SkeletonGroup, from the same import, wraps the shapes for one piece of content and tells assistive tech what is loading. A soft glint crosses each shape; it stops under reduced motion.

## Examples

### A list loading

```tsx
import { Skeleton, SkeletonGroup } from '@liulidesign/core/Skeleton';

/** A list loading: rows shaped as the rows that will arrive, named once as what is loading. */
export function ListLoading() {
  return (
    <SkeletonGroup label="Loading notes" className="gap-0 overflow-hidden rounded-container border border-line bg-surface divide-y divide-line" data-testid="loading-notes">
      {['a', 'b', 'c'].map(key => (
        <view key={key} className="flex-row items-center gap-3 px-4 py-3">
          <Skeleton shape="circle" className="size-10 shrink-0" />
          <view className="grow basis-0 gap-1">
            <Skeleton className="w-2/3" />
            <Skeleton className="w-1/3" />
          </view>
        </view>
      ))}
    </SkeletonGroup>
  );
}
```

### A card loading

```tsx
import { Card } from '@liulidesign/core/Card';
import { Skeleton, SkeletonGroup } from '@liulidesign/core/Skeleton';

/** A card loading: a picture, a heading and a paragraph. */
export function CardLoading() {
  return (
    <Card>
      <SkeletonGroup label="Loading the article" data-testid="loading-article">
        <Skeleton shape="block" className="h-32 self-stretch" />
        <Skeleton className="w-1/2" />
        <Skeleton lines={3} data-testid="paragraph" />
      </SkeletonGroup>
    </Card>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `shape` | `SkeletonShape` | `'text'` | `text` draws lines as tall as the text they stand in for; `block` and `circle` take their size from `className`. |
| `textSize` | `SkeletonTextSize` | `'body'` | With `text`, the type it stands in for: each line is that type's line, and grows with the reader's text size. |
| `lines` | `number` | `1` | With `text`, how many lines; the last is shorter, as a paragraph's is. |
| `className` | `string` |  | Size and placement: `h-32`, `size-10`, `w-2/3`. |
| `data-testid` | `string` |  |  |

## SkeletonGroup props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | What is loading, read once by assistive tech: "Loading notes". |
| `children` (required) | `Child` |  | Skeletons laid out as the content will be. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| group | `<view>` | no | SkeletonGroup: a status region named by `label`, marked busy. |
| lines | `<view>` | no | With `shape` text, the column of lines; the last of several is shorter. |
| bone | `<view>` | yes | One shape: a faint fill in the text colour. |
| glint | `<view>` | yes | A soft band of the surface colour that crosses the shape, then rests. |

## What an app may override

- `className`: Size and placement: `h-32`, `size-10`, `w-2/3`.
- `className` on SkeletonGroup: classes merged after the recipe's, so they win.
- The recipe, `skeletonRecipe` from `@liulidesign/core/Skeleton/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-body`, `--text-callout`, `--text-title`, `--text-display`, `--radius-control`, `--animate-skeleton-glint`.
- Colour roles it paints with, set by the theme pack: `text`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- States: busy (from SkeletonGroup, always).
- Each shape is hidden from assistive tech. SkeletonGroup is a status region named by `label` and marked busy, so the loading is said once, not per shape.
- The shapes are decorative and need no contrast; the content that replaces them meets its own.
- With reduced motion the glint stays out of view and the shapes are still.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Shape the skeleton like the content that will arrive: the same rows, the same picture size.
- Do: Wrap the skeletons for one piece of content in a SkeletonGroup named by what is loading.
- Do: Swap the whole group for the content at once.
- Don't: Use it for a wait inside a control; use Button's loading state or a Spinner.
- Don't: Keep it up when the answer is "nothing"; show an EmptyState.
- Don't: Skeleton text the person must read; a label or a value that is known now is drawn now.

Related: EmptyState, ProgressBar.
