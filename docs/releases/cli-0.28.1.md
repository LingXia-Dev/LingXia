# LingXia 0.28.1

## Breaking changes

### Runner configuration commands

`lingxia auth runner`, `lingxia dev --runner`, and `lingxia dev --display-language` have been removed. Save the cloud server with `lingxia auth login lingxia --env <env> --server <url>`; `lingxia dev` passes that environment's saved server to the Runner. Change a running Runner's device, orientation, appearance, and language with `lxdev runner set`; list devices with `lxdev runner presets`.

Use `lxdev runner set --language zh-CN` to override the display language for the current dev session, or `--language auto` to follow the system. `lxdev runner get` reports the effective language. Ending the session clears the override.

### one side panel for lxapps and browser tabs; two aside slots

Asides group into two slots, `panel` (lxapps and browser tabs in one region) and `native` (terminal), instead of `lxapp` / `browser` / `native`; an expanded window admits two. `SurfaceLayoutAsideSlot.kind` (lx automation) is now `'panel' | 'native'`: compare with `'panel'` where tests matched `'lxapp'` or `'browser'`. Rust: `SlotKind::Lxapp` and `SlotKind::Browser` are `SlotKind::Panel`; the Windows contract's `ASIDE_LXAPP_PANEL_ID` and `ASIDE_BROWSER_PANEL_ID` are `ASIDE_PANEL_ID`. Slot APIs still accept `"lxapp"` and `"browser"` as names for the panel slot.

## Features and fixes

- macOS host apps can declare `macos.arch: arm64|x86_64|universal` in `lingxia.yaml`. `lingxia package` (including `--dmg`) honors it; `--macos-arch` overrides it for a build. Dev and plain builds still target the host architecture.
- Universal macOS packaging isolates SwiftPM outputs by architecture, fixing repeated builds that could reuse or remove the other architecture's artifacts. Publishing checks that the executable contains every declared architecture.
- `lingxia upgrade` updates every published LingXia crate while preserving open lower-bound dependency ranges.

This release ships the CLI tools and matching macOS/Windows Runners. The base runtime, SDKs, and npm packages remain on 0.28.0.
