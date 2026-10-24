# GPU View styling

The authoring target is TSX with Tailwind utilities. Use the official Tailwind
compiler at build time, then lower its CSS declarations to native style rules.
Do not reimplement utility names or load a CSS compiler into the View isolate.
Pin Tailwind's version; review generated output and cascade tests before upgrades.

`view` is an authoring alias for a row flex container. `row`/`column` retain
their existing default directions; utilities override those defaults, and
inline style overrides utilities. `flex`, `flex-row`, `flex-col` and their
reverse/responsive variants lower to fields104 (`display: flex`) and103
(`flexDirection`). Only these three flex containers admit the new fields;
grid, stack, scroll, list and controls keep their distinct layout contracts.
Wrap their contents in a flex container instead of silently changing those
primitives' layout algorithms. Direction changes use shared Taffy layout and
preserve node identity; reversal changes visual placement, not source order.

`overflow-x-auto` lowers to `scrollAxis: 'horizontal'` on `<scroll>` only. Its
children retain their layout direction: wrap a row for horizontal items. Omission
keeps vertical scrolling; virtual lists do not accept this style. This adapter
supports one scrolling axis per region, not general CSS overflow on arbitrary
nodes or simultaneous two-axis scrolling. Other overflow utilities still fail
diagnostics rather than silently selecting an axis.

`flex-wrap`, `flex-nowrap` and `flex-wrap-reverse` map to native `flexWrap`
(field 87). Taffy performs wrapping against the container's available size,
preserving gaps and cross-axis reversal. Omission keeps nowrap. Use wrapped
rows for content-sized responsive items rather than choosing columns from the
viewport width in JS.
Pixel 3 verifies single-row short chart labels, wrapped long labels and their
click actions in both themes; CPU layout tests cover viewport-width reflow.

`items-baseline` / `self-baseline` lower to first-baseline cross alignment.
Feed Taffy the shaped text baseline relative to its border box, including
padding/borders and vertically centered control labels. Known leaf dimensions
can skip measurement, so resolve the baseline at the final content width too.
Nested containers propagate Taffy's baseline; nodes without text use its
synthesized fallback. CPU and GPU consume the same resulting display list.

`grow` changes only the growth factor. `flexBasis` (field99) independently accepts
`auto`, nonnegative logical lengths or percentage strings; omission is auto.
Lower `basis-*` through the compiled CSS dimension and resolve percentages in
Taffy against the parent's main axis. `flex-1` lowers to1/1/0%, `flex-auto` to
1/1/auto, `flex-initial` to0/1/auto and `flex-none` to0/0/auto. Preserve the
length/percentage distinction in the wire and Taffy style. Indefinite-container
percentage behavior still needs browser comparison; do not claim full intrinsic
sizing parity from definite-size tests. Expand
shorthands before cascade so later basis utilities and inline styles can override.
Existing showcase equal-space/fill layouts explicitly use `grow basis-0` or
`flexBasis:0`; they previously depended on growth implicitly resetting basis.

## Boundaries

- Tailwind owns utility expansion, design tokens and utility ordering.
- The adapter owns CSS-to-native conversion and capability diagnostics. Unknown
  declarations, selectors and conditions must fail rather than disappear.
- The View runtime matches compiled rules against class names and View environment,
  resolves computed typography, then submits ordinary validated native styles.
  Class order in the TSX string must not alter Tailwind's cascade. Explicit
  `style` overrides utilities. Appearance and viewport changes reevaluate rules.
- The renderer owns geometry, clipping, painting and hit testing. Add shared
  primitives here for borders, shadows and gradients; do not simulate them
  independently in each showcase page.

Keep compiled rules separate from the native wire schema. An adapter extension
that uses existing fields needs no wire change; a new rendering capability still
requires append-only schema changes and CPU/GPU consistency tests.

Animation payload transport uses field101 (JSON), validated by the shared native
decoder. It is staged: production utility/inline authoring still rejects, and
ordinary native rendering refuses records containing it. Only explicit animated
session preparation may consume matching definitions. See
[ui-animation.md](ui-animation.md) before enabling authoring or collection.

Backdrop blur and saturation lower through Tailwind's generated CSS variables,
then compose in compiler order before emitting `backdropBlur` (field95,0..64
logical pixels) and `backdropSaturation` (field96,0..16, identity1). Preserve
variable values when `backdrop-filter-none` disables the property, so a later
variant can re-enable them. Empty blur variables remove blur; a filter with no
functions emits no backdrop fields. Explicit numeric zero still creates a root.
Fixed CSS filters support blur followed by saturation; reject other functions,
orders and oversized parameters. Native lengths scale once during plan lowering.
These fields are not inherited. Home uses blur-xl/blur-md; ToDo uses20px/180%.

Tailwind `placeholder:text-*` lowers the compiler's `::placeholder` color into
`placeholderColor` (field97). Theme/viewport conditions and inline overrides use
the normal cascade. Reject other placeholder declarations until implemented.
The renderer applies this non-inherited field only to the hint of an empty
input, textarea or search. Keep value text, caret, selection and search adornments
on the ordinary text color; omission retains that color for the hint too.

