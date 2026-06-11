# Workflows

## How to release — the only two buttons you press

```
Release / 1. Prepare Version PR     (pick component + bump)  → review & merge the PR
Release / 2. Publish Component      (same component + version) → tags + publishes
```

1. **Release / 1. Prepare Version PR** — choose the version scope (`all`, `cli`,
   `npm:<package>`, …). It bumps versions and opens a PR. Merge it.
2. **Release / 2. Publish Component** — choose the component (`crates` / `sdk` /
   `cli` / `npm`) and the version you just merged. It verifies the merged
   version, creates the release tag, then dispatches and waits for the matching
   executor below.

## Everything else is plumbing

| Workflow | Role |
| --- | --- |
| `CI` | push/PR checks on `main` |
| `Release Executor / Crates (internal)` | crates.io publish — dispatched by Release / 2 |
| `Release Executor / SDK (internal)` | SDK artifacts upload — dispatched by Release / 2 |
| `Release Executor / CLI (internal)` | CLI binaries + GitHub Release — dispatched by Release / 2 |
| `Release Executor / NPM (internal)` | npm publish per package — dispatched by Release / 2 |

The executors are `workflow_dispatch`-only (tag-push triggers were removed:
tags are pushed with `GITHUB_TOKEN`, which never triggers workflows, so that
path was dead code — and a side door around Release / 2's version checks).
Run an executor by hand only to debug, with `dry_run=true`.
