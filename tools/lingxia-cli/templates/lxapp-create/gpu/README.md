# __APP_DISPLAY_NAME__

A LingXia lxapp: each page is Logic (`pages/*/index.ts`) and a GPU View (`pages/*/index.tsx`).

```bash
lingxia dev            # run it
npm run typecheck      # lingxia gen types && tsc -b
```

Type-check with `npm run typecheck`, or `tsc -b` at the project root: the View reads the Logic's types
from what `tsc -b` builds. The `tsconfig.*.json` files extend bases the CLI writes into `.lingxia/` (not
checked in); `lingxia gen types`, `lingxia dev` and `lingxia build` write them.
