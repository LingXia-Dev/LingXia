_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Utilities

Every utility of Tailwind's default theme, run through the native lowering and grouped by family.
Your `@theme` tokens add names to the same families (`bg-primary`, `rounded-card`); arbitrary values
(`w-[72px]`, `grid-cols-[1fr_auto]`) follow the rules of the family they extend. A supported utility
is still refused on an element that does not render its style ([styles.md](styles.md)).

## Layout

| Family | Supported | Refused |
| --- | --- | --- |
| `block` | `block` | block-size (71) |
| `flex` | `flex`, `flex-1`, `flex-2`, `flex-3`, `flex-4`, … (23) | unsupported value (26) |
| `grid` | `grid`, `grid-flow-col`, `grid-flow-col-dense`, `grid-flow-dense`, `grid-flow-row`, `grid-flow-row-dense` |  |
| `hidden` | `hidden` |  |
| `inline` | `inline-block`, `inline-flex` | inline-size (83); display (3) |
| `order` | `order-1`, `order-2`, `order-3`, `order-4`, `order-5`, … (14) | order (12) |
| `overflow` | `overflow-hidden`, `overflow-visible`, `overflow-x-auto`, `overflow-y-auto` | overflow-x (4); overflow-y (4); overflow (3) |

## Position

| Family | Supported | Refused |
| --- | --- | --- |
| `absolute` | `absolute` |  |
| `bottom` | `-bottom-0`, `-bottom-0.5`, `-bottom-1`, `-bottom-1.5`, `-bottom-2`, … (124) | unsupported inset (1) |
| `end` | `-end-full`, `-end-px`, `end-full`, `end-px` | unsupported inset (1) |
| `fixed` | `fixed` |  |
| `inset` | `-inset-0`, `-inset-0.5`, `-inset-1`, `-inset-1.5`, `-inset-2`, … (124) | unsupported inset (1) |
| `inset-e` | `-inset-e-0`, `-inset-e-0.5`, `-inset-e-1`, `-inset-e-1.5`, `-inset-e-2`, … (124) | unsupported inset (1) |
| `inset-s` | `-inset-s-0`, `-inset-s-0.5`, `-inset-s-1`, `-inset-s-1.5`, `-inset-s-2`, … (124) | unsupported inset (1) |
| `inset-x` | `-inset-x-0`, `-inset-x-0.5`, `-inset-x-1`, `-inset-x-1.5`, `-inset-x-2`, … (124) | unsupported inset (1) |
| `inset-y` | `-inset-y-0`, `-inset-y-0.5`, `-inset-y-1`, `-inset-y-1.5`, `-inset-y-2`, … (124) | unsupported inset (1) |
| `left` | `-left-0`, `-left-0.5`, `-left-1`, `-left-1.5`, `-left-2`, … (124) | unsupported inset (1) |
| `relative` | `relative` |  |
| `right` | `-right-0`, `-right-0.5`, `-right-1`, `-right-1.5`, `-right-2`, … (124) | unsupported inset (1) |
| `start` | `-start-full`, `-start-px`, `start-full`, `start-px` | unsupported inset (1) |
| `sticky` | `sticky` |  |
| `top` | `-top-0`, `-top-0.5`, `-top-1`, `-top-1.5`, `-top-2`, … (124) | unsupported inset (1) |
| `z` | `z-0`, `z-10`, `z-20`, `z-30`, `z-40`, `z-50` | z-index (7) |

## Flex and grid items

