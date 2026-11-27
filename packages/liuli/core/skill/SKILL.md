---
name: liuli
description: >-
  Liuli (@liulidesign/core), the component library for LingXia GPU View pages.
  TRIGGER when a page imports `@liulidesign/core`, `@liulidesign/charts` or `@liulidesign/jinshu`, or a GPU View
  needs a button, field, list row, menu, sheet, dialog, banner, toast, table, chart, document or icon. Read before building UI by hand.
---

# Liuli

Liuli parts are TSX source compiled with your GPU pages: import each from its own path
(`import { Button } from '@liulidesign/core/Button'`). There is no stylesheet or provider to add.

## 1. Look in Liuli first

- Before drawing a control, a row or an overlay yourself, look for it by what the user sees
  (`lingxia ui search <words>`, or `node_modules/@liulidesign/core/catalog.json`).
- A hand-built row loses what the part carries: its accessible name read in order, a 44 pt touch
  band around small controls, start/end mirroring for right-to-left languages, and a disabled
  reason read aloud (`disabledMessage`).
- Charts and figures (bar, line, pie, sparkline, a headline number) come from
  `@liulidesign/charts` under the same rules: its own `catalog.json` and `docs/<Name>.md` in
  `node_modules/@liulidesign/charts`. Never hand-draw a chart.
- Rows that belong together go in `ItemList` (each `Item` is read with its position); choices that
  belong together go in their group (`RadioGroup`, `ClickableCardGroup`).
- Documents an agent writes or a person edits (notes, reports, lessons) come from
  `@liulidesign/jinshu`: `JinshuView` to read, `JinshuEditor` to edit, `JinshuOutline`, `JinshuDiff`.
  Short message text is core's `Markdown`.

## 1a. One page for phone and desktop

- A list and the chosen row's detail is `ListDetail`; main content with a companion beside it is
  `Split`. Both go side by side when wide and stack on a phone, folds included; do not branch the page.
- A page's own header row is `TopBar`. Change layout with `regular:` classes, or
  `useView().env.sizeClass` when the tree itself differs.

## 2. Roles and steps, never raw colours or sizes

- Colours are theme roles: `text-text`, `text-muted`, `text-accent`, `text-danger`, `bg-accent`,
  `bg-text/8`, `border-text/12`. Never hex, never `dark:` on a colour: roles follow appearance.
- Type and spacing are steps: `text-footnote`, `text-body`, `text-title`, `gap-3`, `p-4`.
- Restyle a part through its `className` with the same roles and steps; it is merged last and wins.

## 3. Look a part up, do not guess its props

Before the first use, `lingxia ui info <Part>` prints its props with their allowed values and the
path of `docs/<Name>.md` (examples, accessibility); never guess a `variant` or `size` value.
Common slips:

- Test ids are `data-testid`, never `testId`.
- `Button` takes `label`, not children.
- Booleans are `is*`/`has*` (`isDisabled`, `isLoading`); values are `value` + `onChange` or `bind`;
  overlays are `isOpen` + `onOpenChange`.
- An async action goes in `clickAction` (the button shows loading until it settles); `onClick` is
  for synchronous work.

## 4. Icons through `Icon` and icon props

- Draw icons with `Icon` (`name="search"`) or a part's icon prop (`startIcon`, `endIcon`, `icon`),
  by a name from the set or as an `IconDefinition`: one module from the set
  (`import { addCircleIcon } from '@liulidesign/core/Icon/icons/add-circle'`) or the app's own
  `{ paths: [...] }`.
- Never inline SVG or emoji as an icon. An icon-only control needs a `label`: `IconButton`
  (`label` and `icon` required) or `Button` with `isIconOnly`; a lone meaningful `Icon` takes one.

## Words

- Every word the user sees comes from the app's message catalogue (`t('orders.empty')`), never a
  literal in a prop.
- Liuli's own words ("Close", month names, "2 hours ago") follow the page's language; an app
  changes one by naming its `@liuli.*` key in its own `i18n/<tag>.json`.
