# packages

- `lingxia-bridge`: Web runtime package for LxApp bridge and host integration.
- `lingxia-polyfills`: ES5 polyfill bundle shipped with the runtime for old WebView targets.
- `lingxia-elements`: Pure JS custom elements (native-backed web components).
- `lingxia-react`: Public React package for lxapp pages.
- `lingxia-vue`: Public Vue package for lxapp pages.
- `lingxia-html`: Public HTML package for lxapp pages.
- `lingxia-page-runtime`: Shared implementation package behind the public framework packages.
- `lingxia-terminal-settings`: SDK-owned settings app for the desktop terminal.
- `@lingxia/browser-shell-webui` lives next to its crate at `crates/lingxia-browser-shell/webui` (not in this workspace). It is still released by `scripts/release/npm.sh`.
- `lingxia-types`: Shared TypeScript type definitions for lxapp logic code and runtime contracts.
- `lingxia-test`: Authoring SDK and clock for lxapp tests (`spec`, locators, `t.expect`). Run by `lxdev test`.
- `lingxia-skill`: Agent skill (plain markdown, Anthropic Skills layout) for the LingXia framework. Installs via `npx @lingxia/skill install` so any AI coding tool — Claude Code, Claude Agent SDK, OpenAI Codex, Cursor — can build on LingXia. Content is synced from `docs/skill/` at publish time.

## Release

Use the unified release scripts from repository root:

```bash
scripts/release/main.sh doctor
scripts/release/main.sh npm --package all --dry-run
scripts/release/main.sh npm --package all --publish
```

Or run package-specific release:

```bash
scripts/release/npm.sh --package bridge --publish
scripts/release/npm.sh --package polyfills --publish
scripts/release/npm.sh --package elements --publish
scripts/release/npm.sh --package react --publish
scripts/release/npm.sh --package vue --publish
scripts/release/npm.sh --package html --publish
scripts/release/npm.sh --package page-runtime --publish
scripts/release/npm.sh --package terminal-settings --publish
scripts/release/npm.sh --package browser-shell-webui --publish
scripts/release/npm.sh --package types --publish
scripts/release/npm.sh --package test --publish
scripts/release/npm.sh --package skill --publish
```
