# packages

- `lingxia-bridge`: Web runtime package for LxApp bridge and host integration.
- `lingxia-elements`: Pure JS custom elements (native-backed web components).
- `lingxia-react`: Public React package for lxapp pages.
- `lingxia-vue`: Public Vue package for lxapp pages.
- `lingxia-html`: Public HTML package for lxapp pages.
- `lingxia-page-runtime`: Shared implementation package behind the public framework packages.
- `lingxia-types`: Shared TypeScript type definitions for lxapp logic code and runtime contracts.
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
scripts/release/npm.sh --package elements --publish
scripts/release/npm.sh --package react --publish
scripts/release/npm.sh --package vue --publish
scripts/release/npm.sh --package html --publish
scripts/release/npm.sh --package page-runtime --publish
scripts/release/npm.sh --package types --publish
scripts/release/npm.sh --package skill --publish
```