| Family | Supported | Refused |
| --- | --- | --- |
| `basis` | `basis-0`, `basis-0.5`, `basis-1`, `basis-1.5`, `basis-2`, … (76) |  |
| `col` | `col-auto` |  |
| `col-end` | `-col-end-1`, `-col-end-2`, `-col-end-3`, `-col-end-4`, `-col-end-5`, … (27) |  |
| `col-span` | `col-span-1`, `col-span-2`, `col-span-3`, `col-span-4`, `col-span-5`, … (13) |  |
| `col-start` | `-col-start-1`, `-col-start-2`, `-col-start-3`, `-col-start-4`, `-col-start-5`, … (27) |  |
| `grow` | `grow`, `grow-0` |  |
| `row` | `row-auto` |  |
| `row-end` | `-row-end-1`, `-row-end-2`, `-row-end-3`, `-row-end-4`, `-row-end-5`, … (27) |  |
| `row-span` | `row-span-1`, `row-span-2`, `row-span-3`, `row-span-4`, `row-span-5`, … (13) |  |
| `row-start` | `-row-start-1`, `-row-start-2`, `-row-start-3`, `-row-start-4`, `-row-start-5`, … (27) |  |
| `self` | `self-baseline`, `self-center`, `self-end`, `self-start`, `self-stretch` | align-self (4) |
| `shrink` | `shrink`, `shrink-0` |  |

## Flex and grid containers

| Family | Supported | Refused |
| --- | --- | --- |
| `auto-cols` | `auto-cols-auto`, `auto-cols-fr`, `auto-cols-max`, `auto-cols-min` |  |
| `auto-rows` | `auto-rows-auto`, `auto-rows-fr`, `auto-rows-max`, `auto-rows-min` |  |
| `content` | `content-around`, `content-between`, `content-center`, `content-end`, `content-evenly`, … (7) | align-content (4); content (1) |
| `gap` | `gap-0`, `gap-0.5`, `gap-1`, `gap-1.5`, `gap-2`, … (35) |  |
| `gap-x` | `gap-x-0`, `gap-x-0.5`, `gap-x-1`, `gap-x-1.5`, `gap-x-2`, … (35) |  |
| `gap-y` | `gap-y-0`, `gap-y-0.5`, `gap-y-1`, `gap-y-1.5`, `gap-y-2`, … (35) |  |
| `grid-cols` | `grid-cols-1`, `grid-cols-2`, `grid-cols-3`, `grid-cols-4`, `grid-cols-5`, … (13) | grid-template-columns (1) |
| `grid-rows` | `grid-rows-1`, `grid-rows-2`, `grid-rows-3`, `grid-rows-4`, `grid-rows-5`, … (13) | grid-template-rows (1) |
| `items` | `items-baseline`, `items-center`, `items-end`, `items-start`, `items-stretch` | align-items (3) |
| `justify` | `justify-around`, `justify-between`, `justify-center`, `justify-end`, `justify-evenly`, … (14) | justify-content (5); justify-items (3); justify-self (3) |
| `place` | `place-content-around`, `place-content-between`, `place-content-center`, `place-content-end`, `place-content-evenly`, … (15) | align-content (3); align-self (3); align-items (2) |

## Sizing

| Family | Supported | Refused |
| --- | --- | --- |
| `aspect` | `aspect-auto`, `aspect-square`, `aspect-video` |  |
| `h` | `h-0`, `h-0.5`, `h-1`, `h-1.5`, `h-2`, … (73) | unsupported value (1) |
| `max-h` | `max-h-0`, `max-h-0.5`, `max-h-1`, `max-h-1.5`, `max-h-2`, … (72) | unsupported value (2) |
| `max-w` | `max-w-0`, `max-w-0.5`, `max-w-1`, `max-w-1.5`, `max-w-2`, … (85) | unsupported value (1) |
| `min-h` | `min-h-0`, `min-h-0.5`, `min-h-1`, `min-h-1.5`, `min-h-2`, … (73) | unsupported value (1) |
| `min-w` | `min-w-0`, `min-w-0.5`, `min-w-1`, `min-w-1.5`, `min-w-2`, … (86) |  |
| `size` | `size-0`, `size-0.5`, `size-1`, `size-1.5`, `size-2`, … (72) |  |
| `w` | `w-0`, `w-0.5`, `w-1`, `w-1.5`, `w-2`, … (86) |  |

## Margin

