<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Lightbox

Pictures one at a time, over the whole screen. Status: preview.

```ts
import { Lightbox } from '@liulidesign/core/Lightbox';
```

Lightbox covers the screen with an opaque dark surface and shows pictures whole, one at a time: a swipe or a dot moves between them, and a line of caption follows the one in view. It opens from a Thumbnail at the picture that was pressed, and closes from its button, Escape or back.

## Examples

### Opened from a grid of thumbnails

```tsx
import { useLocal } from '@lingxia/view';
import { Thumbnail } from '@liulidesign/core/Thumbnail';
import { Lightbox } from '@liulidesign/core/Lightbox';

const photos = [
  { src: photo(1), alt: 'The bay at sunrise', caption: 'The bay at sunrise, from the terrace' },
  { src: photo(2), alt: 'Fields behind the house', caption: 'Fields behind the house' },
  { src: photo(3), alt: 'Dusk over the ridge', caption: 'Dusk over the ridge' },
  { src: photo(4), alt: 'Morning fog in the valley' },
] as const;

/** A grid of thumbnails: pressing one opens the pictures at that one; the page holds which is in view. */
export function FromGrid() {
  const open = useLocal('open', false);
  const page = useLocal('page', 0);
  return (
    <view className="gap-2">
      <view className="grid grid-cols-4 gap-2">
        {photos.map((each, index) => (
          <Thumbnail key={each.alt} src={each.src} alt={each.alt} size="fill" onClick={() => { page.set(index); open.set(true); }} data-testid={`open-${index}`} />
        ))}
      </view>
      <Lightbox
        isOpen={open.value}
        onOpenChange={next => open.set(next)}
        label="Photos of the house"
        images={photos}
        index={page.value}
        onIndexChange={next => page.set(next)}
        data-testid="viewer"
      />
    </view>
  );
}
```

### One picture

```tsx
import { useLocal } from '@lingxia/view';
import { Thumbnail } from '@liulidesign/core/Thumbnail';
import { Lightbox } from '@liulidesign/core/Lightbox';

const photo = (n: number) => `public/photos/photo-${n}.png`;

/** One picture: no count and no dots. */
export function Single() {
  const open = useLocal('one', false);
  return (
    <view className="items-start">
      <Thumbnail src={photo(5)} alt="Dunes at noon" size="lg" ratio="landscape" onClick={() => open.set(true)} data-testid="one-open" />
      <Lightbox isOpen={open.value} onOpenChange={next => open.set(next)} label="Dunes at noon" images={[{ src: photo(5), alt: 'Dunes at noon' }]} pageLabel={() => ''} data-testid="one" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<number>` |  | A View-local cell holding which picture is in view, from 0. Set it before opening to start there. |
| `index` | `number` |  | The picture in view, from 0, held by the caller: in Logic state, or to open at the one pressed. |
| `onIndexChange` | `((index: number) => void)` |  | Runs with the picture a swipe or a dot moves to. |
| `isOpen` (required) | `boolean` |  | Whether it is shown. A closed lightbox renders nothing. |
| `onOpenChange` (required) | `(isOpen: boolean) => void` |  | Called with `false` on a close request: Escape, back, the close button. |
| `label` (required) | `string` |  | The lightbox's accessible name: "Photos of the house". |
| `images` (required) | `readonly LightboxImage[]` |  |  |
| `defaultIndex` | `number` | `0` | The picture it opens on when the lightbox holds it. |
| `closeLabel` | `string` |  | The close button's name: "Close". Defaults to "Close" in the page's language. |
| `pageLabel` | `((page: number, count: number) => string)` |  | Where the viewer is, written by the app: `(page, count) => \`${page + 1} of ${count}\``. Shown at the top, and each dot's name. Defaults to "2 of 5" in the page's language. |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| panel | `<dialog>` | yes | The dialog primitive, covering the screen on the opaque media canvas. |
| bar | `<view>` | yes | Where the viewer is, and the close button. |
| scroll | `<scroll>` | yes | The pictures side by side, stopping on each. |
| image | `<image>` | yes | A picture, shown whole. |
| caption | `<text>` | no | The caption of the picture in view. |

## What an app may override

- No class props: it takes its look from its props, the tokens and the roles below.
- The recipe, `lightboxRecipe` from `@liulidesign/core/Lightbox/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`.
- Colour roles it paints with, set by the theme pack: `media-canvas`, `on-media`.

## Accessibility

- Pattern: dialog.
- Name: label.
- Description: none.
- Escape: Closes it.
- Tab: Between the close button and the dots.
- Enter or Space on a dot: Shows its picture.
- Press target: md 44 px.
- A dialog named by `label`; assistive tech reads only the lightbox while it is open, and focus returns to what opened it.
- The pictures are a list read by their `alt`; the place ("2 of 4") is a status, said when it changes.
- A tap on the dark surface does not close it: a missed swipe would.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| controls and caption | `on-media` on `media-canvas` | 4.5:1 | 19.56:1 | 19.56:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Open it at the picture that was pressed: set the bound page, then open.
- Do: Write a caption only where it adds to the picture.
- Don't: Use it for a document or a video; it shows pictures.
- Don't: Put actions that change the picture inside it; it is for looking.

## Limits

- No pinch to zoom and no pan: the View has no pinch gesture.
- No previous and next arrows; on a desktop the dots move between pictures.

Related: Thumbnail, Carousel, Dialog.
