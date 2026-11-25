# Crate Layout

> **Audience**: contributors adding a crate or moving code between crates.
> The public Rust API is [`lingxia-facade-boundary.md`](lingxia-facade-boundary.md);
> this is the internal side: which crate code belongs in.

The workspace crates under `crates/` form six layers. A crate depends only on
crates in its own layer or below. The `check-crate-layers` CI script enforces
this (normal dependencies only; dev- and build-dependencies are free),
and fails on a crate it has no layer for: give a new crate a layer there and a
line below.

## Layers

| Layer | Crates | Owns |
|---|---|---|
| 1. Contracts | `app-context`, `provider`, `log`, `messaging`, `settings`, `surface`, `shell`, `update`, `control-protocol`, `theme`, `i18n` | Types, policy, wire formats. No runtime. |
| 2. Platform | `webview`, `platform`, `windows-contract`, `device-io`, `transfer` | OS, WebView hosting and HTTP transfer. Knows nothing of lxapps. |
| 3. Runtime | `lxapp`, `view`, `view-android`, `view-apple`, `view-harmony`, `view-windows`, `logic`, `service`, `media`, `browser`, `terminal`, `terminal-config` | What a shipped app runs. |
| 4. Products and dev features | `browser-shell`, `proxy`, `control-commands`, `network-dev`, `automation` | Built on the runtime; pulled in by facade features. |
| 5. Facade | `lingxia` | The app-author API, the platform entry surface, init. |
| 6. Host integrations | `control-runtime`, `windows-sdk` | Linked by host apps beside the facade. |

Build tooling sits outside the layers and may be used from anywhere:
`native-macros`, `native-codegen`, `windows-build`.

## Rules

1. **Down, never up.** The runtime never depends on a product or dev crate,
   and nothing below layer 5 depends on `lingxia`. Upward edges are what make
   a dev feature leak into a shipped build and what turn the facade into a
   dependency cycle.
2. **The facade is for app authors.** First-party code that needs internals
   depends on the implementation crate, not on a facade export, so the facade
   can stay small without breaking first-party crates. `lingxia::dev` keeps
   only what a host calls (dev bundles, the simulated device); the devtool
   commands live in `control-runtime`.
3. **A shipped product does not carry dev code.** Development-only code sits
   behind a feature a product build leaves off (`lingxia`'s `devtool` and
   `automation`, `control-runtime`'s `dev-bridge` and `test-runtime`), or in a
   crate only those features pull in (`automation`, `network-dev`). A lower
   crate may expose a hook for it behind such a feature (`lxapp`'s
   `automation` and `dev-bundles` features), not the feature itself.
4. **A description is a claim.** Each crate's `Cargo.toml` `description` says
   what it owns. When the contents drift from it, move the code or fix the
   description in the same change.
5. **View and the app runtime meet only in the facade.** `view*` depends on
   none of `lxapp`, `logic`, `service`, `webview`, `platform`, `media`,
   `browser`, `terminal*`, and `lxapp`, `logic`, `service` depend on no
   `view*`; the script's forbidden-edge list enforces it. View renders without
   the app runtime and carries no app authorization; the facade wires the two
   together.

## What each crate owns

Names omit the `lingxia-` prefix.

**Contracts**

- `app-context`: product identity and configuration shared by every layer.
- `provider`: host provider contracts (registry, update, logs).
- `log`: structured log domain and the in-memory pipeline.
- `messaging`: cross-platform message passing primitives.
- `settings`: preferences store.
- `surface`: surface roles, kinds and positions.
- `shell`: platform-neutral shell state and policy (sidebar actions, pins).
- `update`: update domain models and policy.
- `control-protocol`: control and dev-session wire types and method names.
- `theme`: theme packs, colour roles, preference resolution and the contrast proof.
- `i18n`: message catalogues: the ICU MessageFormat subset, the FormatJS JSON
  file and the translation checks.

**Platform**

- `webview`: generic WebView hosting; no lxapp knowledge.
- `platform`: the platform layer for Android, iOS, macOS, HarmonyOS, Windows.
- `windows-contract`: Windows host UI traits shared by `platform` and `windows-sdk`.
- `device-io`: sessionless desktop input, accessibility, windows, clipboard, processes.
- `transfer`: every HTTP fetch LingXia makes for itself: the HTTP store,
  resumable downloads, uploads; network owner attribution. Callers pass the
  destination folder and the network guard; it has no policy of its own.

**Runtime**

- `lxapp`: the lxapp container and runtime: pages, bridge, host APIs, updates.
- `view`: the View language, layout, text, rendering and the platform adapter
  traits; no app authorization.
- `view-android`: the Android adapter for `view`: the GPU view's JNI natives,
  presentation and platform mechanics.
- `view-apple`: the iOS and macOS adapter for `view`: a View host per GPU
  page behind the Swift view, presentation and platform mechanics.
- `view-harmony`: the HarmonyOS adapter for `view`: a View host per GPU page
  behind a native XComponent node, GLES presentation, the accessibility
  provider and the input method.
- `view-windows`: the Windows adapter for `view`: a View host per GPU page on
  its own UI thread, its child window (input, CPU frames through GDI), wgpu
  presentation on that window and the DWM vsync source.
- `logic`: the JavaScript (Logic) runtime.
- `service`: service APIs shared by JS bindings, shell and facade.
- `media`: media playback and streaming contracts.
- `browser`: the browser runtime capability.
- `terminal`, `terminal-config`: terminal backend, and its configuration.

**Products and dev features**

- `browser-shell`: the browser shell product and its host registrations.
- `proxy`: local CONNECT proxy with PAC routing and capture.
- `control-commands`: the command surface `lxdev` and shipped products share.
- `network-dev`: dev and test network layer: Logic `fetch`/SSE interception,
  mocks and scenarios, and the traffic observer behind `lxdev network`, with
  its `lx.automation()` drivers (authorized through the policy automation
  installs).
- `automation`: `lx.automation()` and the host test runtime.

**Facade and host integrations**

- `lingxia`: the facade.
- `control-runtime`: control dispatch, local IPC, the dev-session bridge, and
  the devtool commands' implementations.
- `windows-sdk`: the Windows host shell.