| Family | Supported | Refused |
| --- | --- | --- |
| `m` | `-m-0`, `-m-0.5`, `-m-1`, `-m-1.5`, `-m-2`, … (71) |  |
| `mb` | `-mb-0`, `-mb-0.5`, `-mb-1`, `-mb-1.5`, `-mb-2`, … (71) |  |
| `mbe` | `-mbe-0`, `-mbe-0.5`, `-mbe-1`, `-mbe-1.5`, `-mbe-2`, … (71) |  |
| `mbs` | `-mbs-0`, `-mbs-0.5`, `-mbs-1`, `-mbs-1.5`, `-mbs-2`, … (71) |  |
| `me` | `-me-0`, `-me-0.5`, `-me-1`, `-me-1.5`, `-me-2`, … (71) |  |
| `ml` | `-ml-0`, `-ml-0.5`, `-ml-1`, `-ml-1.5`, `-ml-2`, … (71) |  |
| `mr` | `-mr-0`, `-mr-0.5`, `-mr-1`, `-mr-1.5`, `-mr-2`, … (71) |  |
| `ms` | `-ms-0`, `-ms-0.5`, `-ms-1`, `-ms-1.5`, `-ms-2`, … (71) |  |
| `mt` | `-mt-0`, `-mt-0.5`, `-mt-1`, `-mt-1.5`, `-mt-2`, … (71) |  |
| `mx` | `-mx-0`, `-mx-0.5`, `-mx-1`, `-mx-1.5`, `-mx-2`, … (71) |  |
| `my` | `-my-0`, `-my-0.5`, `-my-1`, `-my-1.5`, `-my-2`, … (71) |  |
| `space-x` | `-space-x-0`, `-space-x-0.5`, `-space-x-1`, `-space-x-1.5`, `-space-x-2`, … (70) | reversed space/divide order (1) |
| `space-y` | `-space-y-0`, `-space-y-0.5`, `-space-y-1`, `-space-y-1.5`, `-space-y-2`, … (70) | reversed space/divide order (1) |

## Padding

| Family | Supported | Refused |
| --- | --- | --- |
| `p` | `p-0`, `p-0.5`, `p-1`, `p-1.5`, `p-2`, … (35) |  |
| `pb` | `pb-0`, `pb-0.5`, `pb-1`, `pb-1.5`, `pb-2`, … (35) |  |
| `pbe` | `pbe-0`, `pbe-0.5`, `pbe-1`, `pbe-1.5`, `pbe-2`, … (35) |  |
| `pbs` | `pbs-0`, `pbs-0.5`, `pbs-1`, `pbs-1.5`, `pbs-2`, … (35) |  |
| `pe` | `pe-0`, `pe-0.5`, `pe-1`, `pe-1.5`, `pe-2`, … (35) |  |
| `pl` | `pl-0`, `pl-0.5`, `pl-1`, `pl-1.5`, `pl-2`, … (35) |  |
| `pr` | `pr-0`, `pr-0.5`, `pr-1`, `pr-1.5`, `pr-2`, … (35) |  |
| `ps` | `ps-0`, `ps-0.5`, `ps-1`, `ps-1.5`, `ps-2`, … (35) |  |
| `pt` | `pt-0`, `pt-0.5`, `pt-1`, `pt-1.5`, `pt-2`, … (35) |  |
| `px` | `px-0`, `px-0.5`, `px-1`, `px-1.5`, `px-2`, … (35) |  |
| `py` | `py-0`, `py-0.5`, `py-1`, `py-1.5`, `py-2`, … (35) |  |

## Typography

