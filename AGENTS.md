LingXia is a cross-platform app runtime monorepo: Rust crates (`crates/`), npm packages (`packages/`), platform SDKs (`lingxia-sdk/`), and the `lingxia`/`lxdev` CLIs (`tools/`). See `Readme.md` for the full repository layout. (`CLAUDE.md` is just an `@AGENTS.md` import — edit this file, not that one.)

## Docs skill

The repo ships agent-oriented docs as a skill rooted at [docs/skill/SKILL.md](docs/skill/SKILL.md) — the entrypoint and topic router for building on LingXia. Read it first when working on lxapps, host apps, the CLIs, or Rust native extensions; load sub-files only as it directs:

- `docs/skill/lxapp/` — page authoring, native components, `lx.*` API, bridge mechanics
- `docs/skill/app/` — host app projects, `lingxia.yaml`, Apple SDK embedding, app links
- `docs/skill/cli/` — `lingxia` and `lxdev` command references, distribution
- `docs/skill/native/` — Rust native routes and host addons
- `docs/skill/reference/` — file lifecycle

## Example projects

Two examples under `examples/`, one per project shape:

- [examples/lingxia-chat](examples/lingxia-chat) — **standalone lxapp** only: `lxapp.json` + `pages/`, no native host. The minimal reference for lxapp structure.
- [examples/lingxia-showcase](examples/lingxia-showcase) — **native host app + embedded lxapp**: one host app for Android/iOS/macOS/HarmonyOS/Windows embedding the showcase lxapp; kitchen-sink demo of pages, the `lx.*` surface, native components, media, and the terminal. Builds need an explicit `--framework react|vue` (its home pages ship both implementations).

## Build & verify

- Rust gate: `cargo check --workspace --all-targets` (CI runs clippy on the same scope). The workspace default feature set skips the Windows browser shell — cover it with `cargo clippy -p lingxia-windows-sdk --features browser-shell`.
- JS deps live in the `packages/` npm workspace (repo root is not a workspace). `lingxia-cli`'s `build.rs` embeds JS from `packages/lingxia-bridge` and `packages/lingxia-polyfills`, so `npm install` there before the first cargo build; each example project also needs its own `npm install`.
- CLIs: `cargo build -p lingxia-cli -p lingxia-devtools-cli` → `target/debug/lingxia(.exe)` and `lxdev(.exe)`; neither is on PATH. For day-to-day use, copy both into `~/.local/bin` (ensure it's on PATH) so they survive `target/` churn and work from any directory. Re-copy after changing CLI code — a stale copy silently mismatches the repo.
- Run the showcase on desktop: from `examples/lingxia-showcase`, `lingxia dev -p windows|macos --release --framework react`, then drive it with `lxdev` (`lxdev browser …`, `lxdev lxapp eval …`, `lxdev logs --grep X`). A plain `cargo build` of the host produces an app without the dev websocket — always launch via `lingxia dev`.

## Conventions

- Commit messages: conventional commits (`feat(browser): …`, `fix(windows): …`), imperative mood. 
- Comments explain only the non-obvious why; don't restate the code. Keep doc comments terse.
- Rust code is rustfmt-formatted; run `cargo fmt` before committing.
