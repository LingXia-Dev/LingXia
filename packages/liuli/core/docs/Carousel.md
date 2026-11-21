<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Carousel

A few pages to swipe through, one at a time. Status: preview.

```ts
import { Carousel } from '@liulidesign/core/Carousel';
```

Carousel lays its children side by side in a scroll that stops on each one, and puts a PageControl with it: under the pages, or over their bottom edge for pictures. A swipe, a dot or the app's own button moves the page; `bind` shares it with the app. With `hasButtons` round buttons over the side edges move it too, and with `hasLoop` they go round from the last page to the first. With `itemWidth` it is a rail of cards, several in view, that stops on each.

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

### Buttons over the edges, going round

```tsx
import { Carousel } from '@liulidesign/core/Carousel';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  [1, 'The bay at sunrise'],
  [2, 'Fields behind the house'],
  [3, 'Dusk over the ridge'],
  [4, 'Morning fog in the valley'],
] as const;

/** With a pointer: round buttons over the edges, and past the last photo comes the first. */
export function Looping() {
  return (
    <Carousel
      label="Photos of the house"
      dots="over"
      hasButtons
      hasLoop
      previousLabel="Previous photo"
      nextLabel="Next photo"
      pageLabel={(page, count) => `Photo ${page + 1} of ${count}`}
      data-testid="looping"
    >
      {photos.map(([n, alt]) => (
        <view key={alt} className="relative w-full aspect-4/3">
          <image src={photo(n)} alt={alt} className="absolute inset-0 size-full object-cover" />
        </view>
      ))}
    </Carousel>
  );
}
```

### A rail of cards

```tsx
import { Carousel } from '@liulidesign/core/Carousel';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const albums = [
  [1, 'Coast, November', '48 photos'],
  [4, 'The valley', '112 photos'],
  [2, 'Fields', '23 photos'],
  [3, 'The ridge at dusk', '61 photos'],
  [1, 'Harbour walk', '17 photos'],
] as const;

/** A rail of cards, several in view, that stops on each; the buttons move one card. */
export function Rail() {
  return (
    <Carousel
      label="Albums"
      itemWidth={160}
      dots="none"
      hasButtons
      previousLabel="Previous albums"
      nextLabel="More albums"
      pageLabel={(page, count) => `Album ${page + 1} of ${count}`}
      data-testid="rail"
    >
      {albums.map(([n, title, count]) => (
        <view key={title} className="gap-2">
          <image src={photo(n)} alt="" className="w-full aspect-square rounded-control object-cover" />
          <view className="gap-0.5">
            <text className="text-footnote font-medium text-text truncate">{title}</text>
            <text className="text-caption text-muted">{count}</text>
          </view>
        </view>
      ))}
    </Carousel>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | Names the pages for assistive tech: "Photos of the house". |
| `children` (required) | `Child` |  | The pages, one child each; every page is as wide as the carousel, or `itemWidth`. |
| `bind` | `Local<number>` |  | A View-local cell holding the page, from 0, to read or set it from outside. |
| `pageLabel` (required) | `(page: number, count: number) => string` |  | Each dot's accessible name, written by the app: `(page, count) => \`Photo ${page + 1} of ${count}\``. |
| `dots` | `CarouselDots` | `'below'` | `over` lays the dots on the bottom edge of the pages, for pictures; `none` leaves paging to the swipe and the buttons. |
| `itemWidth` | `number` |  | Each item's width in points: a rail of cards, several in view, that stops on each. Without it a page fills the carousel. |
| `hasButtons` | `boolean` | `false` | Round buttons over the side edges that move a page; with a pointer, where a swipe is not to hand. |
| `hasLoop` | `boolean` | `false` | With the buttons: past the last page comes the first, and before the first the last. |
| `previousLabel` | `string` |  | The previous button's accessible name: "Previous photo". Needed with `hasButtons`. |
| `nextLabel` | `string` |  | The next button's accessible name: "Next photo". Needed with `hasButtons`. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the root; the dots get `<testid>-dots`, the buttons `<testid>-previous` and `<testid>-next`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The pages and their dots. |
| scroll | `<scroll>` | yes | The sideways scroll that stops on every page. |
| track | `<view>` | yes | The row of pages: a list named by `label`. |
| page | `<view>` | yes | One child, as wide as the carousel or `itemWidth`: a list item. |
| button | `<view>` | no | With `hasButtons`: a round chip on the overlay surface over each side edge, gone at an end unless it loops. |
| dots | `<view>` | no | A PageControl bound to the same page. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `carouselRecipe` from `@liulidesign/core/Carousel/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--shadow-raised`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `surface-overlay`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: Through the buttons and the dots, and through anything focusable on the page in view.
- Enter or Space on a button: Moves one page or one card.
- Enter or Space on a dot: Goes to its page.
- Press target: .
- The pages are a list, so a screen reader says "2 of 4" on the page it lands on; assistive scrolling steps one page.
- The dots are tabs named by `pageLabel`. They are the way to page without a swipe.
- The buttons are named by `previousLabel` and `nextLabel`; a button with nowhere to go is removed, not dimmed.
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

- A swipe does not go round; only the buttons do.
- A rail's dots step one card, so most rails leave them out.

Related: PageControl, Lightbox, Thumbnail.
