# Cloud functions — worker + dev mocks

Same `lx.fn` contract, two providers:

```
functions.json        worker dir + dev routing (mock/real per function)
server/
  src/functions/*.ts  live — `lingxiao build` / deploy unit
  mocks/*.ts          dev mock — served by `lingxia dev`
```

- `lx.fn(name, handler)`: `name` routes `lx.cloud.invoke(name, payload)`;
  handler gets the payload as `request.input` (sync or async) and returns a
  JSON-serializable value. `throw` → the caller's promise rejects; unknown
  name → `unavailable`. A mock can be promoted to `src/functions/` as-is.
- Typed invoke: export `<Name>Input`/`<Name>Output` from each
  `src/functions/<file>.ts` and `lingxia dev` generates typings for
  `lx.cloud.invoke`. Define these types inline — lingxiao resolves types
  per-file and does not follow imports.

## Mock module rules

Mocks run on a minimal loader, **not a bundler**: single-line `import`/`export`
only, relative paths only (keep the suffix), no npm/bare imports, no dynamic
`import()`. Prefer self-contained single-file mocks.

Mocks are transpiled to `.lingxia/cloud-mocks/` when `lingxia dev` **launches**;
the runtime hot-reloads that directory per invoke, so a `.ts` edit takes effect
on the next `lingxia dev` run, not mid-session.

## Routing & sandbox

- `functions.json` `dev.default` (`"mock"`/`"real"`) + per-function
  `dev.overrides`.
- Mocked `lx.cloud.invoke` is fully offline; `lx.cloud.lingxiao(appId)` still
  needs a logged-in tenant.
- Deployed functions run in a server sandbox: use only `lx.fn` and your own
  relative imports — never device `lx.*`.
