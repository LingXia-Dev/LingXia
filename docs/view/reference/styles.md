_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Style properties

CSS-named keys of the `style` prop and the targets of every utility. A key is accepted only by the
elements listed; anything else fails the build with `UI_STYLE` (inline) or `UI_CLASS_ELEMENT` (utility).
Numbers are logical pixels. **Paint** keys are the only ones `focus:`, `active:` and `hover:` may change.

| Key | Type | CSS | Paint | Accepted by |
| --- | --- | --- | --- | --- |
| `width` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| `${number}ch` \| "auto" \| "fit-content" \| "max-content" \| "min-content" `` | `width` |  | all but `<span>` `<link>` |
| `height` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "auto" \| "fit-content" \| "max-content" \| "min-content" `` | `height` |  | all but `<span>` `<link>` |
| `minWidth` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| `${number}ch` \| "fit-content" \| "max-content" \| "min-content" `` | `min-width` |  | all but `<span>` `<link>` |
| `minHeight` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "fit-content" \| "max-content" \| "min-content" `` | `min-height` |  | all but `<span>` `<link>` |
| `maxWidth` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| `${number}ch` \| "fit-content" \| "max-content" \| "min-content" `` | `max-width` |  | all but `<span>` `<link>` |
| `aspectRatio` | `number \| "auto"` | `aspect-ratio` |  | all but `<span>` `<link>` |
| `maxHeight` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "fit-content" \| "max-content" \| "min-content" `` | `max-height` |  | all but `<span>` `<link>` |
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
| `positionArea` | `"bottom span-inline-end" \| "bottom span-inline-start" \| "bottom" \| "top span-inline-end" \| "top span-inline-start" \| "top" \| "inline-end span-block-end" \| "inline-end span-block-start" \| "inline-end" \| "inline-start span-block-end" \| "inline-start span-block-start" \| "inline-start"` | `position-area` |  | all but `<span>` `<link>` |
| `flexGrow` | `number` | `flex-grow` |  | all but `<span>` `<link>` |
| `flexShrink` | `number` | `flex-shrink` |  | all but `<span>` `<link>` |
| `flexBasis` | `` number \| `${number}%` \| "auto" `` | `flex-basis` |  | all but `<span>` `<link>` |
| `alignSelf` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe flex-end" \| "safe end"` | `align-self` |  | all but `<span>` `<link>` |
| `gridColumn` | `number \| string` | `grid-column` |  | all but `<span>` `<link>` |
| `gridColumnStart` | `number \| string` | `grid-column-start` |  | all but `<span>` `<link>` |
| `gridColumnEnd` | `number \| string` | `grid-column-end` |  | all but `<span>` `<link>` |
| `gridRow` | `number \| string` | `grid-row` |  | all but `<span>` `<link>` |
| `gridRowStart` | `number \| string` | `grid-row-start` |  | all but `<span>` `<link>` |
| `gridRowEnd` | `number \| string` | `grid-row-end` |  | all but `<span>` `<link>` |
| `justifySelf` | `"start" \| "center" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe end"` | `justify-self` |  | all but `<span>` `<link>` |
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
| `ring` | `Ring \| "none"` |  | yes | all but `<span>` `<link>` |
| `backdropFilter` | `string` |  |  | all but `<span>` `<link>` |
| `translate` | `` number \| `${number}%` \| readonly [number \| `${number}%`, number \| `${number}%`] `` |  | yes | all but `<span>` `<link>` |
| `rotate` | `number` |  | yes | all but `<span>` `<link>` |
| `scale` | `number \| readonly [number, number]` |  | yes | all but `<span>` `<link>` |
| `transformOrigin` | `readonly [number, number]` |  | yes | all but `<span>` `<link>` |
| `filter` | `string` |  |  | all but `<span>` `<link>` |
| `padding` | `number` |  |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` |
| `paddingTop` | `number` | `padding-top`, `padding-block-start` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` |
| `paddingRight` | `number` | `padding-right` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` |
| `paddingBottom` | `number` | `padding-bottom`, `padding-block-end` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` |
| `paddingLeft` | `number` | `padding-left` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` |
| `paddingInlineStart` | `number` | `padding-inline-start` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` |
| `paddingInlineEnd` | `number` | `padding-inline-end` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` |
| `display` | `"flex" \| "grid" \| "none" \| "block" \| "inline-flex" \| "inline-block"` | `display` |  | all but `<span>` `<link>` |
| `touchTarget` | `"auto" \| "none"` |  |  | all but `<span>` `<link>` |
| `flexDirection` | `"row" \| "column" \| "row-reverse" \| "column-reverse"` | `flex-direction` |  | `<view>` `<page>` `<button>` `<dialog>` |
| `alignItems` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe flex-end" \| "safe end"` | `align-items` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` |
| `justifyContent` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "space-between" \| "space-around" \| "space-evenly" \| "safe center" \| "safe flex-end" \| "safe end"` | `justify-content` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` |
| `alignContent` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "space-between" \| "space-around" \| "space-evenly" \| "stretch" \| "safe center" \| "safe flex-end" \| "safe end"` | `align-content` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` |
| `flexWrap` | `"nowrap" \| "wrap" \| "wrap-reverse"` | `flex-wrap` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` |
| `gap` | `number` |  |  | `<view>` `<page>` `<button>` `<scroll>` `<List>` `<checkbox>` `<radio>` `<dialog>` |
| `rowGap` | `number` | `row-gap` |  | `<view>` `<page>` `<button>` `<scroll>` `<List>` `<checkbox>` `<radio>` `<dialog>` |
| `columnGap` | `number` | `column-gap` |  | `<view>` `<page>` `<button>` `<scroll>` `<List>` `<checkbox>` `<radio>` `<dialog>` |
| `gridTemplateColumns` | `number \| string` | `grid-template-columns` |  | `<view>` `<page>` `<button>` `<dialog>` |
| `gridTemplateRows` | `number \| string` | `grid-template-rows` |  | `<view>` `<page>` `<button>` `<dialog>` |
| `gridAutoColumns` | `string` | `grid-auto-columns` |  | `<view>` `<page>` `<button>` `<dialog>` |
| `gridAutoRows` | `string` | `grid-auto-rows` |  | `<view>` `<page>` `<button>` `<dialog>` |
| `gridAutoFlow` | `"row" \| "column" \| "row dense" \| "column dense"` | `grid-auto-flow` |  | `<view>` `<page>` `<button>` `<dialog>` |
| `justifyItems` | `"start" \| "center" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe end"` | `justify-items` |  | `<view>` `<page>` `<button>` `<dialog>` |
| `color` | `string` | `color` | yes | all but `<image>` `<plot>` `<video>` `<webview>` |
| `fontSize` | `number` | `font-size` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `fontWeight` | `number` | `font-weight` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `fontFamily` | `"sans-serif" \| "monospace" \| "serif" \| (string & {})` | `font-family` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `fontStyle` | `"normal" \| "italic"` | `font-style` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `lineHeight` | `` number \| `${number}px` `` | `line-height` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` |
| `letterSpacing` | `` number \| `${number}em` `` | `letter-spacing` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `fontVariantNumeric` | `string` |  |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `userSelect` | `"none" \| "text" \| "all"` | `user-select` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` |
| `textAlign` | `"start" \| "end" \| "left" \| "center" \| "right" \| "justify"` | `text-align` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` |
| `textDecorationLine` | `"none" \| "line-through" \| "underline"` | `text-decoration-line` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `textDecorationStyle` | `"solid" \| "dashed" \| "dotted"` | `text-decoration-style` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `textDecorationColor` | `string \| "currentColor"` | `text-decoration-color` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` |
| `textTransform` | `"none" \| "uppercase" \| "lowercase" \| "capitalize"` | `text-transform` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` |
| `backgroundClip` | `"border-box" \| "text"` | `background-clip` |  | `<text>` `<select>` |
| `whiteSpace` | `"normal" \| "nowrap" \| "pre" \| "pre-wrap" \| "pre-line"` | `white-space` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` |
| `wordBreak` | `"normal" \| "break-all" \| "keep-all"` | `word-break` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` |
| `overflowWrap` | `"normal" \| "break-word" \| "anywhere"` | `overflow-wrap` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` |
| `lineClamp` | `number` | `-webkit-line-clamp` |  | `<text>` `<select>` |
| `textOverflow` | `"clip" \| "ellipsis"` | `text-overflow` |  | `<text>` `<input>` `<select>` |
| `overflowX` | `"auto"` | `overflow-x` |  | `<scroll>` |
| `overflowY` | `"auto"` | `overflow-y` |  | `<scroll>` |
| `overscrollBehavior` | `"auto" \| "contain" \| "none"` | `overscroll-behavior`, `overscroll-behavior-x`, `overscroll-behavior-y` |  | `<scroll>` `<List>` |
| `scrollSnapStop` | `"normal" \| "always"` | `scroll-snap-stop` |  | `<scroll>` `<List>` |
| `scrollSnapType` | `"none" \| "x" \| "y"` | `scroll-snap-type` |  | `<scroll>` `<List>` |
| `scrollSnapAlign` | `"none" \| "start" \| "center" \| "end"` | `scroll-snap-align` |  | all but `<span>` `<link>` |
| `placeholderColor` | `string` |  | yes | `<input>` `<textarea>` `<select>` |
| `objectFit` | `"cover" \| "contain" \| "fill"` | `object-fit` |  | `<image>` `<vector>` `<video>` |
| `objectPosition` | `string` | `object-position` |  | `<image>` `<video>` |

