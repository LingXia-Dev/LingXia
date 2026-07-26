# Workflows

## How to release — the only two buttons you press

```text
Release / 1. Prepare Version PR          (workspace version) → review and merge
Release / 2. Publish Workspace Component (same version)      → publish all or one component
```

1. **Release / 1. Prepare Version PR** accepts only `component=all`. It moves
   every Rust workspace crate, SDK, CLI/Runner, and npm package to one version
   and opens a PR.
2. **Release / 2. Publish Workspace Component** verifies that same unified
   version on `main`, then publishes `all`, `crates`, `sdk`, `cli`, or `npm`.
   `all` remains the recommended default and runs crates → SDKs → CLIs/Runners
   → all npm packages in order.

Independent version bumps are unsupported: component selection controls what is
published, never its version. Independent component publishing is supported at
the current workspace version. If a publish is interrupted, rerun its original
bot-dispatched executor; humans cannot start a new real internal publish directly.

## Everything else is plumbing

| Workflow | Role |
| --- | --- |
| `CI` | Push/PR checks on `main` |
| `Release Executor / Crates (internal)` | crates.io publish dispatched by Release / 2 |
| `Release Executor / SDK (internal)` | SDK artifact upload dispatched by Release / 2 |
| `Release Executor / CLI (internal)` | CLI binaries and GitHub Release dispatched by Release / 2 |
| `Release Executor / NPM (internal)` | npm publish dispatched by Release / 2 |

The executors are `workflow_dispatch`-only because tags pushed with
`GITHUB_TOKEN` do not trigger downstream workflows. Run an executor by hand
only to debug with `dry_run=true`.
