# Workflows

## How to release — the only two buttons you press

```text
Release / 1. Prepare Version PR          (workspace version) → review and merge
Release / 2. Publish Workspace Component (same version)      → publish all or one component
```

1. **Release / 1. Prepare Version PR** takes `component=all` or `component=cli`.
   `all` moves every Rust workspace crate, SDK, CLI/Runner, and npm package to
   one version and generates both the exhaustive `CHANGELOG.md` section and
   reviewed notes under `docs/releases/`. `cli` moves only the CLI patch line
   and the Runner that tracks it, for a fix that needs no base release, and
   writes its own reviewed notes file. Either way it opens a PR.
2. **Release / 2. Publish Workspace Component** verifies that same unified
   version on `main`, then publishes `all`, `crates`, `sdk`, `cli`, or `npm`.
   `all` remains the recommended default and runs crates → SDKs → CLIs/Runners
   → all npm packages in order. GitHub Releases use the notes committed by the
   version PR; the publish step never regenerates prose after review.

Apart from the CLI patch line, component selection controls what is published,
never its version: everything else moves together at the workspace version, and
any component can be published on its own at that version. If a publish is interrupted, rerun its original
bot-dispatched executor; humans cannot start a new real internal publish directly.

## Everything else is plumbing

| Workflow | Role |
| --- | --- |
| `CI` | Push/PR checks on `main` |
| `Release Executor / Crates (internal)` | crates.io publish dispatched by Release / 2 |
| `Release Executor / SDK (internal)` | SDK artifact upload dispatched by Release / 2 |
| `Release Executor / CLI (internal)` | CLI binaries and GitHub Release dispatched by Release / 2 |
| `Release Executor / NPM (internal)` | npm publish dispatched by Release / 2 |

## 启用 Harmony HAR CI

`Harmony SDK HAR` 需要安装了 DevEco `ohpm`/`hvigorw` 的可信 self-hosted
runner。仓库默认不调度该 job，避免未分配 runner 时 required check 永久排队。
管理员完成 runner 分配后，在 repository 或 organization Actions variables 中设置
`HARMONY_HAR_CI_ENABLED=true` 即可启用。启用后该 job 仍属于 `CI Success` 的依赖，
HAR 构建失败或取消都会阻止合并；来自 fork 的 pull request 始终不会执行
self-hosted job。

The executors are `workflow_dispatch`-only because tags pushed with
`GITHUB_TOKEN` do not trigger downstream workflows. Run an executor by hand
only to debug with `dry_run=true`.
