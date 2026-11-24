_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Style properties

CSS-named keys of the `style` prop and the targets of every utility. A key is accepted only by the
elements listed; anything else fails the build with `UI_STYLE` (inline) or `UI_CLASS_ELEMENT` (utility).
Numbers are logical pixels. **Paint** keys are the only ones `focus:`, `active:` and `hover:` may change.

| Key | Type | CSS | Paint | Accepted by | Notes |
| --- | --- | --- | --- | --- | --- |
| `width` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| `${number}ch` \| "auto" \| "fit-content" \| "max-content" \| "min-content" `` | `width` |  | all but `<span>` `<link>` | Width in logical pixels, a percent of the parent, a viewport unit, `ch`, or an intrinsic size. |
| `height` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "auto" \| "fit-content" \| "max-content" \| "min-content" `` | `height` |  | all but `<span>` `<link>` | Height in logical pixels, a percent of the parent, a viewport unit, or an intrinsic size. |
| `minWidth` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| `${number}ch` \| "fit-content" \| "max-content" \| "min-content" `` | `min-width` |  | all but `<span>` `<link>` | The smallest width layout may give it. |
| `minHeight` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "fit-content" \| "max-content" \| "min-content" `` | `min-height` |  | all but `<span>` `<link>` | The smallest height layout may give it. |
| `maxWidth` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| `${number}ch` \| "fit-content" \| "max-content" \| "min-content" `` | `max-width` |  | all but `<span>` `<link>` | The largest width layout may give it. |
| `aspectRatio` | `number \| "auto"` | `aspect-ratio` |  | all but `<span>` `<link>` | Width divided by height, kept when only one of them is set. |
| `maxHeight` | `` number \| `${number}%` \| `${number}vw` \| `${number}vh` \| "fit-content" \| "max-content" \| "min-content" `` | `max-height` |  | all but `<span>` `<link>` | The largest height layout may give it. |
| `margin` | `number \| "auto"` |  |  | all but `<span>` `<link>` | Space outside the border on every side; `auto` takes the free space. |
| `marginTop` | `number \| "auto"` | `margin-top`, `margin-block-start` |  | all but `<span>` `<link>` | Space outside the top border. |
| `marginRight` | `number \| "auto"` | `margin-right` |  | all but `<span>` `<link>` | Space outside the right border. |
| `marginBottom` | `number \| "auto"` | `margin-bottom`, `margin-block-end` |  | all but `<span>` `<link>` | Space outside the bottom border. |
| `marginLeft` | `number \| "auto"` | `margin-left` |  | all but `<span>` `<link>` | Space outside the left border. |
| `marginInlineStart` | `number \| "auto"` | `margin-inline-start` |  | all but `<span>` `<link>` | Space outside the start border: left in left-to-right text, right in right-to-left. |
| `marginInlineEnd` | `number \| "auto"` | `margin-inline-end` |  | all but `<span>` `<link>` | Space outside the end border: right in left-to-right text, left in right-to-left. |
| `position` | `"relative" \| "absolute" \| "fixed" \| "sticky"` | `position` |  | all but `<span>` `<link>` | `relative` offsets it from its place; `absolute` places it in its parent; `fixed` in the screen; `sticky` sticks inside its scroll. |
| `inset` | `` number \| `${number}%` `` |  |  | all but `<span>` `<link>` | Offset of every edge from the containing box, for a positioned element. |
| `top` | `` number \| `${number}%` \| "anchor(center)" `` | `top` |  | all but `<span>` `<link>` | Offset of the top edge; `anchor(center)` centres an anchored popover on its anchor. |
| `right` | `` number \| `${number}%` \| "anchor(center)" `` | `right` |  | all but `<span>` `<link>` | Offset of the right edge; `anchor(center)` centres an anchored popover on its anchor. |
| `bottom` | `` number \| `${number}%` \| "anchor(center)" `` | `bottom` |  | all but `<span>` `<link>` | Offset of the bottom edge; `anchor(center)` centres an anchored popover on its anchor. |
| `left` | `` number \| `${number}%` \| "anchor(center)" `` | `left` |  | all but `<span>` `<link>` | Offset of the left edge; `anchor(center)` centres an anchored popover on its anchor. |
| `insetInlineStart` | `` number \| `${number}%` \| "anchor(center)" `` | `inset-inline-start` |  | all but `<span>` `<link>` | Offset of the start edge, following the text direction. |
| `insetInlineEnd` | `` number \| `${number}%` \| "anchor(center)" `` | `inset-inline-end` |  | all but `<span>` `<link>` | Offset of the end edge, following the text direction. |
| `zIndex` | `number` | `z-index` |  | all but `<span>` `<link>` | Paint order among siblings; higher paints over lower. |
| `positionArea` | `"bottom span-inline-end" \| "bottom span-inline-start" \| "bottom" \| "top span-inline-end" \| "top span-inline-start" \| "top" \| "inline-end span-block-end" \| "inline-end span-block-start" \| "inline-end" \| "inline-start span-block-end" \| "inline-start span-block-start" \| "inline-start"` | `position-area` |  | all but `<span>` `<link>` | Where an anchored popover sits against its anchor; it flips when that side lacks room. |
| `flexGrow` | `number` | `flex-grow` |  | all but `<span>` `<link>` | Share of the free space along the main axis it takes. |
| `flexShrink` | `number` | `flex-shrink` |  | all but `<span>` `<link>` | Share of the overflow along the main axis it gives up; 0 keeps its size. |
| `flexBasis` | `` number \| `${number}%` \| "auto" `` | `flex-basis` |  | all but `<span>` `<link>` | Main-axis size before free space is shared out. |
| `alignSelf` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe flex-end" \| "safe end"` | `align-self` |  | all but `<span>` `<link>` | Cross-axis alignment of this item, overriding its parent's `alignItems`. |
| `gridColumn` | `number \| string` | `grid-column` |  | all but `<span>` `<link>` | Grid columns it spans, as `start / end` or `span n`. |
| `gridColumnStart` | `number \| string` | `grid-column-start` |  | all but `<span>` `<link>` | Grid line its column span starts at. |
| `gridColumnEnd` | `number \| string` | `grid-column-end` |  | all but `<span>` `<link>` | Grid line its column span ends at, or `span n`. |
| `gridRow` | `number \| string` | `grid-row` |  | all but `<span>` `<link>` | Grid rows it spans, as `start / end` or `span n`. |
| `gridRowStart` | `number \| string` | `grid-row-start` |  | all but `<span>` `<link>` | Grid line its row span starts at. |
| `gridRowEnd` | `number \| string` | `grid-row-end` |  | all but `<span>` `<link>` | Grid line its row span ends at, or `span n`. |
| `justifySelf` | `"start" \| "center" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe end"` | `justify-self` |  | all but `<span>` `<link>` | Inline alignment of this grid item in its cell. |
| `order` | `number` | `order` |  | all but `<span>` `<link>` | Layout order among flex or grid siblings; lower comes first. |
| `opacity` | `number` | `opacity` | yes | all but `<span>` `<link>` | Opacity of the element and its subtree, 0..1. |
| `overflow` | `"visible" \| "hidden"` | `overflow` |  | all but `<span>` `<link>` | `hidden` clips the subtree to the border box and its radius. |
| `backgroundColor` | `ColorValue \| "currentColor"` | `background-color` | yes | every element | Fill behind the content, as a colour role. |
| `backgroundImage` | `BackgroundGradient \| "none"` |  | yes | all but `<span>` `<link>` | A linear, radial or conic gradient painted behind the content. |
| `borderRadius` | `number` | `border-radius` |  | all but `<span>` `<link>` | Corner radius of every corner. |
| `borderTopLeftRadius` | `number` | `border-top-left-radius` |  | all but `<span>` `<link>` | Radius of the top-left corner. |
| `borderTopRightRadius` | `number` | `border-top-right-radius` |  | all but `<span>` `<link>` | Radius of the top-right corner. |
| `borderBottomRightRadius` | `number` | `border-bottom-right-radius` |  | all but `<span>` `<link>` | Radius of the bottom-right corner. |
| `borderBottomLeftRadius` | `number` | `border-bottom-left-radius` |  | all but `<span>` `<link>` | Radius of the bottom-left corner. |
| `borderStartStartRadius` | `number` | `border-start-start-radius` |  | all but `<span>` `<link>` | Radius of the top corner on the start side. |
| `borderStartEndRadius` | `number` | `border-start-end-radius` |  | all but `<span>` `<link>` | Radius of the top corner on the end side. |
| `borderEndStartRadius` | `number` | `border-end-start-radius` |  | all but `<span>` `<link>` | Radius of the bottom corner on the start side. |
| `borderEndEndRadius` | `number` | `border-end-end-radius` |  | all but `<span>` `<link>` | Radius of the bottom corner on the end side. |
| `borderWidth` | `number` |  |  | all but `<span>` `<link>` | Border width on every side. |
| `borderTopWidth` | `number` | `border-top-width`, `border-block-start-width` |  | all but `<span>` `<link>` | Width of the top border. |
| `borderRightWidth` | `number` | `border-right-width` |  | all but `<span>` `<link>` | Width of the right border. |
| `borderBottomWidth` | `number` | `border-bottom-width`, `border-block-end-width` |  | all but `<span>` `<link>` | Width of the bottom border. |
| `borderLeftWidth` | `number` | `border-left-width` |  | all but `<span>` `<link>` | Width of the left border. |
| `borderInlineStartWidth` | `number` | `border-inline-start-width` |  | all but `<span>` `<link>` | Width of the start-side border, following the text direction. |
| `borderInlineEndWidth` | `number` | `border-inline-end-width` |  | all but `<span>` `<link>` | Width of the end-side border, following the text direction. |
| `borderStyle` | `"solid" \| "dashed" \| "dotted"` | `border-style` |  | all but `<span>` `<link>` | Line style of every border. |
| `borderColor` | `ColorValue \| "currentColor"` | `border-color` | yes | all but `<span>` `<link>` | Colour of every border, as a colour role. |
| `boxShadow` | `ReadonlyArray<BoxShadow> \| "none"` |  | yes | all but `<span>` `<link>` | Shadows painted outside the border box, up to the first listed on top. |
| `ring` | `Ring \| "none"` |  | yes | all but `<span>` `<link>` | A focus-style outline: an outer ring with an optional gap, and an inner ring. |
| `backdropFilter` | `string` |  |  | all but `<span>` `<link>` | Filter applied to what is behind it, such as `blur(12px)` for frosted glass. |
| `translate` | `` number \| `${number}%` \| readonly [number \| `${number}%`, number \| `${number}%`] `` |  | yes | all but `<span>` `<link>` | Moves it without changing layout, in pixels or percents of its own size. |
| `rotate` | `number` |  | yes | all but `<span>` `<link>` | Rotation in degrees around `transformOrigin`, without changing layout. |
| `scale` | `number \| readonly [number, number]` |  | yes | all but `<span>` `<link>` | Scale, uniform or `[x, y]`, around `transformOrigin`, without changing layout. |
| `transformOrigin` | `readonly [number, number]` |  | yes | all but `<span>` `<link>` | The point `rotate` and `scale` turn around, as fractions `[x, y]` of its box. |
| `filter` | `string` |  |  | all but `<span>` `<link>` | Filter applied to its own pixels, such as `blur(4px)` or `grayscale(1)`. |
| `padding` | `number` |  |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` | Space inside the border on every side. |
| `paddingTop` | `number` | `padding-top`, `padding-block-start` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` | Space inside the top border. |
| `paddingRight` | `number` | `padding-right` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` | Space inside the right border. |
| `paddingBottom` | `number` | `padding-bottom`, `padding-block-end` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` | Space inside the bottom border. |
| `paddingLeft` | `number` | `padding-left` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` | Space inside the left border. |
| `paddingInlineStart` | `number` | `padding-inline-start` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` | Space inside the start border, following the text direction. |
| `paddingInlineEnd` | `number` | `padding-inline-end` |  | all but `<image>` `<plot>` `<span>` `<link>` `<video>` `<webview>` | Space inside the end border, following the text direction. |
| `display` | `"flex" \| "grid" \| "none" \| "block" \| "inline-flex" \| "inline-block"` | `display` |  | all but `<span>` `<link>` | `flex` (the default) or `grid` lays out children; `none` removes it from layout. |
| `touchTarget` | `"auto" \| "none"` |  |  | all but `<span>` `<link>` | `none` keeps presses to its own box instead of widening it to the platform touch target. |
| `flexDirection` | `"row" \| "column" \| "row-reverse" \| "column-reverse"` | `flex-direction` |  | `<view>` `<page>` `<button>` `<dialog>` | Main axis of a flex container; `<view>` is a column unless set. |
| `alignItems` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe flex-end" \| "safe end"` | `align-items` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` | Cross-axis alignment of the children. |
| `justifyContent` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "space-between" \| "space-around" \| "space-evenly" \| "safe center" \| "safe flex-end" \| "safe end"` | `justify-content` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` | Main-axis placement of the children and the space between them. |
| `alignContent` | `"flex-start" \| "start" \| "center" \| "flex-end" \| "end" \| "space-between" \| "space-around" \| "space-evenly" \| "stretch" \| "safe center" \| "safe flex-end" \| "safe end"` | `align-content` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` | Placement of wrapped lines or grid tracks in the cross axis. |
| `flexWrap` | `"nowrap" \| "wrap" \| "wrap-reverse"` | `flex-wrap` |  | `<view>` `<page>` `<button>` `<scroll>` `<checkbox>` `<dialog>` | Whether children wrap onto more lines when they do not fit. |
| `gap` | `number` |  |  | `<view>` `<page>` `<button>` `<scroll>` `<List>` `<checkbox>` `<radio>` `<dialog>` | Space between children in both axes. |
| `rowGap` | `number` | `row-gap` |  | `<view>` `<page>` `<button>` `<scroll>` `<List>` `<checkbox>` `<radio>` `<dialog>` | Space between rows. |
| `columnGap` | `number` | `column-gap` |  | `<view>` `<page>` `<button>` `<scroll>` `<List>` `<checkbox>` `<radio>` `<dialog>` | Space between columns. |
| `gridTemplateColumns` | `number \| string` | `grid-template-columns` |  | `<view>` `<page>` `<button>` `<dialog>` | Column tracks of a grid: a count of equal columns, or a track list. |
| `gridTemplateRows` | `number \| string` | `grid-template-rows` |  | `<view>` `<page>` `<button>` `<dialog>` | Row tracks of a grid: a count of equal rows, or a track list. |
| `gridAutoColumns` | `string` | `grid-auto-columns` |  | `<view>` `<page>` `<button>` `<dialog>` | Size of columns the grid creates on its own. |
| `gridAutoRows` | `string` | `grid-auto-rows` |  | `<view>` `<page>` `<button>` `<dialog>` | Size of rows the grid creates on its own. |
| `gridAutoFlow` | `"row" \| "column" \| "row dense" \| "column dense"` | `grid-auto-flow` |  | `<view>` `<page>` `<button>` `<dialog>` | Direction items without a place fill the grid in; `dense` fills earlier holes. |
| `justifyItems` | `"start" \| "center" \| "end" \| "stretch" \| "baseline" \| "safe center" \| "safe end"` | `justify-items` |  | `<view>` `<page>` `<button>` `<dialog>` | Inline alignment of every grid item in its cell. |
| `color` | `ColorValue` | `color` | yes | all but `<image>` `<plot>` `<video>` `<webview>` | Text and icon colour, as a colour role; inherited. |
| `fontSize` | `number` | `font-size` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Font size in logical pixels, scaled by the system text size; inherited. |
| `fontWeight` | `number` | `font-weight` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Font weight, 1..1000; inherited. |
| `fontFamily` | `"sans-serif" \| "monospace" \| "serif" \| (string & {})` | `font-family` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | A generic family or a font the app declares; inherited. |
| `fontStyle` | `"normal" \| "italic"` | `font-style` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Upright or italic; inherited. |
| `lineHeight` | `` number \| `${number}px` `` | `line-height` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` | Line height as a multiple of the font size, or in pixels; inherited. |
| `letterSpacing` | `` number \| `${number}em` `` | `letter-spacing` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Extra space between letters, in pixels or `em`; inherited. |
| `fontVariantNumeric` | `string` |  |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Numeric glyph variants such as `tabular-nums`; inherited. |
| `userSelect` | `"none" \| "text" \| "all"` | `user-select` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` | Whether its text can be selected and copied; inherited. |
| `textAlign` | `"start" \| "end" \| "left" \| "center" \| "right" \| "justify"` | `text-align` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` | Horizontal alignment of lines; `start` and `end` follow the text direction. |
| `textDecorationLine` | `"none" \| "line-through" \| "underline"` | `text-decoration-line` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Underline or strike-through. |
| `textDecorationStyle` | `"solid" \| "dashed" \| "dotted"` | `text-decoration-style` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Line style of the decoration. |
| `textDecorationColor` | `ColorValue \| "currentColor"` | `text-decoration-color` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<video>` `<webview>` | Colour of the decoration, as a colour role. |
| `textTransform` | `"none" \| "uppercase" \| "lowercase" \| "capitalize"` | `text-transform` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` | Case change applied when shown; the text itself is unchanged. |
| `backgroundClip` | `"border-box" \| "text"` | `background-clip` |  | `<text>` `<select>` | `text` paints the background through the glyphs only. |
| `whiteSpace` | `"normal" \| "nowrap" \| "pre" \| "pre-wrap" \| "pre-line"` | `white-space` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` | How spaces and newlines collapse and whether lines wrap. |
| `wordBreak` | `"normal" \| "break-all" \| "keep-all"` | `word-break` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` | Where words may break: anywhere (`break-all`), or never inside CJK runs (`keep-all`). |
| `overflowWrap` | `"normal" \| "break-word" \| "anywhere"` | `overflow-wrap` |  | all but `<image>` `<slider>` `<plot>` `<vector>` `<span>` `<link>` `<video>` `<webview>` | Whether a word longer than the line breaks inside rather than overflowing. |
| `lineClamp` | `number` | `-webkit-line-clamp` |  | `<text>` `<select>` | Lines shown at most; the rest is cut with an ellipsis. |
| `textOverflow` | `"clip" \| "ellipsis"` | `text-overflow` |  | `<text>` `<input>` `<select>` | `ellipsis` ends a cut line with an ellipsis. |
| `overflowX` | `"auto"` | `overflow-x` |  | `<scroll>` | `auto` scrolls a `<scroll>` horizontally. |
| `overflowY` | `"auto"` | `overflow-y` |  | `<scroll>` | `auto` scrolls a `<scroll>` vertically. |
| `overscrollBehavior` | `"auto" \| "contain" \| "none"` | `overscroll-behavior`, `overscroll-behavior-x`, `overscroll-behavior-y` |  | `<scroll>` `<List>` | `contain` keeps a scroll at its edge from moving an outer one; `none` also drops the bounce. |
| `scrollSnapStop` | `"normal" \| "always"` | `scroll-snap-stop` |  | `<scroll>` `<List>` | `always` stops a swipe at each snap point instead of skipping past. |
| `scrollSnapType` | `"none" \| "x" \| "y"` | `scroll-snap-type` |  | `<scroll>` `<List>` | Axis a scroll snaps along, to its children's `scrollSnapAlign`. |
| `scrollSnapAlign` | `"none" \| "start" \| "center" \| "end"` | `scroll-snap-align` |  | all but `<span>` `<link>` | Edge of this child a snapping scroll rests on. |
| `placeholderColor` | `ColorValue` |  | yes | `<input>` `<textarea>` `<select>` | Colour of an editor's placeholder, as a colour role. |
| `objectFit` | `"cover" \| "contain" \| "fill"` | `object-fit` |  | `<image>` `<vector>` `<video>` | How the picture fills its box: `cover` crops, `contain` letterboxes, `fill` stretches. |
| `objectPosition` | `string` | `object-position` |  | `<image>` `<video>` | Where the picture sits in its box when it does not fill it, such as `top` or `25% 75%`. |

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
dark, derived accent roles included, for proving contrast in code. In TypeScript a role is `ColorRole`, a style
colour `ColorValue` (a role, `role/NN` or `transparent`) and a `<plot>` series `ColorSeries`.

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
| `accent` | derived | 4.5:1 on `surface`, 4.5:1 on `canvas` | Primary actions, selection, links and text buttons: reads as text on canvas and surface. |
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
| `accent-solid` | derived |  | Fill of a control that is pressed or switched: primary buttons, checked boxes and radios, switch tracks, a selected day, steppers, floating buttons; the accent in light, a deeper accent in dark, under white marks. A light seed, such as a yellow, stays the fill in both, under dark marks. Accent text, icons, the focus ring and filled labels such as badges keep accent. |
| `on-accent-solid` | derived | 4.5:1 on `accent-solid`, 4.5:1 on `accent-solid-hover`, 4.5:1 on `accent-solid-pressed` | Text and icons on accent-solid. |
| `danger-solid` | derived |  | Fill of a destructive control that is pressed and carries white marks; danger in light, a deeper danger in dark. A danger badge or label keeps danger. |
| `on-danger-solid` | derived | 4.5:1 on `danger-solid`, 4.5:1 on `danger-solid-hover`, 4.5:1 on `danger-solid-pressed` | Text and icons on danger-solid. |
| `accent-solid-hover` | derived |  | accent-solid under a hovering pointer: a visible step deeper, or lighter where deeper would make its label hard to read; hue kept. Always derived; a pack does not write it. |
| `accent-solid-pressed` | derived |  | accent-solid while pressed: the same step again past hover, in the same direction. Always derived; a pack does not write it. |
| `danger-solid-hover` | derived |  | danger-solid under a hovering pointer: a visible step deeper, or lighter where deeper would make its label hard to read; hue kept. Always derived; a pack does not write it. |
| `danger-solid-pressed` | derived |  | danger-solid while pressed: the same step again past hover, in the same direction. Always derived; a pack does not write it. |

### Hover and pressed fills

A pressed or hovered fill deepens; where that would make its label hard to read, it lightens instead. A pack decides
the direction once per fill, from its pressed state: a brand fill with dark text in dark appearance brightens on press.
A lighter press keeps its label at 4.5:1 and the fill at 3:1 (or its rest contrast) against `surface` and `canvas`; a
written fill that holds neither way fails the build, naming the nearest fill that passes.

| Fill and label | Rest | Hover | Pressed |
| --- | --- | --- | --- |
| `#c6f36b` under `#1a2410` (light): deeper | `#c6f36b` 12.6:1 | `#bae564` 11.1:1 | `#aed65d` 9.6:1 |
| `#5285ff` under `#0a1420` (dark): lighter | `#5285ff` 5.4:1 | `#5e97ff` 6.5:1 | `#69a8ff` 7.6:1 |
