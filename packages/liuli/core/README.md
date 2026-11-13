# @liulidesign/core

Liuli (琉璃) is LingXia's design system for GPU pages: styled components written in `@lingxia/view`
TSX and Tailwind classes, inspired by Astryx. The package ships TSX source; the View compiler
compiles it with the app's pages, checks every class it uses, and names its files by package in
errors.

Status: preview. Nineteen components, each unit-tested in Node and driven on devices by the gallery's
specs (`../gallery/zz-tests`).

## What it needs

- An lxapp with `"renderer": "gpu"` and `@lingxia/view` as a dependency.
- Nothing else: no stylesheet import, no provider. Liuli's scale tokens (`tokens.css`: control
  heights, radii) reach every page that uses a Liuli component, below the app's own stylesheet, so
  the app can redefine any of them. Colours are the app's theme roles.
- The token values are provisional (heights 32/40/48, radii 10/14/20 for control, container and
  dialog). They will be aligned with the theme pack's own tokens: control heights that grow on touch
  pointers and named radius roles.

## Use

One import path per component; there is no package root import.

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

| Import | What |
| --- | --- |
| `@liulidesign/core/Button` | Button: variants, sizes, loading, async `clickAction` |
| `@liulidesign/core/Field` | Field: label, help text and status around a raw control |
| `@liulidesign/core/TextInput` | TextInput: one-line editor, bound or controlled, optional clear button |
| `@liulidesign/core/Select` | Select: one choice from a list that opens on demand |
| `@liulidesign/core/CheckboxInput` | CheckboxInput: one on/off choice with label and help text |
| `@liulidesign/core/RadioGroup` | RadioGroup: one choice from a short list, as rows or cards |
| `@liulidesign/core/Switch` | Switch: a setting that takes effect at once, alone or as a row |
| `@liulidesign/core/Slider` | Slider: a value along a track, alone or as a row |
| `@liulidesign/core/Tabs` | Tabs: underline or segmented, over one panel |
| `@liulidesign/core/Dialog` | Dialog: modal panel, `isOpen` + `onOpenChange` |
| `@liulidesign/core/DropdownMenu` | DropdownMenu, DropdownMenuItem, DropdownMenuDivider, DropdownMenuCheckboxItem, DropdownMenuRadioGroup, DropdownMenuRadioItem |
| `@liulidesign/core/Tooltip` | Tooltip: an info button that opens a short note |
| `@liulidesign/core/Item` | Item and ItemList: a list row, and the list that reads rows with their position |
| `@liulidesign/core/Card` | Card, ClickableCard and ClickableCardGroup: grouped content, one pressable card, or cards that choose one value |
| `@liulidesign/core/Details` | Details and DetailsGroup: a row that shows and hides content, and an accordion |
| `@liulidesign/core/Banner` | Banner: a message about the page that stays until resolved |
| `@liulidesign/core/Badge` | Badge: a short status or category label |
| `@liulidesign/core/Avatar` | Avatar and AvatarGroup: a person as a picture or initials |
| `@liulidesign/core/ProgressBar` | ProgressBar and Spinner: an amount done, or a wait |

Each also exports its class recipe at `@liulidesign/core/<Name>/recipe`, for styling a raw primitive
to match. The full reference for each component (props, anatomy, accessibility, examples) is in
`docs/<Name>.md`; `catalog.json` holds the same for agents.

## Conventions

- Every interactive component takes a required `label`, which is also its accessible name; help text,
  status messages and disabled reasons are read as its description (`aria-describedby`).
- Sizes `sm` and `md` are below the 44 pt touch minimum until hit slop exists; prefer `lg` on touch.
- Booleans are `is*`/`has*`; values are `value` + `onChange` or `bind`; overlays are `isOpen` + `onOpenChange`; sizes are `sm`/`md`/`lg`.
- Choices that belong together sit in a group component, which owns the value and the keyboard:
  `RadioGroup`, `ClickableCardGroup`, `DropdownMenuRadioGroup`. Rows that belong together sit in an
  `ItemList`, so each is read with its position.
- Keyboard focus follows the shape: a row (list item, menu item, underline tab, checkbox, radio or
  switch row) fills with a rounded wash; a box (select, text field, card, segment, thumb) takes a ring.
- Liuli contains no copy: every visible or spoken string arrives as a prop.
- Controls look the same on every OS.

## Layout of the package

Each component is one folder: `X.tsx` (component and prop types), `X.recipe.ts` (class maps),
`X.doc.ts` (anatomy, accessibility contract, guidance, gaps), `X.examples.tsx` (named examples that
the gallery shows and the tests render) and `X.test.mjs`.

```sh
npm test            # unit tests: each example built by the View compiler and rendered in Node
npm run check       # boundary rules, and generated files up to date
npm run generate    # rewrite docs/, catalog.json and the gallery's component pages from the doc files
npm run typecheck
```
