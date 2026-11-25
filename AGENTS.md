LingXia is a cross-platform app runtime monorepo: Rust crates (`crates/`), npm packages (`packages/`), platform SDKs (`lingxia-sdk/`), and the `lingxia`/`lxdev` CLIs (`tools/`). See `Readme.md` for the full repository layout.

## Docs skill

The repo ships agent-oriented docs as a skill rooted at [docs/skill/SKILL.md](docs/skill/SKILL.md) — the entrypoint and topic router for building on LingXia. Read it first when working on lxapps, host apps, the CLIs, or Rust native extensions; load sub-files only as it directs:

- `docs/skill/lxapp/` — page authoring, native components, `lx.*` API, bridge, files, testing
- `docs/skill/app/` — host app projects, `lingxia.yaml`, Apple SDK embedding, app links
- `docs/skill/cli/` — `lingxia` and `lxdev` command references, distribution
- `docs/skill/native/` — Rust native routes and host addons

Skill docs are loaded into an agent's context, so keep them concise: state what to do and the one non-obvious consequence. Leave rationale, history, and edge-case reasoning to the PR and code comments. The skill is written for agents and is self-contained: it never references repository paths or other docs. Human-facing documentation is kept separately and maintained in parallel.

## Internal docs

`docs/internal/` is the other half, and it is not part of the skill: it is for
work *on* this repo, not on apps built with it. Read the relevant one before
changing the subsystem it covers — each states invariants that are not
recoverable from the code alone.

- [`trusted-control-plane.md`](docs/internal/trusted-control-plane.md) — session classes, route audiences, and the ingress lock discipline. Read before touching authorization, session creation, or WebView message ingress.
- [`bridge-protocol.md`](docs/internal/bridge-protocol.md) — normative bridge contract: message families, the WebView frame transport and the GPU in-process transport.
- [`webview-lifecycle.md`](docs/internal/webview-lifecycle.md) — WebView creation, presentation, and teardown across platforms.
- [`logic-runtime.md`](docs/internal/logic-runtime.md) — the Logic JS engine choice, the worker pool and its pump discipline, quarantine of a terminated worker, extension registration, and cross-cutting Logic contracts.
- [`platform-ffi.md`](docs/internal/platform-ffi.md) — `lingxia-platform` trait architecture, `lingxia-messaging`, per-platform FFI boundaries (Android JNI, Apple `swift-bridge`, HarmonyOS N-API, Windows in-process SDK), threading and `capture-contract` vs `device-io` boundaries.
- [`lxapp-container.md`](docs/internal/lxapp-container.md) — `.lxapp` archive extraction, `lxapp.json` validation, startup/bootstrap, page ownership and pinning, the navigation queue, network access guards, and native components.
- [`lingxia-facade-boundary.md`](docs/internal/lingxia-facade-boundary.md) — what stays behind the `lingxia` crate facade.
- [`crate-layout.md`](docs/internal/crate-layout.md) — crate layers, dependency rules, and which crate owns what. Read before adding a crate or moving code between crates.
- [`view-authoring.md`](docs/internal/view-authoring.md) · [`view-renderer.md`](docs/internal/view-renderer.md) · [`view-styling.md`](docs/internal/view-styling.md) · [`view-animation.md`](docs/internal/view-animation.md) · [`view-engine-limits.md`](docs/internal/view-engine-limits.md) — GPU View contract, compiler/authoring diagnostics, renderer/adapter architecture, styling, animations, and wire/engine limits.
- [`shell-ui-spec.md`](docs/internal/shell-ui-spec.md) · [`view-environment-spec.md`](docs/internal/view-environment-spec.md) — the shell surface model and its projections; host-chrome geometry (tab bar, capsule, insets) and how each page receives it.
- [`launch-screen.md`](docs/internal/launch-screen.md) — `splash:`, the OS launch frame, the first-frame cover, dismissal and the campaign hook. Read before changing the CLI splash generation or an SDK's splash overlay.
- [`theme.md`](docs/internal/theme.md) — theme packs, role tiers, scale slots and their run-time resolution, `palette`/`scales` delivery. Read before changing `lingxia-theme`, the schema's `roles`/`scales`, or scale lowering.
- [`terminal-engine.md`](docs/internal/terminal-engine.md) — the single PTY path and terminal-core contract, the frame cell format, Kitty graphics, restore snapshots, and the `terminal.json` fallback.
- [`browser-runtime.md`](docs/internal/browser-runtime.md) — `lingxia-browser` navigation/tab/download ownership, `lingxia-browser-shell` persistence and GPU system pages, and `lingxia-proxy` local CONNECT/MITM/WebView coordination.
- [`host-control-ipc.md`](docs/internal/host-control-ipc.md) — external CLI/IPC control (`dev-bridge` vs `local-control` IPC), single-binary product CLI startup, dev/product feature gating, and dispatch/error framing.
- [`test-runner.md`](docs/internal/test-runner.md) — test isolation, deadlines, result schema, and interrupted-run reporting.
- [`companion-protocol.md`](docs/internal/companion-protocol.md) — how scenario `function` rules and the Function half of mocks reach the dev session's companion: capabilities, `scenario.*` and `mock.*` messages, owners and their lifetime. Read before changing the relay or implementing a companion.
- [`dev-session.md`](docs/internal/dev-session.md) — `lingxia dev` session broker and files, dev websocket transport, bundle sync, mock readiness, and CLI/Runner version checks.
- [`storage.md`](docs/internal/storage.md) · [`update-pipeline.md`](docs/internal/update-pipeline.md) — managed file storage and cache maintenance, update signing and installation.
- [`i18n.md`](docs/internal/i18n.md) — who owns the language, the three text stores, catalogues and fallback, formatting, SDK strings. Read before touching language resolution or any catalogue.
- [`logging.md`](docs/internal/logging.md) · [`env-version.md`](docs/internal/env-version.md) · [`release-versioning.md`](docs/internal/release-versioning.md) — log pipeline, host env vs lxapp channel, release version rules.

