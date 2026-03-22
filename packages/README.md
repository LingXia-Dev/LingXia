# packages

- `lingxia-core`: Web runtime package for LxApp rendering, bridge, and host integration.
- `lingxia-components`: Reusable UI component library for LingXia apps.
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
scripts/release/npm.sh --package core --publish
scripts/release/npm.sh --package types --publish
scripts/release/npm.sh --package components --publish
```
