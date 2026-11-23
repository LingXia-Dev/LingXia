# @liulidesign/core

Liuli (琉璃) is the design system for LingXia's GPU pages: buttons, fields, menus, sheets, lists,
tables and the rest, written in `@lingxia/view` TSX and Tailwind classes, inspired by Astryx. A
GPU page is drawn natively on iOS, Android, HarmonyOS, macOS and Windows, with no browser, so
the same component is the same pixels and the same accessibility tree on all five.

The package is source: the View compiler compiles it with your pages, checks every class it uses,
and reports errors in its files by package name. There is nothing to import but components: no
stylesheet, no provider. Colours come from your app's theme, and every string on screen is a prop.

Status: preview. Components may still change; each is unit-tested in Node and driven on a device by
the gallery's specs.

## Start

Liuli is not on npm yet. It lives in the LingXia repository's npm workspace (`packages/`), and an
lxapp in that workspace depends on it by name:

```json
{ "dependencies": { "@lingxia/view": "*", "@liulidesign/core": "*" } }
```

A page is a GPU View when it exports a named `render`:

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { TextInput } from '@liulidesign/core/TextInput';

export function render() {
  const name = useLocal('name', '');
  return (
    <page className="p-4 gap-3">
      <TextInput bind={name} label="Name" />
      <Button label="Save" variant="primary" isDisabled={!name.value} onClick={() => {}} />
    </page>
  );
}
```

To see every component, run the gallery, an lxapp in `../gallery` with a page per component, the
guides (colour, type, shape, motion), whole-screen patterns and a playground: `lingxia dev` from
that directory.

## Components

<!-- components:start -->
78 components. Import any of them from `@liulidesign/core`, or from its own path; a second component in a row (`AvatarGroup`, `Spinner`) has a path of its own too:

| Import | Exports | What it is |
| --- | --- | --- |
| `@liulidesign/core/Button` | Button | Runs an action when pressed, with variants, sizes and a loading state |
| `@liulidesign/core/ToggleButton` | ToggleButton | A button that turns one option on or off and stays down while on |
| `@liulidesign/core/IconButton` | IconButton | A button that shows only an icon, with a name that is always read |
| `@liulidesign/core/ButtonGroup` | ButtonGroup | Buttons that belong together, apart or joined edge to edge, as one keyboard stop |
| `@liulidesign/core/TextInput` | TextInput | Single-line text field with label, help text, status and an optional clear button |
| `@liulidesign/core/TextArea` | TextArea | Multi-line text field that grows with its text, with label, help text and status |
| `@liulidesign/core/DateInput` | DateInput | A labelled date field that opens a month picker |
| `@liulidesign/core/TimeInput` | TimeInput | A labelled time field that opens an hour and minute picker |
| `@liulidesign/core/Slider` | Slider | A value picked along a track, with its label and the value in words |
| `@liulidesign/core/Field` | Field | Label, help text and validation status around one form control |
| `@liulidesign/core/SearchField` | SearchField | A field for searching what is on the page, with a clear button and a line that says what was found |
| `@liulidesign/core/NumberInput` | NumberInput | A number typed or nudged by steps, kept between a minimum and a maximum |
| `@liulidesign/core/Calendar` | Calendar | A month of days in the page, to choose one day or a span |
| `@liulidesign/core/DateRangeInput` | DateRangeInput | A span of days in a form: a field that opens a calendar |
| `@liulidesign/core/FormLayout` | FormLayout, FormSpan | A form's fields at one rhythm: a column on a phone, two where there is room |
| `@liulidesign/core/DateTimeInput` | DateTimeInput | A day and a time of day as one value |
| `@liulidesign/core/FileInput` | FileInput | Files the user chose, and a way to choose more |
| `@liulidesign/core/Composer` | Composer | Where a message is written and sent |
| `@liulidesign/core/ListInput` | ListInput | A field of rows the user adds and removes |
| `@liulidesign/core/CheckboxInput` | CheckboxInput | One on/off choice with a label, help text and status |
| `@liulidesign/core/RadioGroup` | RadioGroup | One choice from a short list, every option in view |
| `@liulidesign/core/Switch` | Switch | A setting that turns on or off the moment it flips |
| `@liulidesign/core/Select` | Select | One choice from a list that opens on demand |
| `@liulidesign/core/SegmentedControl` | SegmentedControl | One value from a few, as a row of segments with a thumb on the chosen one |
| `@liulidesign/core/Combobox` | Combobox | One choice from a long list, found by typing: in the field itself with a keyboard, in a sheet with a finger |
| `@liulidesign/core/CheckboxList` | CheckboxList | Several on/off choices under one question, any number of which can be chosen |
| `@liulidesign/core/Token` | Token, TokenList | One chosen thing as a pill |
| `@liulidesign/core/MultiSelect` | MultiSelect | Several choices from a list, shown as tokens |
| `@liulidesign/core/Tabs` | Tabs | A row of tabs over one panel, as an underline or a segmented control |
| `@liulidesign/core/NavList` | NavList | A sidebar's list of places, in sections that open and close, with the current one marked |
| `@liulidesign/core/Breadcrumb` | Breadcrumb | Where a page sits: the levels above it, each a way back up |
| `@liulidesign/core/Pagination` | pagesShown, Pagination | Moves through a long set a page at a time |
| `@liulidesign/core/Stepper` | Stepper | Where someone is in a task with a known number of steps |
| `@liulidesign/core/Link` | Link | A link among the words of a sentence |
| `@liulidesign/core/PageControl` | PageControl | Dots for which page of a few is in view |
| `@liulidesign/core/TreeList` | TreeList | Things inside things, opened in place |
| `@liulidesign/core/Badge` | Badge | A short status or category label in a tinted pill |
| `@liulidesign/core/ProgressBar` | ProgressBar, Spinner | How much of a task is done, or a spinner while the amount is unknown |
| `@liulidesign/core/Banner` | Banner | A message in the page about the page or the account, that stays until resolved |
| `@liulidesign/core/EmptyState` | EmptyState | What a page or list shows when it has nothing in it yet, and the way forward |
| `@liulidesign/core/Skeleton` | Skeleton, SkeletonGroup | Quiet shapes that stand in for content while it loads |
| `@liulidesign/core/Toast` | Toast, useToast, ToastRegion | A short message about something that just happened, over the bottom of the screen |
| `@liulidesign/core/CircularProgress` | CircularProgress, CircularProgressGroup | How much is done, as a ring |
| `@liulidesign/core/StatusDot` | StatusDot | How something stands, as a dot and a word |
| `@liulidesign/core/Dialog` | Dialog | Modal panel over the page for a decision or a short task |
| `@liulidesign/core/DropdownMenu` | DropdownMenu, DropdownMenuItem, DropdownMenuCheckboxItem, DropdownMenuGroup, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuSubmenu, DropdownMenuDivider | A short menu of actions that opens from its trigger |
| `@liulidesign/core/Tooltip` | Tooltip | A short hint naming its trigger, shown on hover, focus or a long press |
| `@liulidesign/core/InfoTip` | InfoTip | An info button that opens a short note about what is beside it |
| `@liulidesign/core/BottomSheet` | BottomSheet | A modal panel on the bottom edge that drags between detents and off to close |
| `@liulidesign/core/Popover` | Popover | A panel of content that opens beside its trigger, closed by a tap outside |
| `@liulidesign/core/Drawer` | Drawer | A panel that slides in from a side edge |
| `@liulidesign/core/HoverCard` | HoverCard | A preview of what a word names |
| `@liulidesign/core/ContextMenu` | ContextMenu | A menu for one thing, asked for on the thing |
| `@liulidesign/core/CommandPalette` | CommandPalette | Type to find a command or a place, and run it |
| `@liulidesign/core/Tour` | Tour | A few callouts, one at a time, each pointing at something |
| `@liulidesign/core/Card` | Card, ClickableCard, ClickableCardGroup | A surface that groups related content, or one pressable choice |
| `@liulidesign/core/Item` | Item, ItemList | One row of a list: a label, a second line, and content at either end |
| `@liulidesign/core/Details` | Details, DetailsGroup | A summary row that shows and hides the content under it |
| `@liulidesign/core/Avatar` | Avatar, AvatarGroup | A person or team as a circle: their picture, or their initials |
| `@liulidesign/core/SplitView` | SplitView | A sidebar beside its content, each scrolling on its own; one pane at a time on a phone |
| `@liulidesign/core/Icon` | iconPaths, useIcon, Icon, IconSetProvider | An icon by what it means, from a set an app or theme can replace |
| `@liulidesign/core/Table` | Table | Records in rows and fields in columns, under headings that stay in view |
| `@liulidesign/core/Markdown` | Markdown | Markdown text drawn as Liuli type: a message, a note, a help page |
| `@liulidesign/core/MetadataList` | MetadataList | What is known about one thing, as labels and their values |
| `@liulidesign/core/Section` | Section | A titled part of a page: a heading, then its content |
| `@liulidesign/core/Divider` | Divider | A hairline between two things that belong apart |
| `@liulidesign/core/Kbd` | Kbd | A keyboard shortcut, drawn as key caps |
| `@liulidesign/core/Timer` | formatTimer, Timer | A length of time in steady digits |
| `@liulidesign/core/Code` | Code, CodeBlock | Code as it was written, in a sentence or as a block |
| `@liulidesign/core/Timestamp` | formatTimestamp, Timestamp | When something happened, in the words people use |
| `@liulidesign/core/MessageBubble` | MessageBubble, MessageReasoning | One message of a conversation |
| `@liulidesign/core/QRCode` | QRCode | A code a phone camera reads |
| `@liulidesign/core/VideoPlayer` | VideoPlayer | A video with its controls, over the picture or under it, and full screen |
| `@liulidesign/core/VideoControls` | VideoControls | Play, seek, mute, speed, quality and full screen for a useVideo player |
| `@liulidesign/core/Carousel` | Carousel | A few pages to swipe through, one at a time |
| `@liulidesign/core/Thumbnail` | Thumbnail | A small picture that stands for a bigger one |
| `@liulidesign/core/Lightbox` | Lightbox | Pictures one at a time, over the whole screen |
| `@liulidesign/core/WebView` | WebView | An embedded web page, inline or expanded into a 70% sheet or full screen without reloading |
<!-- components:end -->

Each component also exports its class recipe at `@liulidesign/core/<Name>/recipe`, for styling a raw
primitive to match. Its reference (props, anatomy, accessibility, examples, what an app may
override) is `docs/<Name>.md`; `catalog.json` holds the same for agents and tools.

Beside the components:

| Import | What it is |
| --- | --- |
| `@liulidesign/core/density` | `DensityProvider`: compact form controls and rows for a part of the screen |
| `@liulidesign/core/hue` | `Hue` and `hueFor(text)`: one of the theme's eight category colours, the same for the same text |
| `@liulidesign/core/Icon/set` | The icon set provider, to swap the built-in icons for your own |

`@liulidesign/core/internal/*` builds Liuli's own docs and tests: it is unsupported and absent from the
published package. To test your own screens, render them with `@lingxia/view/testing`.

## Words

Liuli's own words — "Close", "Next page", month names, "2 hours ago" — follow the page's language,
in English and Chinese. A label a component asks for is optional where a generic word fits. An lxapp
changes a word, or adds a language, by naming its `@liuli.*` key in its own `i18n/<tag>.json`.

## Theming

Liuli has no colours of its own: every class names a role of your app's theme pack
(`bg-surface`, `text-muted`), and the host resolves the pack for light, dark and high contrast.

```json
{ "name": "brand", "version": 1, "accent": { "seed": "#0f9d58", "derive": true } }
```

Save that as `theme/pack.json` in your lxapp and the accent, its soft step, their text colours and
the focus ring follow the seed in both appearances; everything you leave out comes from the built-in
pack. `lingxia build` refuses a pack that misses a contrast pair, and the pack is fixed for the app:
there is no switch at run time.

- A pack may go further and set surfaces, text, lines and status colours, as
  `../themes/warm/pack.json` does, or any role in `contracts/view/schema.json`; each role it sets is
  held to that role's contrast pairs.
- A pack does not change Liuli's control heights, type sizes or corner radii yet. Those are
  `tokens.css` in this package; an app retunes one by redefining the token in its own stylesheet.
- To see the gallery under a pack, as its own lxapp beside the default one:
  `node ../scripts/gallery-with-pack.mjs warm` prints the directory it wrote.

## Conventions

- Every interactive component takes a required `label`, which is also its accessible name; help text,
  status messages and disabled reasons are read as its description (`aria-describedby`).
- Sizes are `sm`, `md` and `lg` (32, 40 and 48 pt for most controls). A control shorter than the 44 pt
  touch minimum still takes a finger in a band around it, which the host adds without changing what
  is drawn.
- Booleans are `is*`/`has*`; values are `value` + `onChange` or `bind`; overlays are `isOpen` + `onOpenChange`; sizes are `sm`/`md`/`lg`.
- Choices that belong together sit in a group component, which owns the value and the keyboard:
  `RadioGroup`, `ClickableCardGroup`, `DropdownMenuRadioGroup`. Rows that belong together sit in an
  `ItemList`, so each is read with its position.
- Keyboard focus follows the shape: a row (list item, menu item, underline tab, checkbox, radio or
  switch row) fills with a rounded wash; a box (select, text field, card, segment, thumb) takes a ring.
- Liuli contains no copy: every visible or spoken string arrives as a prop.
- Controls look the same on every OS.
- `data-testid` and any other `data-*` props (`data-state`, `data-current`) land on the same element; a test asserts it for every component. The View takes at most 16 `data-*` props per element, within a byte budget per page.

## Layout of the package

Each component is one folder: `X.tsx` (component and prop types), `X.recipe.ts` (class maps),
`X.doc.ts` (anatomy, accessibility contract, guidance, gaps and limits), `X.examples.tsx` (named examples that
the gallery shows and the tests render) and `X.test.mjs`.

```sh
npm test            # unit tests: each example built by the View compiler and rendered in Node
npm run check       # boundary rules, generated files up to date, types, and test:pages
npm run test:pages  # every gallery page opened in its phone and desk form (src/testing/gallery-pages.mjs, a few minutes)
npm run generate    # rewrite docs/, catalog.json, this file's component list and the gallery's component pages
```
