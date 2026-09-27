# `lingxia` CLI

The project lifecycle: scaffold, start dev sessions, build, package. Driving a
running session is [`lxdev`](./lxdev.md); signing, publishing, and stores are
in [Distribution](./distribution.md).

## `--help` is the source of truth

This file teaches what each command is for. It lists no flags, defaults, or
value enums; the installed binary's `--help` is exhaustive and current.

```bash
lingxia --help               # the command list + global flags
lingxia <cmd> --help         # exact flags, defaults, and which are required
lingxia <cmd> <sub> --help   # e.g. lingxia auth login apple --help
```

Platforms: `android`, `ios`, `macos`, `harmony`, `windows`.

## `lingxia new`

Scaffold a project, interactively or scripted:

```bash
lingxia new my-lxapp -t lxapp -y                                   # standalone lxapp
lingxia new my-app -t native-app -p macos,windows --package-id com.example.myapp -y
lingxia new my-terminal -t native-app --main terminal --control native -y
lingxia new my-browser -t native-app -p windows --main browser --control lxapp -y
```

- `--main lxapp|terminal|browser` picks a host's main experience (default
  `lxapp`); `--control lxapp|native` picks where host control lives. Terminal
  and browser mains are macOS/Windows and default to native control. Both flags
  are host-only, and native control with an lxapp main is rejected.
- `--template <name>` scaffolds an lxapp from an installed template provider;
  arguments after `--` go to the template:

```bash
lingxia new my-lxapp --template acme-starter --yes -- --preset dashboard
```

## `lingxia template`

`add <git-url|local-repo>`, `list`, `update [name]`, `remove <name>` manage
template providers: repositories with a `lingxia-template.json` manifest that
may ship project files, CLI commands, and skills.

## `lingxia dev`

Starts a dev session. In a host project it builds, installs, launches, and
opens the dev websocket `lxdev` connects to; in an lxapp it launches the
LingXia Runner (macOS/Windows).

```bash
lingxia dev                                # this project
lingxia dev ../my-lxapp                    # Runner target elsewhere; state stays here
lingxia dev <http(s)://url> --headless --background
lingxia dev -p runner                      # the Runner explicitly
lingxia dev --display-language sr-Latn-RS  # or auto; this process only
lingxia dev --background --mock all        # answer from mocks/ (CI against mocks)
```

- **Takeover.** Re-running for the same platform stops that project's session
  and starts fresh. Different platforms run side by side.
- **Watch and reload.** Saving a standalone lxapp or a local
  `resources.bundles[].path` rebuilds and reloads it in place (`pages`,
  `tabBar`, `navigationStyle` included). Host code needs a new `lingxia dev`.
- **Background.** `--background` returns once the session is ready (`--json`
  prints it). On failure or no readiness within 30 minutes it stops what it
  started, prints the log tail, and exits non-zero.
- **Names.** `--name NAME` gives a stable alias for `lxdev --session` and
  `lingxia dev stop`.
- **Mocks.** `--mock all|none` answers the session from `mocks/` handlers or
  the real backend from its first request, replacing `mocks/config.json`;
  the ready output prints the selection. [Mocks](../lxapp/mock.md).
- **Stop.** `lingxia dev stop [SESSION]` ends a session (exit 0 when none).
  `lxdev session` lists live ones.
- **Version skew.** `dev`, `build`, and `lxdev test` fail fast when the CLI,
  the host or Runner, and the project's `@lingxia/*` packages are on different
  major.minor lines; the message names the fix. `LINGXIA_ALLOW_SKEW=1` turns
  it into a warning; `lingxia doctor --project` prints every version.
- **Devices.** Android and Harmony get reverse port forwarding. iOS devices
  connect over the LAN, so the device must reach the Mac; set
  `LINGXIA_DEV_HOST` to override the detected address.
- **Remote machines.** Run `lingxia dev` and `lxdev` on the same machine (over
  SSH if needed). On Windows over SSH the same account must be signed in to
  the desktop; use `--background`.

## `lingxia build`

`--env dev|prod` picks the host environment (package-id suffix, server);
`--release` picks the compiler profile. They are independent:
`lingxia build --env prod --release` is shippable. See
[Environment](../app/project.md#environment).

`build` can also sign and package per platform (iOS IPA, macOS DMG, Windows
MSIX, Android APK or AAB), build only the native library, reuse native
binaries, add native features or a provider crate, and pick Android ABIs or
macOS arch. It generates the native client when `lxapp.config.ts` declares
`native`, and enforces the [View/Logic boundary](../lxapp/guide.md#build).
Signing setup: [Distribution](./distribution.md#app-signing).

### iOS Packet Tunnel extensions

`ios/PacketTunnel/Info.plist` opts in. Provide a SwiftPM executable
product/target named `PacketTunnel`; the CLI embeds it as
`PlugIns/PacketTunnel.appex` with bundle id `<app bundle id>.PacketTunnel`.
Extension entitlements go in `ios/PacketTunnel/PacketTunnel.entitlements`.
Device builds need the Network Extension capability and provisioning for app
and extension.

## `lingxia clean`

Removes generated artifacts (host outputs and platform build directories, or
an lxapp's `dist/` and caches). Use it when a `lingxia.yaml` change seems
ignored after a rebuild.

## `lingxia package`

A release build staged for delivery: Android under `dist/android/`, macOS
update zips under `dist/macos/`, Windows under `dist/windows/`.

```bash
lingxia package -p windows                         # NSIS + update ZIP
lingxia package -p windows --format nsis,portable,zip
lingxia package -p windows --format msix             # OS-managed distribution
lingxia package -p windows --msix --self-signed      # local MSIX testing
```

Windows needs NSIS 3 (or `LINGXIA_MAKENSIS`) for Setup, the Windows SDK for
MSIX. Build every direct format you ship in one run; `*-windows.zip` is the
update payload for `lingxia publish` (MSIX-only builds have none). More:
[Windows](./distribution.md#windows).

## Devices: `devices`, `install`, `uninstall`, `launch`

- `lingxia devices` lists devices; pass the id when more than one is connected.
- `lingxia install` installs a built artifact (auto-detected, or an APK/HAP).
- `lingxia uninstall` removes the app (id from `lingxia.yaml` by default).
- `lingxia launch` starts it; `--restart` works on Android and iOS.

## `lingxia icon`

Generates app icons from one full-bleed source image, or converts to a
standalone `.ico`/`.png`. For Android/Harmony layered icons, keep the
background colour matched to the source, or pass a transparent foreground.

## `lingxia doctor`

```bash
lingxia doctor
lingxia doctor --platform harmony
lingxia doctor --project   # this CLI vs the project's @lingxia/* packages
```

## `lingxia upgrade`

Updates the CLI, `lxdev`, and the Runner. Inside a project it then moves the
project's LingXia pins (npm, crates, platform SDKs, `lxapp.json` `minRuntime`)
to the CLI's major.minor line, after a prompt.

- `--yes` applies without prompting; non-interactive runs need it.
- `--check` reports drift without writing.
- Exit 10 means something is still behind; re-run `lingxia upgrade`.

## Environment variables

| Variable | Used by | Description |
|----------|---------|-------------|
| `ANDROID_SDK_ROOT` | android | Android SDK root path |
| `ANDROID_NDK_ROOT` | android | Android NDK path (e.g. `$ANDROID_SDK_ROOT/ndk/<version>`) |
| `OHOS_NDK_HOME` | harmony | Harmony command-line tools SDK path |
| `JAVA_HOME` | android | Java JDK path |

`lingxia doctor --platform <p>` shows what is missing. Credential and signing
variables are in [Distribution](./distribution.md).
