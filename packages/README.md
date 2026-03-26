# packages

- `lingxia-bridge`: Web runtime package for LxApp bridge and host integration.
- `lingxia-elements`: Pure JS custom elements (native-backed web components).
- `lingxia-react`: React hook (`useLingXia`) + component wrappers. Depends on bridge + elements.
- `lingxia-vue`: Vue composable (`useLingXia`) + component wrappers. Depends on bridge + elements.
- `lingxia-types`: Shared TypeScript type definitions used by LingXia runtime and apps.

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
scripts/release/npm.sh --package types --publish
```
