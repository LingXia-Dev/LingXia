# __APP_DISPLAY_NAME__

A LingXia lxapp: each page is Logic (`pages/*/index.ts`) and a GPU View (`pages/*/index.tsx`).

```bash
lingxia dev            # run it
npm run typecheck      # lingxia gen types && tsc -b
```

The `tsconfig.*.json` files extend bases the CLI writes into `.lingxia/` (not checked in), so a fresh
checkout runs `lingxia gen types` once before `tsc`; `lingxia dev` and `lingxia build` write them too.
