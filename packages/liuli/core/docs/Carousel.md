<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Carousel

A few pages to swipe through, one at a time. Status: preview.

```ts
import { Carousel } from '@liulidesign/core/Carousel';
```

Carousel lays its children side by side in a scroll that stops on each one, and puts a PageControl with it: under the pages, or over their bottom edge for pictures. A swipe, a dot or the app's own button moves the page; `bind` shares it with the app.

## Examples

### Photos, with the dots over them

```tsx
import { Carousel } from '@liulidesign/core/Carousel';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  [1, 'The bay at sunrise'],
  [2, 'Fields behind the house'],
  [3, 'Dusk over the ridge'],
  [4, 'Morning fog in the valley'],
] as const;

/** Photos of one place: the dots lie over the pictures. */
export function Photos() {
  return (
    <Carousel label="Photos of the house" dots="over" pageLabel={(page, count) => `Photo ${page + 1} of ${count}`} data-testid="photos">
      {photos.map(([n, alt]) => (
        <view key={alt} className="relative w-full aspect-4/3">
          <image src={photo(n)} alt={alt} className="absolute inset-0 size-full object-cover" />
        </view>
      ))}
    </Carousel>
  );
}
```

### Pages moved by a button too

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Carousel } from '@liulidesign/core/Carousel';

const steps = [
  ['Write anywhere', 'Notes open in a tap and save as you type.'],
  ['Find it again', 'Search reads every note, list and photo.'],
  ['Share what you choose', 'Each note is private until you invite someone.'],
] as const;

/** Pages of content, with the page held outside so a button can move it. */
export function Welcome() {
  const page = useLocal('step', 0);
  const isLast = Math.round(page.value) === steps.length - 1;
  return (
    <view className="gap-3">
      <Carousel label="Welcome" bind={page} pageLabel={(at, count) => `Step ${at + 1} of ${count}`} data-testid="welcome">
        {steps.map(([title, body]) => (
          <view key={title} className="gap-1 bg-surface-sunken p-5 min-h-28">
            <text className="text-callout font-semibold text-text">{title}</text>
            <text className="text-footnote text-muted">{body}</text>
          </view>
        ))}
      </Carousel>
      <Button label={isLast ? 'Get started' : 'Next'} variant="primary" width="fill" onClick={() => { if (!isLast) page.set(Math.round(page.value) + 1); }} data-testid="next" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the pages for assistive tech: "Photos of the house". |
| `children` (required) | `Child` |  | The pages, one child each; every page is as wide as the carousel. |
| `bind` | `Local<number>` |  | A View-local cell holding the page, from 0, to read or set it from outside. |
| `pageLabel` (required) | `(page: number, count: number) => string` |  | Each dot's accessible name, written by the app: `(page, count) => \`Photo ${page + 1} of ${count}\``. |
| `dots` | `CarouselDots` | `'below'` | `over` lays the dots on the bottom edge of the pages, for pictures; `none` leaves paging to the swipe. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the root; the dots get `<testid>-dots`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The pages and their dots. |
| scroll | `<scroll>` | yes | The sideways scroll that stops on every page. |
| track | `<view>` | yes | The row of pages: a list named by `label`. |
| page | `<view>` | yes | One child, as wide as the carousel: a list item. |
| dots | `<view>` | no | A PageControl bound to the same page. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `carouselRecipe` from `@liulidesign/core/Carousel/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--radius-container`.
- It paints with no colour role of its own.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: Through the dots, and through anything focusable on the page in view.
- Enter or Space on a dot: Goes to its page.
- Press target: .
- The pages are a list, so a screen reader says "2 of 4" on the page it lands on; assistive scrolling steps one page.
- The dots are tabs named by `pageLabel`. They are the way to page without a swipe.
- It never moves by itself.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Keep it to a handful of pages that are alike: photos of one thing, the steps of a welcome.
- Do: Give every picture its `alt`.
- Don't: Hide things people must see behind a swipe; most never go past the first page.
- Don't: Put a carousel inside another sideways scroll.
- Don't: Move it on a timer.

## Limits

- Every page is the full width; a carousel does not show part of the next page.
- There are no previous and next arrows; on a desktop the dots page it.

Related: PageControl, Lightbox, Thumbnail.