## Example projects

Reference projects under `examples/`:

- [examples/lingxia-showcase-web](examples/lingxia-showcase-web) — **standalone WebView lxapp**, no native host: React pages for what only a WebView page shows (native components in a page, the bridge, streams, channels) and a chat. The reference for WebView lxapp structure; run it in the Runner.
- [examples/lingxia-showcase](examples/lingxia-showcase) — **native host app + embedded lxapp**: one host app for Android/iOS/macOS/HarmonyOS/Windows embedding the showcase lxapp; kitchen-sink demo of pages, the `lx.*` surface, View elements, media, and the terminal. Its pages are GPU Views (TSX).

## Build & verify

SDK device probes for the showcase's GPU pages belong
in `lingxia-sdk/android/tests/gpu`, outside app bundles. `lingxia-sdk/fixtures/hello-view`
is the bare-renderer smoke fixture every platform's smoke harness loads. Read `docs/internal/view-authoring.md`,
`view-renderer.md`, `view-styling.md` and `view-animation.md` before changing GPU contracts.
The View contract lives in `contracts/view/`; after editing `schema.json` or `api.d.ts`, run
`node contracts/view/generate.mjs` (CI runs it with `--check`).

- Rust gate: `cargo check --workspace --all-targets`. Run clippy without `--all-targets` (`cargo clippy --workspace`, or `-p <crate>`). The workspace default feature set skips the Windows browser shell — cover it with `cargo clippy -p lingxia-windows-sdk --features browser-shell`.
- JS deps live in the `packages/` npm workspace (repo root is not a workspace). `lingxia-cli`'s `build.rs` embeds JS from `packages/lingxia-bridge` and `packages/lingxia-polyfills`, so `npm install` there before the first cargo build; each example project also needs its own `npm install`.
- CLIs: `cargo build -p lingxia-cli -p lingxia-devtools-cli` → `target/debug/lingxia(.exe)` and `lxdev(.exe)`; neither is on PATH. For day-to-day use, copy both into `~/.local/bin` (ensure it's on PATH) so they survive `target/` churn and work from any directory. Re-copy after changing CLI code — a stale copy silently mismatches the repo.
- Run the showcase on desktop: from `examples/lingxia-showcase`, `lingxia dev -p windows|macos --release`, then drive it with `lxdev` (`lxdev browser …`, `lxdev lxapp eval …`, `lxdev logs --grep X`). A plain `cargo build` of the host produces an app without the dev websocket — always launch via `lingxia dev`.
- Verify platform behavior on real devices. Simulator and emulator runs don't count as verification for device features (audio, Bluetooth, sensors, permission prompts, orientation); if no device is available, say the change is compile-only.

## Conventions

- Commit messages: conventional commits (`feat(browser): …`, `fix(windows): …`), imperative mood. 
- Comments explain only the non-obvious why; don't restate the code. Keep doc comments terse.
- Rust code is rustfmt-formatted; run `cargo fmt` before committing.