Editor `focus:` rules support color, background, opacity, border color, gradients,
outer shadows and placeholder color. Keep pseudo-class specificity above ordinary
media-only rules, then compiler order within equal specificity; inline styles win.
Resolve base and focused styles together so custom-property shadow/gradient layers
compose before encoding a bounded paint-only delta in field98. Reject geometry,
typography, backdrop and non-editor focus styles until their state contracts exist.
Native focus supplies the state; do not round-trip focus through page Logic.

Outer `ring-*` widths/colors lower to a zero-blur spread shadow before ordinary
shadows. Preserve their independent CSS variables; a fixed box-shadow declaration
replaces the composition. Insets and ring offsets remain unsupported. Home uses
focus:ring-2 plus focus:ring-blue-500/30; ToDo uses its subtle focus background.

Use `<vector paths={[{d, fill: 'none', stroke: 'currentColor', strokeWidth: 1.5}]} />`
for outline icons. Tailwind width/height and text-color utilities control their
size and inherited tint, including theme variants. Keep path data in reusable
TSX components; the renderer handles geometry. Vector children inside a composed
button stay decorative under that button's explicit accessibility label.

## Initial implementation

`packages/lingxia-ui/tailwind.mjs` compiles and lowers CSS;
`src/classes.js` indexes rules and computes styles. Only rules for a node's classes
are visited at render time. The compiler and color parser are build dependencies
and are absent from the generated View module.

TS/JS string literals supply complete utility candidates. Conditional complete
strings work; concatenated utility fragments are not inferred. Unknown class
names fail at runtime; literal JSX `className` attributes also receive source
diagnostics at build time. Component authors forward `className` explicitly.

A page imports one CSS entry; it may import other token files using quoted
relative `@import` paths. Expand imports in source order, reject cycles and cap
the graph at 128 files. Loader completion order must not affect token precedence.
The default Tailwind theme is loaded automatically. Conditional/remote imports,
custom variants, plugins and general CSS selectors remain unsupported.

Use `@theme`/`@theme inline` for Tailwind tokens and `:root` plus
`[data-theme='light'|'dark']` for palette custom properties. Resolve aliases
against the complete palette before lowering utilities, preserving CSS source
order. Compile appearance-specific values into rules; the isolate selects them
from resolved `View.env.theme`. Do not parse CSS or query OS appearance per node.
Matching `color-scheme` annotations are accepted; they do not override View.env.
Reject other declarations and `!important` in palette blocks.

Honor Tailwind's supported `color-mix` branch for variable colors such as
`bg-surface/80`; using its fallback alone loses the alpha. Unknown or cyclic
tokens in either appearance fail compilation for literal classes. Palette aliases
do not imply support for animations or other unsupported rendering properties.

Supported lowering currently includes fixed and percentage dimensions, per-edge padding, gap,
radius, alignment, grow/shrink, font size/weight, line height, colors and opacity. `rem` uses a
16-logical-pixel reference. Unitless line height retains its multiplier across
font-size overrides, component boundaries and list rows; fixed line height keeps
its logical-pixel value. Dark and min/max-width media rules use `View.env`, not
independent OS queries. Unsupported state variants fail even when inactive.
Width/height and min/max constraints accept percentage utilities, including
`w-full`, `w-1/2` and `max-w-[72%]`. Lower Tailwind's fraction calculation to a
percentage string and let native layout resolve the parent content box. Inline
dimensions use numbers for logical pixels or strings such as `'72%'`. Explicit
`height: 'auto'` / `h-auto` uses intrinsic content height with min/max constraints;
omitting height preserves primitive defaults. Other `auto` dimensions, general
calculations and percentage padding/gaps remain unsupported.
Tailwind's `rounded-full` emits `calc(infinity * 1px)`; lower that radius to the
native 16,384-unit bound, which painting clamps to the box. Infinite dimensions
remain unsupported. Pixel 3 verifies the resulting capsule on Device result
headings alongside normal bordered result rows.

Padding utilities lower to explicit edges, including CSS shorthands and the
horizontal inline / vertical block axes. Thus a later `md:p-4` resets earlier
`px-6` edges. Inline uniform `padding` overrides utility edges; explicit inline
edge values override that shorthand. Unspecified edges retain native primitive
defaults. Writing-mode and direction-dependent start/end utilities are not
implemented; do not silently map them to physical edges.

`text-left`, `text-right`, `text-center`, `text-start`, `text-end`, and
`text-justify` lower directly to inherited native `textAlign`. Keep physical
left/right distinct from the paragraph-direction-aware start/end values.

Tailwind's standard system sans/monospace stacks lower to the native `sans`/`mono`
families. Reject custom face stacks and serif until the renderer supports them;
do not silently discard requested font faces merely because a generic fallback
appears last.