| Family | Supported | Refused |
| --- | --- | --- |
| `break` | `break-all`, `break-keep`, `break-normal` | break-after (8); break-before (8); break-inside (4) |
| `capitalize` | `capitalize` |  |
| `diagonal` | `diagonal-fractions` |  |
| `font` | `font-black`, `font-bold`, `font-extrabold`, `font-extralight`, `font-light`, … (11) | unsupported font-family (1) |
| `italic` | `italic` |  |
| `leading` | `leading-0`, `leading-0.5`, `leading-1`, `leading-1.5`, `leading-2`, … (41) |  |
| `line` | `line-through` |  |
| `line-clamp` | `line-clamp-1`, `line-clamp-2`, `line-clamp-3`, `line-clamp-4`, `line-clamp-5`, `line-clamp-6` | -webkit-box-orient (1) |
| `lining` | `lining-nums` |  |
| `lowercase` | `lowercase` |  |
| `no` | `no-underline` |  |
| `normal` | `normal-case`, `normal-nums` |  |
| `not` | `not-italic` | position (1) |
| `oldstyle` | `oldstyle-nums` |  |
| `ordinal` | `ordinal` |  |
| `proportional` | `proportional-nums` |  |
| `select` | `select-all`, `select-none`, `select-text` | select-auto (1) |
| `slashed` | `slashed-zero` |  |
| `stacked` | `stacked-fractions` |  |
| `tabular` | `tabular-nums` |  |
| `text` | `text-2xl`, `text-3xl`, `text-4xl`, `text-5xl`, `text-6xl`, … (310) | text-wrap (4); unsupported color (2) |
| `tracking` | `tracking-normal`, `tracking-tight`, `tracking-tighter`, `tracking-wide`, `tracking-wider`, `tracking-widest` | unsupported length (6) |
| `truncate` | `truncate` |  |
| `underline` | `underline` |  |
| `uppercase` | `uppercase` |  |
| `whitespace` | `whitespace-normal`, `whitespace-nowrap`, `whitespace-pre`, `whitespace-pre-line`, `whitespace-pre-wrap` | white-space (1) |
| `wrap` | `wrap-anywhere`, `wrap-break-word`, `wrap-normal` |  |

## Backgrounds

| Family | Supported | Refused |
| --- | --- | --- |
| `bg` | `bg-amber-50`, `bg-amber-100`, `bg-amber-200`, `bg-amber-300`, `bg-amber-400`, … (291) | background-blend-mode (16); background-position (9); background-repeat (6) |
| `bg-conic` | `bg-conic`, `bg-conic-0`, `bg-conic-30`, `bg-conic-60`, `bg-conic-90`, … (13) | unsupported gradient shape (12) |
| `bg-linear` | `bg-linear-0`, `bg-linear-30`, `bg-linear-60`, `bg-linear-90`, `bg-linear-120`, … (20) | unsupported gradient shape (12) |
| `bg-radial` | `bg-radial` |  |
| `from` | `from-0%`, `from-5%`, `from-10%`, `from-15%`, `from-20%`, … (310) | unsupported color (2) |
| `to` | `to-0%`, `to-5%`, `to-10%`, `to-15%`, `to-20%`, … (310) | unsupported color (2) |
| `via` | `via-0%`, `via-5%`, `via-10%`, `via-15%`, `via-20%`, … (311) | unsupported color (2) |

## Radius

