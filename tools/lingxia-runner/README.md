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

## Permissions

A project running in the Runner is a guest even when it occupies the home slot.
With no registry provider and no env, guests are unrestricted (public network
and every privilege class). To constrain named apps:

```sh
LINGXIA_RUNNER_LXAPP_PERMISSIONS='{"lingxia-chat":{"domains":["www.deepseek.com"]}}' lingxia dev
```

Each entry takes `{"domains": [...], "privileges": [...]}`. An omitted field
stays unconstrained; an empty list denies that half. An unlisted app is one this
local registry has no policy for, so it stays unrestricted — list it with empty
arrays to test a denial. Malformed JSON denies every app: a lookup that merely
failed would read as no answer and restrict nothing. Leave this
variable unset when a real registry provider is linked in. See the
[permission contract](../../docs/skill/native/permissions.md) for lifecycle and
integration details.
