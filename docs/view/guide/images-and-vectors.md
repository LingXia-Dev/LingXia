# Images and vectors

```tsx
import { useLocal, type VectorPath } from '@lingxia/ui';

const icons: Record<'chevron' | 'star', VectorPath[]> = {
  chevron: [{ d: 'M9 6l6 6-6 6', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' }],
  star: [{ d: 'M12 2l2.9 6.9 7.1.6-5.4 4.7 1.7 7-6.3-3.9-6.3 3.9 1.7-7L2 9.5l7.1-.6z', fill: 'currentColor' }],
};

function Icon({ name, className }: { name: keyof typeof icons; className?: string }) {
  return <vector accessibilityHidden paths={icons[name]} className={className} />;
}

export function render() {
  const attempt = useLocal('attempt', 0);
  const failed = useLocal('failed', false);
  return (
    <page className="p-4 gap-4">
      <stack className="w-full aspect-video rounded-xl overflow-hidden bg-neutral-200">
        <image
          src="https://images.example.com/cover.jpg"
          retryKey={String(attempt.value)}
          alt="Harbor at dusk"
          className="w-full h-full object-cover"
          onLoad={() => failed.set(false)}
          onError={() => failed.set(true)}
        />
        {failed.value && (
          <button onClick={() => attempt.set(attempt.value + 1)} className="self-center justify-self-center rounded-full bg-white text-neutral-900">
            Retry
          </button>
        )}
      </stack>
      <row className="items-center gap-3">
        <image src="images/avatar.png" alt="" className="w-10 h-10 rounded-full" />
        <text className="grow text-sm">Mina Park</text>
        <Icon name="star" className="w-5 h-5 text-amber-500" />
        <Icon name="chevron" className="w-5 h-5 text-neutral-400 rtl:-scale-x-100" />
      </row>
    </page>
  );
}
```

## `<image>`

`src` is one of:

- a path inside the lxapp package, without a leading `/` (`images/avatar.png`);
- an `lx://` URI the platform APIs hand you for managed files;
- an `http://` or `https://` URL whose host the lxapp is allowed to reach.

PNG and JPEG decode. `data:` URLs and SVG files are not supported; draw vector art with `<vector>`.
A resource may be up to 8 MiB encoded and its `src` up to 8 KiB.

Remote images go through the host's HTTP cache: they revalidate like any HTTP resource, and a cached
copy keeps showing when the device is offline.

### Size and fit

An unsized image is 64 × 64. Give it a size, or one dimension plus `aspect-*`. `object-cover` (fill and
crop, the default), `object-contain` (fit inside) and `object-fill` (stretch) set how the picture fills its box;
`object-position` is not supported. Round or clip it with `rounded-*` and `overflow-hidden` on the image
or a parent. Images decode at the size they are painted, so a thumbnail of a large photo costs a
thumbnail's memory.

### Loading and errors

Loads are asynchronous. `onLoad` and `onError` fire once per `(src, retryKey)` pair, after the result
is on screen. A failed image stays failed until that pair changes: bump `retryKey` to try again.
A thrown error inside these callbacks is reported without failing the frame.

A page paints at most 64 MiB of decoded images at once; an image past that budget gets `onError` and a
placeholder. Long image lists should be virtual (`<List>`), which keeps only visible rows alive.

### Accessibility

`alt` names the image. `alt=""` marks it decorative and hides it from accessibility. Leaving `alt` out
keeps the image exposed without a name.

## `<vector>`

`<vector>` draws SVG path data natively. It is a leaf, 24 × 24 until sized.

| Prop | Meaning |
| --- | --- |
| `paths` | 1–32 paths: `{ d, fill, stroke, strokeWidth, strokeDasharray, strokeDashoffset, strokeLinecap, strokeLinejoin, transform, vectorEffect }` |
| `viewBox` | `[x, y, width, height]`, default `[0, 0, 24, 24]` |
| `gradients` | up to 8 named SVG gradients (`linear` or `radial`, up to 16 stops), used as `fill: 'url(#id)'` |

`fill` and `stroke` take a hex color, `currentColor`, `none` or `url(#id)`. `currentColor` is the
vector's text color, so `text-*` utilities color an icon. `transform` is an SVG transform list
(`translate`, `scale`, `rotate`, `skewX`, `skewY`, `matrix`). `vectorEffect: 'non-scaling-stroke'` keeps
stroke widths constant when the vector is scaled. `object-contain` (default) and `object-fill` map the
`viewBox` into the box.

```tsx
export function render() {
  return (
    <page className="p-4">
      <vector
        className="w-16 h-16"
        paths={[{ d: 'M12 2l2.9 6.9 7.1.6-5.4 4.7 1.7 7-6.3-3.9-6.3 3.9 1.7-7L2 9.5l7.1-.6z', fill: 'url(#gold)', stroke: '#b45309', strokeWidth: 0.75 }]}
        gradients={{ gold: { type: 'radial', fx: 0.35, fy: 0.3, stops: [{ offset: 0, color: '#fff3b0' }, { offset: 1, color: '#f59e0b' }] } }}
      />
    </page>
  );
}
```

There is no `<svg>` element and no SVG file import; convert icons to path data at authoring time.

## Icons

Keep icons as path data in a component, as in the first example:

- Decorative icons, and icons inside a button that has text or a label, take `accessibilityHidden`.
- An icon-only button needs `accessibilityLabel` on the button.
- Icons that point along the reading direction (chevrons, arrows, send) mirror with `rtl:-scale-x-100`.

## Backgrounds

`bg-[url(…)]` is not supported. Put an `<image>` and the content in a `<stack>`: children of a stack share
one cell and paint in order, as in the first example.