| Family | Supported | Refused |
| --- | --- | --- |
| `rounded` | `rounded-2xl`, `rounded-3xl`, `rounded-4xl`, `rounded-full`, `rounded-lg`, … (10) |  |
| `rounded-b` | `rounded-b-2xl`, `rounded-b-3xl`, `rounded-b-4xl`, `rounded-b-full`, `rounded-b-lg`, … (10) |  |
| `rounded-bl` | `rounded-bl-2xl`, `rounded-bl-3xl`, `rounded-bl-4xl`, `rounded-bl-full`, `rounded-bl-lg`, … (10) |  |
| `rounded-br` | `rounded-br-2xl`, `rounded-br-3xl`, `rounded-br-4xl`, `rounded-br-full`, `rounded-br-lg`, … (10) |  |
| `rounded-e` | `rounded-e-2xl`, `rounded-e-3xl`, `rounded-e-4xl`, `rounded-e-full`, `rounded-e-lg`, … (10) |  |
| `rounded-ee` | `rounded-ee-2xl`, `rounded-ee-3xl`, `rounded-ee-4xl`, `rounded-ee-full`, `rounded-ee-lg`, … (10) |  |
| `rounded-es` | `rounded-es-2xl`, `rounded-es-3xl`, `rounded-es-4xl`, `rounded-es-full`, `rounded-es-lg`, … (10) |  |
| `rounded-l` | `rounded-l-2xl`, `rounded-l-3xl`, `rounded-l-4xl`, `rounded-l-full`, `rounded-l-lg`, … (10) |  |
| `rounded-r` | `rounded-r-2xl`, `rounded-r-3xl`, `rounded-r-4xl`, `rounded-r-full`, `rounded-r-lg`, … (10) |  |
| `rounded-s` | `rounded-s-2xl`, `rounded-s-3xl`, `rounded-s-4xl`, `rounded-s-full`, `rounded-s-lg`, … (10) |  |
| `rounded-se` | `rounded-se-2xl`, `rounded-se-3xl`, `rounded-se-4xl`, `rounded-se-full`, `rounded-se-lg`, … (10) |  |
| `rounded-ss` | `rounded-ss-2xl`, `rounded-ss-3xl`, `rounded-ss-4xl`, `rounded-ss-full`, `rounded-ss-lg`, … (10) |  |
| `rounded-t` | `rounded-t-2xl`, `rounded-t-3xl`, `rounded-t-4xl`, `rounded-t-full`, `rounded-t-lg`, … (10) |  |
| `rounded-tl` | `rounded-tl-2xl`, `rounded-tl-3xl`, `rounded-tl-4xl`, `rounded-tl-full`, `rounded-tl-lg`, … (10) |  |
| `rounded-tr` | `rounded-tr-2xl`, `rounded-tr-3xl`, `rounded-tr-4xl`, `rounded-tr-full`, `rounded-tr-lg`, … (10) |  |

## Borders

| Family | Supported | Refused |
| --- | --- | --- |
| `border` | `border`, `border-0`, `border-2`, `border-4`, `border-8`, … (298) | border-style (3); border-collapse (2); unsupported color (1) |
| `border-b` | `border-b`, `border-b-0`, `border-b-2`, `border-b-4`, `border-b-8` | border-bottom-color (291) |
| `border-be` | `border-be`, `border-be-0`, `border-be-2`, `border-be-4`, `border-be-8` | border-block-end-color (291) |
| `border-bs` | `border-bs`, `border-bs-0`, `border-bs-2`, `border-bs-4`, `border-bs-8` | border-block-start-color (291) |
| `border-e` | `border-e`, `border-e-0`, `border-e-2`, `border-e-4`, `border-e-8` | border-inline-end-color (291) |
| `border-l` | `border-l`, `border-l-0`, `border-l-2`, `border-l-4`, `border-l-8` | border-left-color (291) |
| `border-r` | `border-r`, `border-r-0`, `border-r-2`, `border-r-4`, `border-r-8` | border-right-color (291) |
| `border-s` | `border-s`, `border-s-0`, `border-s-2`, `border-s-4`, `border-s-8` | border-inline-start-color (291) |
| `border-t` | `border-t`, `border-t-0`, `border-t-2`, `border-t-4`, `border-t-8` | border-top-color (291) |
| `border-x` | `border-x`, `border-x-0`, `border-x-2`, `border-x-4`, `border-x-8` | border-inline-color (291) |
| `border-y` | `border-y`, `border-y-0`, `border-y-2`, `border-y-4`, `border-y-8` | border-block-color (291) |
| `divide` | `divide-amber-50`, `divide-amber-100`, `divide-amber-200`, `divide-amber-300`, `divide-amber-400`, … (293) | border-style (2); unsupported color (1) |
| `divide-x` | `divide-x`, `divide-x-0`, `divide-x-2`, `divide-x-4`, `divide-x-8` | reversed space/divide order (1) |
| `divide-y` | `divide-y`, `divide-y-0`, `divide-y-2`, `divide-y-4`, `divide-y-8` | reversed space/divide order (1) |

