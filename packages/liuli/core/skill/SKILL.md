---
name: liuli
description: >-
  Liuli (@liulidesign/core), the component library for LingXia GPU View pages.
  TRIGGER when a page imports `@liulidesign/core`, or a GPU View needs a button, field,
  list row, menu, sheet, dialog, banner, toast, table or icon. Read before building UI by hand.
---

# Liuli

Liuli parts are TSX source compiled with your GPU pages: import each from its own path
(`import { Button } from '@liulidesign/core/Button'`). There is no stylesheet or provider to add.

## 1. Look in Liuli first

- Before drawing a control, a row or an overlay yourself, look for it by what the user sees in
  `node_modules/@liulidesign/core/catalog.json` (`summary`, `keywords`, `exports`).
- A hand-built row loses what the part carries: its accessible name read in order, a 44 pt touch
  band around small controls, start/end mirroring for right-to-left languages, and a disabled
  reason read aloud (`disabledMessage`).
- Charts and figures (bar, line, pie, sparkline, a headline number) come from
  `@liulidesign/charts` under the same rules: its own `catalog.json` and `docs/<Name>.md` in
  `node_modules/@liulidesign/charts`. Never hand-draw a chart.
- Rows that belong together go in `ItemList` (each `Item` is read with its position); choices that
  belong together go in their group (`RadioGroup`, `ClickableCardGroup`).

## 2. Roles and steps, never raw colours or sizes

- Colours are theme roles: `text-text`, `text-muted`, `text-accent`, `text-danger`, `bg-accent`,
  `bg-text/8`, `border-text/12`. Never hex, never `dark:` on a colour: roles follow appearance.
- Type and spacing are steps: `text-footnote`, `text-body`, `text-title`, `gap-3`, `p-4`.
- Restyle a part through its `className` with the same roles and steps; it is merged last and wins.

## 3. Look a part up, do not guess its props

Read `node_modules/@liulidesign/core/docs/<Name>.md` (props, allowed values, examples,
accessibility) before the first use; never guess a `variant` or `size` value. Common slips:

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
