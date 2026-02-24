# Cache Manager Design Doc

## Purpose

This document explains the cache manager design in `lingxia-lxapp`, including its goals, constraints, invariants, and trade-offs. It is intended for internal maintainers.

## Problem Statement

Each lxapp can continuously write/read cached assets. Without control, cache growth can:

- exceed device storage budgets
- keep stale files forever
- interfere with app startup and runtime stability

We need a design that enforces limits without adding latency to JS file access paths.

## Goals

- Enforce per-lxapp cache size limit.
- Enforce per-lxapp cache age limit.
- Avoid deleting in-progress downloads.
- Keep cleanup off the synchronous `resolve_access` path.
- Keep behavior safe under symlinks and path traversal edge cases.

## Non-Goals

- Global cross-lxapp quota scheduling.
- Hard real-time quota enforcement.
- User-facing cache UI/policy controls in SDK runtime.

## Scope

- Cache root per lxapp: `<host_cache>/lingxia/lxapps/usercache/<lxapp_hash>/`
- Manager implementation: `lingxia-lxapp/src/cache.rs`
- Runtime integration: `lingxia-lxapp/src/appservice.rs`
- Startup sweep hook: `lingxia-lxapp/src/lxapp.rs`

## Configuration Model

Source of truth (developer-facing): `lingxia.config.json`.

At build time, CLI maps host cache settings into generated `app.json`.
Runtime (`lingxia-lxapp`) reads effective values from `app.json`.

- `cacheMaxAgeDays` (default `7`)
- `cacheMaxSizeMB` (default `1024`)

Semantics:

- `cacheMaxAgeDays = 0`: disable age-based eviction.
- `cacheMaxSizeMB = 0`: disable capacity-based eviction.
- both zero: cleanup disabled.

## High-Level Architecture

Each lxapp owns one `CacheCapacityManager` (shared by guard clones via `Arc`).

Execution has two layers:

- Startup sweep:
  - one background scan over existing `usercache/*` directories
  - ensures stale/oversized caches can be cleaned even if an lxapp is never reopened
- Runtime event-driven cleanup:
  - on cache access, enqueue an event
  - worker performs throttled cleanup asynchronously

## Lifecycle

### Create

- `LxAppCtx::new(...)` constructs a shared `CacheCapacityManager`.
- Same `LxAppCtx` instance is cloned for console/file/network guards.

### Trigger

- `FileAccessGuard::resolve_access(...)`:
  - resolves safe path
  - if path is under `user_cache_dir`, touches atime for existing file
  - enqueues `Access` event

### Worker Behavior

- lazy start on first event
- event queue (`tokio::sync::mpsc`) with coalescing under pressure
- throttled by `min_check_interval`
- blocking file scan/delete runs in `rong::bg::spawn_blocking`

### Shutdown

- On `TerminateAppSvc`, guards are reset, old `LxAppCtx` is dropped.
- `CacheCapacityManager::drop` sends shutdown signal and closes channel.

## Cleanup Policy

Main function: `enforce_cache_limits(cache_dir, max_bytes, max_age)`.

Order:

1. Build candidate list + aggregate `total_bytes`.
2. Age pass: remove files older than `max_age`.
3. Capacity pass: if still over `max_bytes`, evict by LRU (`last_access` ascending).

## Candidate And Accounting Rules

Counted in total usage:

- regular cache files
- protected files (`.lock`, `.part`, `.ok`)

Not directly evicted:

- `*.lock`
- `*.part`
- `*.ok`
- files with active sibling lock (`<stem>.lock`)

Rationale: in-progress and integrity-marker files must not be direct eviction targets, but they still consume storage and must count against usage.

## Safety Invariants

Must always hold:

1. No traversal into symlink directories during recursive scan.
2. Before delete, candidate path canonicalizes under canonicalized cache root.
3. Deletion outside root is skipped and logged.
4. Deleting a data file also removes sibling `.ok`.
5. Empty parent directories may be pruned, but never above cache root.

## Performance Model

Hot path (`resolve_access`) remains lightweight:

- path check + optional atime touch + queue send
- no directory walk
- no file deletion

Heavy I/O is offloaded to background runtime and throttled.

## Observability

Key logs:

- worker spawn failures
- cleanup task failures
- startup cleanup summary (`files_removed`, `bytes_freed`)
- runtime cleanup summary (`files_removed`, `bytes_freed`, limits)
- deletion skipped due to root-boundary validation

## Trade-Offs

- Using atime for LRU can degrade on filesystems where atime is coarse or disabled.
- Protected files may temporarily keep usage above quota.
- Startup sweep is best-effort async, not a blocking guarantee before app open.

## Maintenance Checklist

Before merging cache changes, confirm:

1. In-progress download safety (`.lock` / `.part`) is preserved.
2. Root-boundary deletion guard is preserved.
3. Symlink recursion is still blocked.
4. Startup sweep still exists or is intentionally replaced.
5. `resolve_access` is still non-blocking (no scan/delete).
6. `.ok` marker cleanup still matches storage layout.

## Future Evolution

Possible next steps:

- Optional global quota coordinator across all lxapps.
- Better telemetry counters (per-app cleanup frequency and bytes reclaimed).
- Optional cooldown window for newly completed files before eviction.
