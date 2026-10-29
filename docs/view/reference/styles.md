_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Style properties

CSS-named keys of the `style` prop and the targets of every utility. A key is accepted only by the
elements listed; anything else fails the build with `UI_STYLE` (inline) or `UI_CLASS_ELEMENT` (utility).
Numbers are logical pixels. **Paint** keys are the only ones `focus:`, `active:` and `hover:` may change.

| Key | Type | CSS | Paint | Accepted by |
| --- | --- | --- | --- | --- |
| `width` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "auto" \| "fit-content" \| "max-content" \| "min-content" `` | `width` |  | all but `<span>` `<link>` |
| `height` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "auto" \| "fit-content" \| "max-content" \| "min-content" `` | `height` |  | all but `<span>` `<link>` |
| `minWidth` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` `` | `min-width` |  | all but `<span>` `<link>` |
| `minHeight` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` `` | `min-height` |  | all but `<span>` `<link>` |
| `maxWidth` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| `${number}ch` `` | `max-width` |  | all but `<span>` `<link>` |
| `aspectRatio` | `number \| "auto"` | `aspect-ratio` |  | all but `<span>` `<link>` |
| `maxHeight` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` `` | `max-height` |  | all but `<span>` `<link>` |
| `margin` | `number \| "auto"` |  |  | all but `<span>` `<link>` |
| `marginTop` | `number \| "auto"` | `margin-top`, `margin-block-start` |  | all but `<span>` `<link>` |
| `marginRight` | `number \| "auto"` | `margin-right` |  | all but `<span>` `<link>` |
| `marginBottom` | `number \| "auto"` | `margin-bottom`, `margin-block-end` |  | all but `<span>` `<link>` |
| `marginLeft` | `number \| "auto"` | `margin-left` |  | all but `<span>` `<link>` |
| `marginInlineStart` | `number \| "auto"` | `margin-inline-start` |  | all but `<span>` `<link>` |
| `marginInlineEnd` | `number \| "auto"` | `margin-inline-end` |  | all but `<span>` `<link>` |
| `position` | `"relative" \| "absolute" \| "fixed" \| "sticky"` | `position` |  | all but `<span>` `<link>` |
| `inset` | `` number \| `${number}%` `` |  |  | all but `<span>` `<link>` |
| `top` | `` number \| `${number}%` `` | `top` |  | all but `<span>` `<link>` |
| `right` | `` number \| `${number}%` `` | `right` |  | all but `<span>` `<link>` |
| `bottom` | `` number \| `${number}%` `` | `bottom` |  | all but `<span>` `<link>` |
| `left` | `` number \| `${number}%` `` | `left` |  | all but `<span>` `<link>` |
| `insetInlineStart` | `` number \| `${number}%` `` | `inset-inline-start` |  | all but `<span>` `<link>` |
| `insetInlineEnd` | `` number \| `${number}%` `` | `inset-inline-end` |  | all but `<span>` `<link>` |
| `zIndex` | `number` | `z-index` |  | all but `<span>` `<link>` |
| `flexGrow` | `number` | `flex-grow` |  | all but `<span>` `<link>` |
| `flexShrink` | `number` | `flex-shrink` |  | all but `<span>` `<link>` |
| `flexBasis` | `` number \| `${number}%` \| "auto" `` | `flex-basis` |  | all but `<span>` `<link>` |
| `alignSelf` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "stretch" \| "baseline"` | `align-self` |  | all but `<span>` `<link>` |
| `gridColumn` | `number \| string` | `grid-column` |  | all but `<span>` `<link>` |
| `gridColumnStart` | `number \| string` | `grid-column-start` |  | all but `<span>` `<link>` |
| `gridColumnEnd` | `number \| string` | `grid-column-end` |  | all but `<span>` `<link>` |
| `gridRow` | `number \| string` | `grid-row` |  | all but `<span>` `<link>` |
| `gridRowStart` | `number \| string` | `grid-row-start` |  | all but `<span>` `<link>` |
| `gridRowEnd` | `number \| string` | `grid-row-end` |  | all but `<span>` `<link>` |
| `justifySelf` | `"start" \| "center" \| "end" \| "stretch" \| "baseline"` | `justify-self` |  | all but `<span>` `<link>` |
| `order` | `number` | `order` |  | all but `<span>` `<link>` |
| `opacity` | `number` | `opacity` | yes | all but `<span>` `<link>` |
| `overflow` | `"visible" \| "hidden"` | `overflow` |  | all but `<span>` `<link>` |
| `backgroundColor` | `string \| "currentColor"` | `background-color` | yes | every element |
| `backgroundImage` | `BackgroundGradient \| "none"` |  | yes | all but `<span>` `<link>` |
| `borderRadius` | `number` | `border-radius` |  | all but `<span>` `<link>` |
| `borderTopLeftRadius` | `number` | `border-top-left-radius` |  | all but `<span>` `<link>` |
| `borderTopRightRadius` | `number` | `border-top-right-radius` |  | all but `<span>` `<link>` |
| `borderBottomRightRadius` | `number` | `border-bottom-right-radius` |  | all but `<span>` `<link>` |
| `borderBottomLeftRadius` | `number` | `border-bottom-left-radius` |  | all but `<span>` `<link>` |
| `borderStartStartRadius` | `number` | `border-start-start-radius` |  | all but `<span>` `<link>` |
| `borderStartEndRadius` | `number` | `border-start-end-radius` |  | all but `<span>` `<link>` |
| `borderEndStartRadius` | `number` | `border-end-start-radius` |  | all but `<span>` `<link>` |
| `borderEndEndRadius` | `number` | `border-end-end-radius` |  | all but `<span>` `<link>` |
| `borderWidth` | `number` |  |  | all but `<span>` `<link>` |
| `borderTopWidth` | `number` | `border-top-width`, `border-block-start-width` |  | all but `<span>` `<link>` |
| `borderRightWidth` | `number` | `border-right-width` |  | all but `<span>` `<link>` |
| `borderBottomWidth` | `number` | `border-bottom-width`, `border-block-end-width` |  | all but `<span>` `<link>` |
| `borderLeftWidth` | `number` | `border-left-width` |  | all but `<span>` `<link>` |
| `borderInlineStartWidth` | `number` | `border-inline-start-width` |  | all but `<span>` `<link>` |
| `borderInlineEndWidth` | `number` | `border-inline-end-width` |  | all but `<span>` `<link>` |
| `borderStyle` | `"solid" \| "dashed" \| "dotted"` | `border-style` |  | all but `<span>` `<link>` |
| `borderColor` | `string \| "currentColor"` | `border-color` | yes | all but `<span>` `<link>` |
| `boxShadow` | `ReadonlyArray<BoxShadow> \| "none"` |  | yes | all but `<span>` `<link>` |
| `backdropFilter` | `string` |  |  | all but `<span>` `<link>` |
| `translate` | `` number \| `${number}%` \| readonly [number \| `${number}%`, number \| `${number}%`] `` |  | yes | all but `<span>` `<link>` |
| `rotate` | `number` |  | yes | all but `<span>` `<link>` |
| `scale` | `number \| readonly [number, number]` |  | yes | all but `<span>` `<link>` |
| `transformOrigin` | `readonly [number, number]` |  | yes | all but `<span>` `<link>` |
| `filter` | `string` |  |  | all but `<span>` `<link>` |
| `padding` | `number` |  |  | all but `<spacer>` `<image>` `<plot>` `<span>` `<link>` |
| `paddingTop` | `number` | `padding-top`, `padding-block-start` |  | all but `<spacer>` `<image>` `<plot>` `<span>` `<link>` |
| `paddingRight` | `number` | `padding-right` |  | all but `<spacer>` `<image>` `<plot>` `<span>` `<link>` |
| `paddingBottom` | `number` | `padding-bottom`, `padding-block-end` |  | all but `<spacer>` `<image>` `<plot>` `<span>` `<link>` |
| `paddingLeft` | `number` | `padding-left` |  | all but `<spacer>` `<image>` `<plot>` `<span>` `<link>` |
| `paddingInlineStart` | `number` | `padding-inline-start` |  | all but `<spacer>` `<image>` `<plot>` `<span>` `<link>` |
| `paddingInlineEnd` | `number` | `padding-inline-end` |  | all but `<spacer>` `<image>` `<plot>` `<span>` `<link>` |
| `display` | `"flex" \| "grid" \| "none" \| "block" \| "inline-flex" \| "inline-block"` | `display` |  | all but `<span>` `<link>` |
| `flexDirection` | `"row" \| "column" \| "row-reverse" \| "column-reverse"` | `flex-direction` |  | `<view>` `<column>` `<row>` `<page>` `<button>` |
| `alignItems` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "stretch" \| "baseline"` | `align-items` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<scroll>` `<checkbox>` `<grid>` |
| `justifyContent` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "space-between" \| "space-around" \| "space-evenly"` | `justify-content` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<scroll>` `<checkbox>` `<grid>` |
| `alignContent` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "space-between" \| "space-around" \| "space-evenly" \| "stretch"` | `align-content` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<scroll>` `<checkbox>` `<grid>` |
| `flexWrap` | `"nowrap" \| "wrap" \| "wrap-reverse"` | `flex-wrap` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<scroll>` `<checkbox>` |
| `gap` | `number` |  |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<scroll>` `<list>` `<checkbox>` `<radio>` `<grid>` |
| `rowGap` | `number` | `row-gap` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<scroll>` `<list>` `<checkbox>` `<radio>` `<grid>` |
| `columnGap` | `number` | `column-gap` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<scroll>` `<list>` `<checkbox>` `<radio>` `<grid>` |
| `gridTemplateColumns` | `number \| string` | `grid-template-columns` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<grid>` |
| `gridTemplateRows` | `number \| string` | `grid-template-rows` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<grid>` |
| `gridAutoColumns` | `string` | `grid-auto-columns` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<grid>` |
| `gridAutoRows` | `string` | `grid-auto-rows` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<grid>` |
| `gridAutoFlow` | `"row" \| "column" \| "row dense" \| "column dense"` | `grid-auto-flow` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<grid>` |
| `justifyItems` | `"start" \| "center" \| "end" \| "stretch" \| "baseline"` | `justify-items` |  | `<view>` `<column>` `<row>` `<page>` `<button>` `<grid>` |
| `color` | `string` | `color` | yes | all but `<spacer>` `<image>` `<plot>` |
| `fontSize` | `number` | `font-size` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` |
| `fontWeight` | `number` | `font-weight` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` |
| `fontFamily` | `"sans-serif" \| "monospace" \| "serif" \| (string & {})` | `font-family` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` |
| `fontStyle` | `"normal" \| "italic"` | `font-style` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` |
| `lineHeight` | `` number \| `${number}px` `` | `line-height` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` |
| `letterSpacing` | `` number \| `${number}em` `` | `letter-spacing` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` |
| `fontVariantNumeric` | `string` |  |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` |
| `userSelect` | `"none" \| "text" \| "all"` | `user-select` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` |
| `textAlign` | `"start" \| "end" \| "left" \| "center" \| "right" \| "justify"` | `text-align` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` |
| `textDecorationLine` | `"none" \| "line-through" \| "underline"` | `text-decoration-line` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` |
| `textTransform` | `"none" \| "uppercase" \| "lowercase" \| "capitalize"` | `text-transform` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` |
| `backgroundClip` | `"border-box" \| "text"` | `background-clip` |  | `<text>` |
| `whiteSpace` | `"normal" \| "nowrap" \| "pre" \| "pre-wrap" \| "pre-line"` | `white-space` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` |
| `wordBreak` | `"normal" \| "break-all" \| "keep-all"` | `word-break` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` |
| `overflowWrap` | `"normal" \| "break-word" \| "anywhere"` | `overflow-wrap` |  | all but `<spacer>` `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` |
| `lineClamp` | `number` | `-webkit-line-clamp` |  | `<text>` |
| `textOverflow` | `"clip" \| "ellipsis"` | `text-overflow` |  | `<text>` |
| `overflowX` | `"auto"` | `overflow-x` |  | `<scroll>` |
| `overflowY` | `"auto"` | `overflow-y` |  | `<scroll>` |
| `overscrollBehavior` | `"auto" \| "contain" \| "none"` | `overscroll-behavior`, `overscroll-behavior-x`, `overscroll-behavior-y` |  | `<scroll>` `<list>` |
| `placeholderColor` | `string` |  | yes | `<input>` `<textarea>` |
| `objectFit` | `"cover" \| "contain" \| "fill"` | `object-fit` |  | `<image>` `<vector>` |
| `objectPosition` | `string` | `object-position` |  | `<image>` |

