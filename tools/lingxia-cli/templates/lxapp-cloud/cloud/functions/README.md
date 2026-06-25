# Cloud functions (`lx.cloud`)

Each `.js` file registers a mock LingXiao cloud function with `lx.fn(name, handler)`.
`lingxia dev` serves them **in-process** (no HTTPS, no login), so Logic can call
`await lx.cloud.invoke("name", payload)` while developing offline; edits
**hot-reload** (change a file, call again — no restart). `lx.fn` is the same
contract the real LingXiao runtime uses, so the call site is identical when you
later switch to the real service — these files are dev stubs, not the production
functions.

- One function per file (recommended), or several `lx.fn(...)` in one file.
- `handler(input)` returns JSON-serializable data; sync or `async`.

## Call it from a page

`lx.cloud` lives in the Logic runtime — call it from a page's Logic and push the
result to the View:

```ts
// in a page's index.ts (e.g. pages/home/index.ts)
callHello: async function () {
  const r = await lx.cloud.invoke("hello", { name: "world" });
  this.setData({ result: JSON.stringify(r) });
}
```

> **`import` here is extremely limited** — single-line, relative path (`./...`
> with the `.js` suffix), local helpers only. No npm packages, no `.ts`, no
> multi-line imports, no dynamic `import()`. For anything more, keep the function
> self-contained.

Requires the cloud provider (`lingxia build`/`dev --with-provider cloud`).
Default is mock in dev; set `LINGXIAO_MOCK=0` in the runner env to call the real
service instead.
