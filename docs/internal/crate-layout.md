# Crate Layout

> **Audience**: contributors adding a crate or moving code between crates.
> The public Rust API is [`lingxia-facade-boundary.md`](lingxia-facade-boundary.md);
> this is the internal side: which crate code belongs in.

## Layers

A crate depends only on crates in its own layer or below.
`scripts/ci/check-crate-layers.mjs` enforces it in CI, and fails on a crate
it has no layer for: give a new crate one there and a line below.

| Layer | Crates | Owns |
|---|---|---|
| 1. Contracts | `app-context`, `provider`, `log`, `messaging`, `settings`, `surface`, `shell`, `update`, `control-protocol` | Types, policy, wire formats. No runtime. |
| 2. Platform | `webview`, `platform`, `windows-contract`, `device-io` | OS and WebView hosting. Knows nothing of lxapps. |
| 3. Runtime | `lxapp`, `logic`, `service`, `transfer`, `media`, `browser`, `terminal`, `terminal-config` | What a shipped app runs. |
| 4. Products and dev features | `browser-shell`, `proxy`, `control-commands`, `automation` | Built on the runtime; pulled in by facade features. |
| 5. Facade | `lingxia` | The app-author API, the platform entry surface, init. |
| 6. Host integrations | `control-runtime`, `windows-sdk` | Linked by host apps beside the facade. |

Build tooling sits outside the layers: `native-macros`, `native-codegen`,
`windows-build`.

## Rules

1. **Down, never up.** The runtime never depends on a product or dev crate,
   and nothing below layer 5 depends on `lingxia`.
2. **The facade is for app authors.** First-party code that needs
   internals depends on the implementation crate, not on a facade export.
   `lingxia::dev` keeps only what a host calls (dev bundles, the simulated
   device); the devtool commands live in `control-runtime`.
3. **A shipped product does not carry dev code.** Development-only code
   sits behind a feature a product build leaves off (`devtool`,
   `test-runtime`, `dev-bridge`), or in a crate only those features pull in
   (`automation`). A lower crate may expose a hook for it behind such a
   feature (`lxapp`'s `automation` lower half, `dev-bundles`), not the
   feature itself.
4. **A description is a claim.** Each crate's `description` says what it
   owns. When the contents drift from it, move the code or fix the
   description in the same change.

## What each crate owns

Names omit the `lingxia-` prefix.

- `app-context`: product identity and configuration shared by every layer.
- `provider`: host provider contracts (registry, update, logs).
- `log`: structured log domain and the in-memory pipeline.
- `messaging`: cross-platform message passing primitives.
- `settings`: preferences store.
- `surface`: surface roles, kinds and positions (Adaptive Surface Layout core).
- `shell`: platform-neutral shell state and policy (sidebar actions, Pins).
- `update`: update domain models and policy.
- `control-protocol`: control and dev-session wire types and method names.
- `webview`: generic WebView hosting; no lxapp knowledge.
- `platform`: the platform layer for Android, iOS, macOS, HarmonyOS, Windows.
- `windows-contract`: Windows host UI traits shared by `platform` and `windows-sdk`.
- `device-io`: sessionless desktop input, AX, windows, clipboard, processes.
- `lxapp`: the lxapp container and runtime: pages, bridge, host APIs, updates.
- `logic`: the JavaScript (Logic) runtime.
- `service`: service APIs shared by JS bindings, shell and facade.
- `transfer`: downloads and uploads; network owner attribution.
- `media`: media playback and streaming contracts.
- `browser`: the browser runtime capability.
- `terminal`, `terminal-config`: terminal backend, and its configuration.
- `browser-shell`: the browser shell product and its host registrations.
- `proxy`: local CONNECT proxy with PAC routing and capture.
- `control-commands`: the command surface `lxdev` and shipped products share.
- `automation`: `lx.automation()`, the host test runtime, and dev and test
  network interception and observation.
- `lingxia`: the facade.
- `control-runtime`: control dispatch, local IPC, the dev-session bridge,
  and the devtool commands' implementations.
- `windows-sdk`: the Windows host shell.