`transition` and `animation` are set only by the motion utilities (`transition-*`, `animate-*`).

## By element

### `<view>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<page>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<text>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `backgroundClip`, `lineClamp`, `textOverflow`

### `<button>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<scroll>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `overflowX`, `overflowY`, `overscrollBehavior`, `scrollSnapStop`, `scrollSnapType`

### `<List>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `overscrollBehavior`, `scrollSnapStop`, `scrollSnapType`

### `<image>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `objectFit`, `objectPosition`

### `<input>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `placeholderColor`, `textOverflow`

### `<textarea>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `placeholderColor`

### `<checkbox>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<switch>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<radio>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`

### `<slider>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`

### `<dialog>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `flexWrap`, `alignItems`, `justifyContent`, `alignContent`, `flexDirection`, `gap`, `rowGap`, `columnGap`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `gridTemplateColumns`, `gridTemplateRows`, `gridAutoColumns`, `gridAutoRows`, `gridAutoFlow`, `justifyItems`

### `<plot>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`

### `<vector>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `objectFit`

### `<span>`

`color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `letterSpacing`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `fontVariantNumeric`, `backgroundColor`

### `<link>`

`color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `letterSpacing`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `fontVariantNumeric`, `backgroundColor`

### `<select>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `padding`, `paddingTop`, `paddingRight`, `paddingBottom`, `paddingLeft`, `paddingInlineStart`, `paddingInlineEnd`, `color`, `fontSize`, `fontWeight`, `fontFamily`, `fontStyle`, `lineHeight`, `letterSpacing`, `fontVariantNumeric`, `userSelect`, `textAlign`, `textDecorationLine`, `textDecorationStyle`, `textDecorationColor`, `textTransform`, `whiteSpace`, `wordBreak`, `overflowWrap`, `backgroundClip`, `lineClamp`, `textOverflow`, `placeholderColor`

