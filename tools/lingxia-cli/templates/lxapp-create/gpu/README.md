# __APP_DISPLAY_NAME__

A LingXia lxapp: each page is Logic (`pages/*/index.ts`) and a GPU View (`pages/*/index.tsx`).

```bash
lingxia dev            # run it; type errors show as you save
lingxia check          # every type error and build refusal, without building
```

`lingxia build` fails on type errors too (`--no-typecheck` skips the check). `tsc -b` at the project
root also works: the View reads the Logic's types from what it builds. The `tsconfig.*.json` files
extend bases the CLI writes into `.lingxia/` (not checked in); `lingxia check`, `gen types`, `dev` and
`build` write them.
