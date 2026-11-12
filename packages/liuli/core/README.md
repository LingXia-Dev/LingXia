# @liulidesign/core

Liuli (琉璃) is LingXia's design system for GPU pages: styled components written in `@lingxia/view`
TSX and Tailwind classes, inspired by Astryx. The package ships TSX source; the View compiler
compiles it with the app's pages, checks every class it uses, and names its files by package in
errors.

Status: lab. Six components, verified by Node unit tests and the compiler only, not yet on a device.

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
| `@liulidesign/core/CheckboxInput` | CheckboxInput: one on/off choice with label and help text |
| `@liulidesign/core/Dialog` | Dialog: modal panel, `isOpen` + `onOpenChange` |
| `@liulidesign/core/DropdownMenu` | DropdownMenu, DropdownMenuItem, DropdownMenuDivider |

Each also exports its class recipe at `@liulidesign/core/<Name>/recipe`, for styling a raw primitive
to match.

## Conventions

- Every interactive component takes a required `label`, which is also its accessible name; help text,
  status messages and disabled reasons are read as its description (`aria-describedby`).
- Sizes `sm` and `md` are below the 44 pt touch minimum until hit slop exists; prefer `lg` on touch.
- Booleans are `is*`/`has*`; values are `value` + `onChange` or `bind`; overlays are `isOpen` + `onOpenChange`; sizes are `sm`/`md`/`lg`.
- Liuli contains no copy: every visible or spoken string arrives as a prop.
- Controls look the same on every OS.

## Layout of the package

Each component is one folder: `X.tsx` (component and prop types), `X.recipe.ts` (class maps),
`X.doc.ts` (anatomy, accessibility contract, guidance, gaps), `X.examples.tsx` (named examples that
the gallery shows and the tests render) and `X.test.mjs`.

```sh
npm test            # unit tests: each example built by the View compiler and rendered in Node
npm run check       # boundary rules (imports, engine globals, class assembly, layout tags, files)
npm run typecheck
```
