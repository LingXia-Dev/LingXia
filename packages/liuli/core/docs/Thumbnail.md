<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Thumbnail

A small picture that stands for a bigger one. Status: preview.

```ts
import { Thumbnail } from '@liulidesign/core/Thumbnail';
```

Thumbnail crops a picture to a small rounded tile: square, 4:3 or 16:9. On its own it is an image; with `onClick` it is a button named by `alt` that opens the picture, usually in a Lightbox. A faint tint holds its place until the picture loads.

## Examples

### A grid of photos

```tsx
import { Thumbnail } from '@liulidesign/core/Thumbnail';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  [1, 'The bay at sunrise'],
  [2, 'Fields behind the house'],
  [3, 'Dusk over the ridge'],
  [4, 'Morning fog in the valley'],
  [5, 'Dunes at noon'],
  [1, 'The bay, a minute later'],
] as const;

/** A grid of photos, each as wide as its column. */
export function Grid() {
  return (
    <view className="grid grid-cols-3 gap-2">
      {photos.map(([n, alt], index) => <Thumbnail key={alt} src={photo(n)} alt={alt} size="fill" onClick={() => {}} data-testid={`grid-${index}`} />)}
    </view>
  );
}
```

### At the start of a row

```tsx
import { Item, ItemList } from '@liulidesign/core/Item';
import { Thumbnail } from '@liulidesign/core/Thumbnail';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  [1, 'The bay at sunrise'],
  [2, 'Fields behind the house'],
  [3, 'Dusk over the ridge'],
  [4, 'Morning fog in the valley'],
  [5, 'Dunes at noon'],
  [1, 'The bay, a minute later'],
] as const;

/** At the start of a row, for the thing the row is about. */
export function InRows() {
  return (
    <ItemList label="Albums">
      <Item label="Coast, November" description="48 photos" startContent={<Thumbnail src={photo(1)} alt="" size="sm" data-testid="row" />} hasChevron onClick={() => {}} />
      <Item label="The valley" description="112 photos" startContent={<Thumbnail src={photo(4)} alt="" size="sm" />} hasChevron onClick={() => {}} />
    </ItemList>
  );
}
```

### Sizes and ratios

```tsx
import { Thumbnail } from '@liulidesign/core/Thumbnail';

const photo = (n: number) => `public/photos/photo-${n}.png`;

export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-end gap-3">
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" size="sm" />
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" />
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" size="lg" />
      <Thumbnail src={photo(3)} alt="Dusk over the ridge" size="lg" ratio="landscape" />
    </view>
  );
}
```

### A strip with one selected

```tsx
import { Thumbnail } from '@liulidesign/core/Thumbnail';

const photo = (n: number) => `public/photos/photo-${n}.png`;
const photos = [
  [1, 'The bay at sunrise'],
  [2, 'Fields behind the house'],
  [3, 'Dusk over the ridge'],
  [4, 'Morning fog in the valley'],
  [5, 'Dunes at noon'],
  [1, 'The bay, a minute later'],
] as const;

/** A strip under a picture: the one in view is ringed. */
export function Strip() {
  return (
    <view className="flex-row gap-3 p-1">
      {photos.slice(0, 4).map(([n, alt], index) => <Thumbnail key={alt} src={photo(n)} alt={alt} size="sm" isSelected={index === 1} onClick={() => {}} data-testid={`strip-${index}`} />)}
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `src` (required) | `string` |  |  |
| `alt` (required) | `string` |  | What the picture shows. With `onClick` it is the button's name. |
| `size` | `ThumbnailSize` | `'md'` | 48, 72 or 96 px wide; `fill` takes the width it is given, as in a grid. |
| `ratio` | `ThumbnailRatio` | `'square'` | 1:1, 4:3 or 16:9; the picture is cropped to fill it. |
| `onClick` | `(() => void)` |  | Makes it a button: opens the picture, usually in a Lightbox. |
| `isSelected` | `boolean` | `false` | Marks it as the one in view among several. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The rounded tile: a view, or a button with `onClick`. |
| image | `<image>` | yes | The picture, cropped to fill the tile. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `thumbnailRecipe` from `@liulidesign/core/Thumbnail/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `focus`, `text`.

## Accessibility

- Pattern: none.
- Name: alt.
- Description: none.
- Tab: Moves to a thumbnail that opens.
- Enter or Space: Opens it.
- Press target: sm 48 px.
- Without `onClick` it is an image read by `alt`; pass an empty `alt` when the name is already beside it, as in a row.
- With `onClick` it is a button named by `alt`; the selected one reports itself as selected.
- The selected ring is a shape, not only a colour.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| selected ring | `accent` on `surface` | 3:1 | 5.62:1 | 6.19:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write `alt` for what the picture shows, not "photo" or its file name.
- Do: Use `fill` in a grid so the columns set the width.
- Don't: Use a thumbnail for a person; that is an Avatar.
- Don't: Go below the small size for one that opens; it is the smallest a finger finds.

## Limits

- A picture that fails to load leaves the tint; there is no broken-image mark.

Related: Lightbox, Carousel, Avatar.