`transition` and `animation` are set only by the motion utilities (`transition-*`, `animate-*`).

## By element

### `<view>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<column>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<row>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<page>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<text>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `backgroundClip`, `lineClamp`, `textOverflow`

### `<button>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<spacer>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`

### `<scroll>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `overflowX`, `overflowY`, `overscrollBehavior`

### `<list>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `overscrollBehavior`

### `<image>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `objectFit`, `objectPosition`

### `<input>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `placeholderColor`

### `<textarea>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `placeholderColor`

### `<checkbox>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<switch>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<radio>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<slider>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`

### `<grid>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`, `alignItems`, `justifyContent`, `alignContent`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<stack>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<plot>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`

### `<vector>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `objectFit`

### `<span>`

`color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `letterSpacing`, `textDecorationLine`, `fontVariantNumeric`, `backgroundColor`

### `<link>`

`color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `letterSpacing`, `textDecorationLine`, `fontVariantNumeric`, `backgroundColor`

### `<select>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

## Colour roles

Every colour is one of these roles, optionally with an alpha percent (`accent/12`). The utility drops a `text-`
prefix (`text-muted`, not `text-text-muted`). The theme pack resolves each role per appearance and proves the
listed contrast minimums.

| Role | Minimum contrast | Use |
| --- | --- | --- |
| `canvas` |  | Window and page floor. |
| `surface` |  | Cards, cells and sheets. |
| `surface-raised` |  | Menus, popovers and elevated sheets. |
| `surface-sunken` |  | Inset fields and grouped backgrounds. |
| `line` | 1.2:1 on `surface`, 1.1:1 on `canvas` | Decorative separators and card borders; interactive boundaries use line-strong. |
| `text` | 4.5:1 on `canvas`, 4.5:1 on `surface`, 4.5:1 on `surface-raised`, 4.5:1 on `surface-sunken`, 4.5:1 on `selection` | Body text. |
| `text-muted` | 4.5:1 on `canvas`, 4.5:1 on `surface` | Secondary text. |
| `text-faint` | 3:1 on `surface` | Hints and disabled text. |
| `accent` | 3:1 on `surface`, 3:1 on `canvas` | Primary actions, selection and links. |
| `on-accent` | 4.5:1 on `accent` | Text and icons on accent. |
| `accent-soft` |  | Tinted containers and selected rows. |
| `on-accent-soft` | 4.5:1 on `accent-soft` | Text on accent-soft. |
| `danger` | 3:1 on `surface` | Destructive actions and errors. |
| `on-danger` | 4.5:1 on `danger` | Text and icons on danger. |
| `danger-soft` |  | Error containers; text on it uses text. |
| `success` | 3:1 on `surface` | Success state. |
| `on-success` | 4.5:1 on `success` | Text and icons on success. |
| `success-soft` |  | Success containers; text on it uses text. |
| `warning` | 3:1 on `surface` | Warning state. |
| `on-warning` | 4.5:1 on `warning` | Text and icons on warning. |
| `warning-soft` |  | Warning containers; text on it uses text. |
| `focus` | 3:1 on `surface`, 3:1 on `canvas` | Focus ring. |
| `scrim` |  | Modal backdrop, translucent. |
| `chart-1` | 3:1 on `surface`, 3:1 on `canvas` | First categorical series. |
| `chart-2` | 3:1 on `surface`, 3:1 on `canvas` | Second categorical series. |
| `chart-3` | 3:1 on `surface`, 3:1 on `canvas` | Third categorical series. |
| `chart-4` | 3:1 on `surface`, 3:1 on `canvas` | Fourth categorical series. |
| `chart-5` | 3:1 on `surface`, 3:1 on `canvas` | Fifth categorical series. |
| `line-strong` | 3:1 on `surface`, 3:1 on `canvas` | Interactive boundaries: editor borders and control outlines. |
| `selection` | 3:1 on `surface`, 3:1 on `canvas` | Selected-text background; derived from the accent when a pack omits it. |
| `shadow` |  | Elevation shadow ink; `shadow-*` layers take it at their alpha. |
