# Workflows

## How to release — the only two buttons you press

```text
Release / 1. Prepare Version PR  (workspace version) → review and merge the PR
Release / 2. Publish Workspace   (same version)      → tag and publish everything
```

1. **Release / 1. Prepare Version PR** accepts only `component=all`. It moves
   every Rust workspace crate, SDK, CLI/Runner, and npm package to one version
   and opens a PR.
2. **Release / 2. Publish Workspace** verifies that unified version on `main`,
   then publishes crates → SDKs → CLIs/Runners → all npm packages in order.

Independent or partial version bumps and releases are unsupported. If a unified
release is interrupted, recover it by rerunning the failed internal executor for
the existing release tags; humans cannot start a real internal publish directly.

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