`break-all`, `break-keep`, and `break-normal` lower to `wordBreak`;
`wrap-anywhere`, `wrap-break-word`, and `wrap-normal` lower to `overflowWrap`.
Preserve both declarations emitted by `break-normal`. Both properties inherit
through native parents, affect Parley shaping, and belong in the text-cache key.
Keep the original text intact: wrapping must not insert invisible characters
into addresses, accessible names or copyable values. Single-line editors remain
single-line regardless of wrapping styles.

Linear gradient utilities retain typed declaration metadata across rules: merge
direction, interpolation space, colors and positions in Tailwind order before
emitting `backgroundGradient`. Do not resolve `from`/`via`/`to` variables inside
each utility independently. Preserve `bg-none`, conditional overrides, transparent
defaults and CSS's nondecreasing stop fixup. Inline `backgroundGradient` overrides
the computed utility value. Background color remains beneath the gradient.
Only linear sRGB/Oklab gradients with bounded percentage stops are supported;
other interpolation spaces, length stops and arbitrary background images fail.

Outer box shadows retain typed layers and optional color overrides across
Tailwind rules. Preserve CSS order, transparent `shadow-none`, signed spread,
multiple layers and inline overrides. Lower only the recognized Tailwind shadow
composition; diagnose inset shadows, ring offsets and unsupported variable compositions.
Resolve nested transparent color mixes before quantizing colors to native bytes.
Known color/gradient `@supports` branches may surround a rule or occur inside it;
both forms retain the surrounding theme/viewport condition and CSS order.

Solid border utilities lower to physical edge widths and one border color.
Preserve Tailwind's generated custom-property defaults and cascade; normalize
CSS `currentcolor` to native `currentColor`. Uniform inline `borderWidth`
overrides all class-derived edges, then explicit inline edges override it.
Use `border-0` to remove borders; unsupported border styles and per-edge colors
must fail compilation. Border widths participate in layout, not just paint.

The first adapter milestone passes compiler/runtime tests and physical Pixel 3
checks of class-authored dimensions, colors and conditional opacity, including
scrolling, rotation retention and both appearances after a disconnected cold
start. This proves that those compiled rules reach the device; it does not verify
responsive breakpoints or establish showcase visual parity.

Semantic-palette acceptance uses the independent Home page and a static-token
comparison against the React showcase. Pixel 3 checks verify page, surface and
primary colors in both appearances, repeated switches and rotation after a
disconnected cold start. Home greeting, language and navigation-retention
regressions pass. This covers palette selection, not the pending Home layout.

## Expansion and acceptance

Use the React showcase as the inventory for both workflows and visible styling,
excluding only the agreed native-component demonstrations. Keep the GPU example
independent. Prioritize:

1. Shared semantic tokens for light/dark, deterministic CSS imports and variable
   resolution, with matching React and GPU theme fixtures.
2. Per-edge spacing, percentage/flexible sizing, borders, shadows, gradients,
   image backgrounds and text alignment required by the reference layouts.
3. Control-state variants, transitions and accessibility/input parity with
   lifecycle-bound timing and no idle render loop.
4. Page-by-page migration and physical Android screenshot/interaction comparisons
   at matching viewport, appearance, language, data and keyboard state.

For each supported declaration, test cascade and computed-value semantics as
well as resulting native fields. For new rendering primitives, compare CPU/GPU
output and inspect physical-device pixels. Compiler tests alone do not establish
visual parity. Preserve the broader workflow and performance gates in
[ui-renderer.md](ui-renderer.md); this adapter is not full CSS engine support.

`line-through` and `no-underline` lower to `textDecorationLine` (`line-through`
or `none`, wire 81). Underlines and other decoration styles remain rejected.
Draw each visible shaped run using its advance and font strike metrics after
line breaking/alignment; keep the decoration out of shaping/cache keys. A
decoration propagates through native layout descendants in the originating
node's text color; descendant `none` does not erase an ancestor's decoration.
The current single strike uses the outermost decorating ancestor. Nested
independently colored/positioned strikes are not represented yet.
Pixel 3 verifies two decorated lines through whitespace, ancestor propagation,
scroll clipping/reveal and cached toggle restoration. ToDo bulk/filter/clear,
dark theme, rotation and physical keyboard regressions pass on the new native
binary.

Tracking utilities lower `letter-spacing` lengths, including signed px/rem and
em values, to `letterSpacing`. Resolve em on the owner using its computed font
size; native descendants inherit that computed length. `tracking-normal` resets
to zero. Inline spacing accepts a bounded numeric logical length only.


Physical corner utilities (`rounded-t-*`, `rounded-br-*`, etc.) lower through
Tailwind declarations into radiusTopLeft/TopRight/BottomRight/BottomLeft. Radius
shorthands set all four fields in CSS cascade order; inline radius replaces
class-derived corners before explicit inline corner overrides. Percentage and
elliptical authored radii remain unsupported and fail diagnostics.


Cache class cascades by full class string and focus state, bounded to256 entries.
Clear on compiled-rule replacement or theme/viewport-width change. Clone mutable
paint values before returning them; resolve inline styles and inherited relative
typography per node outside the cache. Never cache a node's computed style solely
by its class string.