## Effects

| Family | Supported | Refused |
| --- | --- | --- |
| `backdrop-blur` | `backdrop-blur-2xl`, `backdrop-blur-3xl`, `backdrop-blur-lg`, `backdrop-blur-md`, `backdrop-blur-none`, … (8) |  |
| `backdrop-saturate` | `backdrop-saturate-0`, `backdrop-saturate-50`, `backdrop-saturate-100`, `backdrop-saturate-150`, `backdrop-saturate-200` |  |
| `opacity` | `opacity-0`, `opacity-5`, `opacity-10`, `opacity-15`, `opacity-20`, … (21) |  |
| `ring` | `ring`, `ring-0`, `ring-1`, `ring-2`, `ring-4`, … (295) | unsupported color (2); unsupported shadow property (1) |
| `shadow` | `shadow`, `shadow-2xl`, `shadow-2xs`, `shadow-amber-50`, `shadow-amber-100`, … (298) | unsupported color (3); inset shadows are not supported yet (1) |

## Transforms

| Family | Supported | Refused |
| --- | --- | --- |
| `origin` | `origin-bottom`, `origin-bottom-left`, `origin-bottom-right`, `origin-center`, `origin-left`, … (9) |  |
| `rotate` | `-rotate-0`, `-rotate-1`, `-rotate-2`, `-rotate-3`, `-rotate-6`, … (19) |  |
| `scale` | `-scale-0`, `-scale-50`, `-scale-75`, `-scale-90`, `-scale-95`, … (22) | scale-z (1); unsupported value (1) |
| `scale-x` | `-scale-x-0`, `-scale-x-50`, `-scale-x-75`, `-scale-x-90`, `-scale-x-95`, … (22) |  |
| `scale-y` | `-scale-y-0`, `-scale-y-50`, `-scale-y-75`, `-scale-y-90`, `-scale-y-95`, … (22) |  |
| `translate` | `-translate-0`, `-translate-0.5`, `-translate-1`, `-translate-1.5`, `-translate-2`, … (124) | translate-z (1); unsupported value (1) |
| `translate-x` | `-translate-x-0`, `-translate-x-0.5`, `-translate-x-1`, `-translate-x-1.5`, `-translate-x-2`, … (124) |  |
| `translate-y` | `-translate-y-0`, `-translate-y-0.5`, `-translate-y-1`, `-translate-y-1.5`, `-translate-y-2`, … (124) |  |

## Scrolling

| Family | Supported | Refused |
| --- | --- | --- |
| `overscroll` | `overscroll-auto`, `overscroll-contain`, `overscroll-none`, `overscroll-x-auto`, `overscroll-x-contain`, … (9) |  |

## Editors

| Family | Supported | Refused |
| --- | --- | --- |
| `placeholder` | `placeholder-amber-50`, `placeholder-amber-100`, `placeholder-amber-200`, `placeholder-amber-300`, `placeholder-amber-400`, … (289) | unsupported color (2) |

## Images and vectors

| Family | Supported | Refused |
| --- | --- | --- |
| `object` | `object-bottom`, `object-bottom-left`, `object-bottom-right`, `object-center`, `object-contain`, … (12) | object-fit (2) |

## Gated: motion

Refused unless the page opts into the gated motion lowering (see the motion chapter):

`animate` (5), `delay` (8), `duration` (9), `ease` (5)

## Not supported

