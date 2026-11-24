<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# QRCode

A code a phone camera reads. Status: preview.

```ts
import { fitsQRCode, QRCode } from '@liulidesign/core/QRCode';
```

QRCode draws a value as a QR code for a phone camera: a link to open, a network to join, a code to pair with. It is dark on light in both themes, with its quiet zone inside the tile, as readers expect. The value is encoded in Liuli; nothing is fetched.

## Examples

### A network to join

```tsx
import { QRCode } from '@liulidesign/core/QRCode';

/** A guest network to join: the code, and the same details in words for whoever cannot scan. */
export function Join() {
  return (
    <view className="flex-row items-center gap-4">
      <QRCode value="WIFI:T:WPA;S:Harbour Café Guest;P:tide-pool-73;;" label="Code to join Harbour Café Guest" data-testid="join" />
      <view className="shrink min-w-0 gap-1">
        <text className="text-callout font-semibold text-text">Harbour Café Guest</text>
        <text className="text-footnote text-muted">Password tide-pool-73</text>
      </view>
    </view>
  );
}
```

### A link to open

```tsx
import { QRCode } from '@liulidesign/core/QRCode';

/** A link to open on a phone, with the address under it. */
export function Link() {
  return (
    <view className="items-start gap-2">
      <QRCode value="https://example.com/invite/7Q4M" label="Code for the invite link" size="sm" data-testid="invite" />
      <text className="text-footnote text-muted">example.com/invite/7Q4M</text>
    </view>
  );
}
```

### The three sizes

```tsx
import { QRCode } from '@liulidesign/core/QRCode';

/** The three sizes. */
export function Sizes() {
  return (
    <view className="flex-row flex-wrap items-end gap-4">
      <QRCode value="https://example.com" label="Small code" size="sm" />
      <QRCode value="https://example.com" label="Medium code" />
      <QRCode value="https://example.com" label="Large code" size="lg" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `value` (required) | `string` |  | What the code holds: a link, a join code, Wi-Fi details. Text of any language. |
| `label` (required) | `string` |  | What the code is for, read in its place: "Code to join Harbour Café Guest". |
| `size` | `QRCodeSize` | `'md'` | 96, 160 or 240 points square. |
| `level` | `QRLevel` | `'M'` | How much of the code may be lost and still read: L 7 %, M 15 %, Q 25 %, H 30 %. Higher levels make denser codes. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The square tile: an image named by `label`. |
| code | `<vector>` | yes | The modules, dark on light, with a four-module quiet zone. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `qrCodeRecipe` from `@liulidesign/core/QRCode/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--radius-control`.
- Colour roles it paints with, set by the theme pack: `text`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Press target: .
- It is an image named by `label`, which says what the code is for; its value is not read.
- Put the value in words beside it when a person may need to type it instead.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say what scanning does in `label`: "Code to join Harbour Café Guest".
- Do: Show the link or password beside the code for whoever cannot scan.
- Do: Check a value that comes from data with `fitsQRCode` and show it as text when it does not fit.
- Don't: Draw it smaller than `sm`; phone cameras read a 96 pt code at arm's length.
- Don't: Raise `level` without a reason; each step makes the code denser.

## Limits

- Up to version 15: 520 bytes at level L, 412 at M, 292 at Q, 220 at H. A longer value draws an empty tile and warns once; `fitsQRCode(value, level)` answers before rendering.
- Byte mode only, so digits and capitals take as much room as any text.
- No logo in the middle.

Related: Thumbnail, Code.