### `<video>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`, `objectFit`, `objectPosition`

### `<webview>`

`width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `aspectRatio`, `maxHeight`, `margin`, `marginTop`, `marginRight`, `marginBottom`, `marginLeft`, `marginInlineStart`, `marginInlineEnd`, `position`, `inset`, `top`, `right`, `bottom`, `left`, `insetInlineStart`, `insetInlineEnd`, `zIndex`, `positionArea`, `flexGrow`, `flexShrink`, `flexBasis`, `alignSelf`, `gridColumn`, `gridColumnStart`, `gridColumnEnd`, `gridRow`, `gridRowStart`, `gridRowEnd`, `justifySelf`, `order`, `opacity`, `overflow`, `backgroundColor`, `backgroundImage`, `borderRadius`, `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomRightRadius`, `borderBottomLeftRadius`, `borderStartStartRadius`, `borderStartEndRadius`, `borderEndStartRadius`, `borderEndEndRadius`, `borderWidth`, `borderTopWidth`, `borderRightWidth`, `borderBottomWidth`, `borderLeftWidth`, `borderInlineStartWidth`, `borderInlineEndWidth`, `borderStyle`, `borderColor`, `boxShadow`, `ring`, `backdropFilter`, `translate`, `rotate`, `scale`, `transformOrigin`, `filter`, `display`, `touchTarget`, `scrollSnapAlign`

## Colour roles

Every colour is one of these roles, optionally with an alpha percent (`accent/12`). The utility drops a `text-`
prefix (`text-muted`, not `text-text-muted`). The theme pack resolves each role per appearance and proves the
listed contrast minimums. `defaultPalette` from `@lingxia/view/theme` holds the default pack resolved for light and
dark, derived accent roles included, for proving contrast in code.

A pack writes the authored roles. Fixed roles are the same in every pack, and derived roles are computed from the
authored ones and the accent seed, unless a pack sets them.

| Role | Set by | Minimum contrast | Use |
| --- | --- | --- | --- |
| `canvas` | the pack |  | Window and page floor. |
| `surface` | the pack |  | Cards, cells and sheets. |
| `surface-raised` | the pack |  | Raised cards and panels that stay in the page. |
| `surface-sunken` | the pack |  | Inset fields and grouped backgrounds. |
| `line` | the pack | 1.2:1 on `surface`, 1.1:1 on `canvas` | Decorative separators and card borders; interactive boundaries use line-strong. |
| `text` | the pack | 4.5:1 on `canvas`, 4.5:1 on `surface`, 4.5:1 on `surface-raised`, 4.5:1 on `surface-sunken`, 4.5:1 on `selection`, 4.5:1 on `surface-overlay`, 4.5:1 on `hue-1-soft`, 4.5:1 on `hue-2-soft`, 4.5:1 on `hue-3-soft`, 4.5:1 on `hue-4-soft`, 4.5:1 on `hue-5-soft`, 4.5:1 on `hue-6-soft`, 4.5:1 on `hue-7-soft`, 4.5:1 on `hue-8-soft` | Body text. |
| `text-muted` | the pack | 4.5:1 on `canvas`, 4.5:1 on `surface`, 4.5:1 on `surface-overlay` | Secondary text. |
| `text-faint` | the pack | 3:1 on `surface`, 3:1 on `surface-overlay` | Hints and disabled text. |
| `accent` | derived | 3:1 on `surface`, 3:1 on `canvas` | Primary actions, selection and links. |
| `on-accent` | derived | 4.5:1 on `accent` | Text and icons on accent. |
| `accent-soft` | derived |  | Tinted containers and selected rows. |
| `on-accent-soft` | derived | 4.5:1 on `accent-soft` | Text on accent-soft. |
| `danger` | the pack | 3:1 on `surface` | Destructive actions and errors. |
| `on-danger` | derived | 4.5:1 on `danger` | Text and icons on danger. |
| `danger-soft` | derived |  | Error containers; text on it uses text. |
| `success` | the pack | 3:1 on `surface` | Success state. |
| `on-success` | derived | 4.5:1 on `success` | Text and icons on success. |
| `success-soft` | derived |  | Success containers; text on it uses text. |
| `warning` | the pack | 3:1 on `surface` | Warning state. |
| `on-warning` | derived | 4.5:1 on `warning` | Text and icons on warning. |
| `warning-soft` | derived |  | Warning containers; text on it uses text. |
| `focus` | derived | 3:1 on `surface`, 3:1 on `canvas` | Focus ring. |
| `scrim` | fixed |  | Modal backdrop, translucent; a pack without it takes a near-black of its own neutral. |
| `chart-1` | derived | 3:1 on `surface`, 3:1 on `canvas` | First categorical series. |
| `chart-2` | derived | 3:1 on `surface`, 3:1 on `canvas` | Second categorical series. |
| `chart-3` | derived | 3:1 on `surface`, 3:1 on `canvas` | Third categorical series. |
| `chart-4` | derived | 3:1 on `surface`, 3:1 on `canvas` | Fourth categorical series. |
| `chart-5` | derived | 3:1 on `surface`, 3:1 on `canvas` | Fifth categorical series. |
| `line-strong` | the pack | 3:1 on `surface`, 3:1 on `canvas` | Interactive boundaries: editor borders and control outlines. |
| `selection` | derived | 3:1 on `surface`, 3:1 on `canvas` | Selected-text background, from the accent's soft tone. |
| `shadow` | fixed |  | Elevation shadow ink; `shadow-*` layers take it at their alpha. |
| `media` | fixed |  | Translucent dock over video and images; the same in both appearances; a pack without it takes a near-black of its own neutral. |
| `on-media` | fixed | 4.5:1 on `media`, 4.5:1 on `media-canvas` | Text and icons over media; tracks and buffered ranges take it at an alpha (`on-media/25`). |
| `surface-overlay` | derived |  | Anything floating over the page: menus, popovers, toasts, dialogs and sheets; surface-raised, a step lighter in dark. |
| `knob` | fixed |  | The thumb of a switch or slider, drawn on an accent track; white in both appearances by default. |
| `success-mark` | derived | 3:1 on `surface`, 3:1 on `surface-sunken` | Success as a dot or shape, never text. |
| `warning-mark` | derived | 3:1 on `surface`, 3:1 on `surface-sunken` | Warning as a dot or shape, never text. |
| `danger-mark` | derived | 3:1 on `surface`, 3:1 on `surface-sunken` | Danger as a dot or shape, never text. |
| `media-canvas` | fixed |  | The whole screen behind a full-screen picture or video: opaque, near-black in both appearances; a pack without it takes a near-black of its own neutral. |
| `info` | derived | 3:1 on `surface` | Information that is neither good nor bad: notices and hints; a fixed blue at the accent's strength, or the accent family when the accent is that blue. |
| `on-info` | derived | 4.5:1 on `info` | Text and icons on info. |
| `info-soft` | derived |  | Information containers; text on it uses text. |
| `info-mark` | derived | 3:1 on `surface`, 3:1 on `surface-sunken` | Information as a dot or shape, never text. |
| `hue-1` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-1-soft` | First categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-2` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-2-soft` | Second categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-3` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-3-soft` | Third categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-4` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-4-soft` | Fourth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-5` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-5-soft` | Fifth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-6` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-6-soft` | Sixth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-7` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-7-soft` | Seventh categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-8` | derived | 3:1 on `surface`, 3:1 on `surface-sunken`, 3:1 on `hue-8-soft` | Eighth categorical hue, for tags, avatars and series past the chart colours: a dot or shape, never text. |
| `hue-1-soft` | derived |  | Fill behind the first hue; text on it uses text. |
| `hue-2-soft` | derived |  | Fill behind the second hue; text on it uses text. |
| `hue-3-soft` | derived |  | Fill behind the third hue; text on it uses text. |
| `hue-4-soft` | derived |  | Fill behind the fourth hue; text on it uses text. |
| `hue-5-soft` | derived |  | Fill behind the fifth hue; text on it uses text. |
| `hue-6-soft` | derived |  | Fill behind the sixth hue; text on it uses text. |
| `hue-7-soft` | derived |  | Fill behind the seventh hue; text on it uses text. |
| `hue-8-soft` | derived |  | Fill behind the eighth hue; text on it uses text. |
| `chart-6` | derived | 3:1 on `surface`, 3:1 on `canvas` | Sixth categorical series. |
| `chart-7` | derived | 3:1 on `surface`, 3:1 on `canvas` | Seventh categorical series. |
| `chart-8` | derived | 3:1 on `surface`, 3:1 on `canvas` | Eighth categorical series. |
| `accent-solid` | derived |  | Fill of a control that is pressed or switched and carries white marks: primary buttons, checked boxes and radios, switch tracks, a selected day, steppers, floating buttons; the accent in light, a deeper accent in dark. Accent text, icons, the focus ring and filled labels such as badges keep accent. |
| `on-accent-solid` | derived | 4.5:1 on `accent-solid`, 4.5:1 on `accent-solid-hover`, 4.5:1 on `accent-solid-pressed` | Text and icons on accent-solid. |
| `danger-solid` | derived |  | Fill of a destructive control that is pressed and carries white marks; danger in light, a deeper danger in dark. A danger badge or label keeps danger. |
| `on-danger-solid` | derived | 4.5:1 on `danger-solid`, 4.5:1 on `danger-solid-hover`, 4.5:1 on `danger-solid-pressed` | Text and icons on danger-solid. |
| `accent-solid-hover` | derived |  | accent-solid under a hovering pointer: a visible step deeper in both appearances, hue kept. Always derived; a pack does not write it. |
| `accent-solid-pressed` | derived |  | accent-solid while pressed: the same step again past hover, so a press always goes deeper. Always derived; a pack does not write it. |
| `danger-solid-hover` | derived |  | danger-solid under a hovering pointer: a visible step deeper, hue kept. Always derived; a pack does not write it. |
| `danger-solid-pressed` | derived |  | danger-solid while pressed: the same step again past hover. Always derived; a pack does not write it. |
