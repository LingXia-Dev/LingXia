# Adding or changing a component

## The five files

A component `X` is the folder `src/X/` with exactly these files, plus an `./X` and `./X/recipe`
entry in `package.json` `exports`:

| File | Holds |
| --- | --- |
| `X.tsx` | The component and its exported `XProps` type. Prop comments become the reference. |
| `X.recipe.ts` | Class maps, `as const`; a map keyed by a prop's values is `satisfies Record<ThatType, string>`. Every leaf is one string literal. |
| `X.doc.ts` | Summary, anatomy, accessibility contract, guidance, gaps, and the list of examples. |
| `X.examples.tsx` | One exported function per example named in the doc. The gallery shows them; the tests render them. |
| `X.test.mjs` | Node tests that build and render the examples through `src/testing/harness.mjs`. |

After editing any of them, run `npm run generate`: it rewrites `docs/X.md`, `catalog.json` and the
gallery's pages. Never edit those by hand; `npm run check` fails when they are stale.

Foundations and Patterns pages are hand-written: list each in `../gallery/guides.json` and add its
tile drawing to `../gallery/shared/Specimens.tsx`; the generator adds it to the home page and manifest.

## Rules `npm run check` enforces

- Import only public `@lingxia/view` exports (`@lingxia/view/testing` in tests only); no relative
  import leaves the package.
- No engine or browser globals (`Intl`, `globalThis`, `window`, `document`, `navigator`, timers,
  `Date`), no OS or form-factor branching.
- Classes come from the recipe and `cx`; no class literals in components, no template or `+`
  assembly. Recipes use logical sides (`ps`, `me`, `start-*`), theme roles and no `dark:`.
- Layout uses `view` with flex classes, not the `column`/`row`/`grid`/`stack`/`spacer` tags.
- Props: booleans are `is*`/`has*`; no `disabled`, `checked`, `open`, `onPress`, `onClose`,
  `onValueChange`, `style`, `ref`, or `left*`/`right*` names.

Not checked but required: every visible or spoken string arrives as a prop; the control looks the
same on every OS; no backdrop blur over moving content; every interactive component takes a
`label` as its accessible name.

## Running

```sh
npm test            # Node only: compiler + testing renderer
npm run check
npm run typecheck
```

Node tests do not prove device behaviour. Run the gallery (`../gallery`) with `lingxia dev` before
calling a change verified, and say which platforms it ran on.
