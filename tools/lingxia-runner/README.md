# LingXia Runner

The development host that `lingxia dev` launches for standalone lxapp projects
on desktop. The CLI keeps the installed Runner version aligned with its own
version and passes the lxapp path at launch.

| Directory | Contents |
|---|---|
| `macos/` | SwiftPM `LingXia Runner.app` plus its Rust static library (`macos/native`, crate `lingxia-runner-lib`) |
| `windows/` | Rust executable crate for the Windows runner, on top of `crates/lingxia-windows-sdk` |
| `config/` | shared crate resolving runner configuration (`~/.lingxia/runner/config.toml`, env vars, lxapp function routing) |

Platform startup code stays in its platform directory. Add shared Rust here
only for real cross-platform runner behavior — `config/` is the model — never
as a dumping ground for platform-specific code.

Build, install, and release specifics: [macOS](./macos/README.md) ·
[Windows](./windows/README.md).