| Family | Utilities | Reason |
| --- | --- | --- |
| `@container` | 2 | container-type (2) |
| `accent` | 292 | accent-color (292) |
| `align` | 8 | vertical-align (8) |
| `antialiased` | 1 | -webkit-font-smoothing (1) |
| `appearance` | 2 | appearance (2) |
| `backdrop-brightness` | 11 | unsupported backdrop property (11) |
| `backdrop-contrast` | 7 | unsupported backdrop property (7) |
| `backdrop-grayscale` | 6 | unsupported backdrop property (6) |
| `backdrop-hue-rotate` | 6 | unsupported backdrop property (6) |
| `backdrop-invert` | 6 | unsupported backdrop property (6) |
| `backdrop-opacity` | 21 | unsupported backdrop property (21) |
| `backdrop-sepia` | 4 | unsupported backdrop property (4) |
| `backface` | 2 | backface-visibility (2) |
| `blur` | 8 | filters (8) |
| `border-spacing` | 35 | border-spacing (35) |
| `border-spacing-x` | 35 | border-spacing (35) |
| `border-spacing-y` | 35 | border-spacing (35) |
| `box` | 4 | -webkit-box-decoration-break (2); box-sizing (2) |
| `brightness` | 11 | filters (11) |
| `caption` | 2 | caption-side (2) |
| `caret` | 291 | caret-color (291) |
| `clear` | 6 | clear (6) |
| `collapse` | 1 | visibility (1) |
| `columns` | 26 | columns (26) |
| `contain` | 8 | contain (8) |
| `container` | 1 | nested rule @media (1) |
| `contents` | 1 | display (1) |
| `contrast` | 7 | filters (7) |
| `cursor` | 36 | cursor (36) |
| `decoration` | 301 | text-decoration-color (291); text-decoration-style (5); text-decoration-thickness (5) |
| `drop-shadow` | 298 | drop-shadow colors (291); filters (7) |
| `field` | 2 | field-sizing (2) |
| `fill` | 292 | fill (292) |
| `float` | 5 | float (5) |
| `flow` | 1 | display (1) |
| `font-stretch` | 19 | font-stretch (19) |
| `forced` | 2 | forced-color-adjust (2) |
| `grayscale` | 6 | filters (6) |
| `hue-rotate` | 6 | filters (6) |
| `hyphens` | 3 | -webkit-hyphens (3) |
| `indent` | 70 | text-indent (70) |
| `inset-be` | 125 | inset-block-end (125) |
| `inset-bs` | 125 | inset-block-start (125) |
| `inset-ring` | 297 | unsupported shadow property (297) |
| `inset-shadow` | 296 | unsupported shadow property (296) |
| `invert` | 6 | filters (6) |
| `invisible` | 1 | visibility (1) |
| `isolate` | 1 | isolation (1) |
| `isolation` | 1 | isolation (1) |
| `list` | 6 | list-style-type (3); list-style-position (2); display (1) |
| `list-image` | 1 | list-style-image (1) |
| `mask` | 43 | mask-position (9); mask-clip (7); mask-origin (6) |
| `mask-b-from` | 346 | mask-image (346) |
| `mask-b-to` | 346 | mask-image (346) |
| `mask-conic` | 18 | mask-image (18) |
| `mask-conic-from` | 346 | mask-image (346) |
| `mask-conic-to` | 346 | mask-image (346) |
| `mask-l-from` | 346 | mask-image (346) |
| `mask-l-to` | 346 | mask-image (346) |
| `mask-linear` | 18 | mask-image (18) |
| `mask-linear-from` | 346 | mask-image (346) |
| `mask-linear-to` | 346 | mask-image (346) |
| `mask-r-from` | 346 | mask-image (346) |
| `mask-r-to` | 346 | mask-image (346) |
| `mask-radial` | 4 | masks (4) |
| `mask-radial-at` | 9 | masks (9) |
| `mask-radial-from` | 346 | mask-image (346) |
| `mask-radial-to` | 346 | mask-image (346) |
| `mask-t-from` | 346 | mask-image (346) |
| `mask-t-to` | 346 | mask-image (346) |
| `mask-x-from` | 346 | mask-image (346) |
| `mask-x-to` | 346 | mask-image (346) |
| `mask-y-from` | 346 | mask-image (346) |
| `mask-y-to` | 346 | mask-image (346) |
| `max-block` | 71 | max-block-size (71) |
| `max-inline` | 83 | max-inline-size (83) |
| `min-block` | 71 | min-block-size (71) |
| `min-inline` | 83 | min-inline-size (83) |
| `mix` | 18 | mix-blend-mode (18) |
| `outline` | 303 | outline (303) |
| `outline-offset` | 10 | outline (10) |
| `overline` | 1 | text-decoration-line (1) |
| `perspective` | 6 | perspective (6) |
| `perspective-origin` | 9 | perspective-origin (9) |
| `pointer` | 2 | pointer-events (2) |
| `resize` | 4 | resize (4) |
| `ring-offset` | 296 | unsupported shadow property (296) |
| `rotate-x` | 18 | transform (18) |
| `rotate-y` | 18 | transform (18) |
| `rotate-z` | 18 | transform (18) |
| `saturate` | 5 | filters (5) |
| `scale-z` | 22 | scale-z (22) |
| `scheme` | 6 | color-scheme (6) |
| `scroll` | 2 | scroll-behavior (2) |
| `scroll-m` | 70 | scroll-margin (70) |
| `scroll-mb` | 70 | scroll-margin-bottom (70) |
| `scroll-mbe` | 70 | scroll-margin-block-end (70) |
| `scroll-mbs` | 70 | scroll-margin-block-start (70) |
| `scroll-me` | 70 | scroll-margin-inline-end (70) |
| `scroll-ml` | 70 | scroll-margin-left (70) |
| `scroll-mr` | 70 | scroll-margin-right (70) |
| `scroll-ms` | 70 | scroll-margin-inline-start (70) |
| `scroll-mt` | 70 | scroll-margin-top (70) |
| `scroll-mx` | 70 | scroll-margin-inline (70) |
| `scroll-my` | 70 | scroll-margin-block (70) |
| `scroll-p` | 35 | scroll-padding (35) |
| `scroll-pb` | 35 | scroll-padding-bottom (35) |
| `scroll-pbe` | 35 | scroll-padding-block-end (35) |
| `scroll-pbs` | 35 | scroll-padding-block-start (35) |
| `scroll-pe` | 35 | scroll-padding-inline-end (35) |
| `scroll-pl` | 35 | scroll-padding-left (35) |
| `scroll-pr` | 35 | scroll-padding-right (35) |
| `scroll-ps` | 35 | scroll-padding-inline-start (35) |
| `scroll-pt` | 35 | scroll-padding-top (35) |
| `scroll-px` | 35 | scroll-padding-inline (35) |
| `scroll-py` | 35 | scroll-padding-block (35) |
| `scrollbar` | 6 | scrollbar-gutter (3); scrollbar-width (3) |
| `scrollbar-thumb` | 291 | scrollbar-color (291) |
| `scrollbar-track` | 291 | scrollbar-color (291) |
| `sepia` | 4 | filters (4) |
| `skew` | 12 | transform (12) |
| `skew-x` | 12 | transform (12) |
| `skew-y` | 12 | transform (12) |
| `snap` | 12 | scroll-snap-align (4); scroll-snap-type (4); scroll-snap-stop (2) |
| `sr` | 1 | clip-path (1) |
| `static` | 1 | position (1) |
| `stroke` | 296 | stroke (292); stroke-width (4) |
| `subpixel` | 1 | -webkit-font-smoothing (1) |
| `tab` | 3 | tab-size (3) |
| `table` | 11 | display (9); table-layout (2) |
| `text-shadow` | 298 | text shadows (292); text-shadow (6) |
| `touch` | 10 | touch-action (10) |
| `transform` | 11 | transform-box (5); transform (4); transform-style (2) |
| `transition` | 9 | gated motion (7); transition-behavior (2) |
| `translate-z` | 70 | translate-z (70) |
| `underline-offset` | 11 | text-underline-offset (11) |
| `visible` | 1 | visibility (1) |
| `will-change` | 4 | will-change (4) |
| `zoom` | 10 | zoom (10) |
