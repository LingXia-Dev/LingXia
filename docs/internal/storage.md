# Lxapp storage

For contributors changing managed files, quotas, or cache maintenance. The
author-facing contract is [`docs/skill/lxapp/files.md`](../skill/lxapp/files.md).

## Physical layout

Temp lives under the OS app-cache directory (disposable). Userdata and
usercache both live under app data: usercache deliberately so, because LingXia
owns its cleanup policy rather than the OS. The on-disk layout is internal and
may change between releases.

## Atomic writes

Downloads stage in a private location and move into place on success, so a
failed or canceled download never leaves a partial final file; pausing keeps
the staging for `resume()`, and identical URLs may download concurrently.
`lx.fs` final writes use a sibling temp file and rename/replace.

## Eviction triggers

Usercache cleanup runs at host startup, on usercache writes, and app-wide when
total storage nears `appStorageMaxSizeMB`. A freshly written file is never
evicted by the write that stored it. Access time is refreshed by `LxFile`
reads, `readDir`, `stat`, `exists`, copy/move from usercache, and WebView
`lx://usercache` loads that reach the scheme handler.

On `ENOSPC`, `lx.fs` writes and `downloadFile` finalization evict LRU usercache
(never userdata) and retry once; a second failure surfaces the IO error.

## Host cache maintenance

`lx.host.cache.clear()` removes unprotected lxapp usercache, idle session temp,
shared runtime artwork, and WebView HTTP cache where supported. Package
maintenance also reclaims orphaned installs and staged archives, preserving
referenced paths, active work, recent writes, and paths with uncertain
timestamps.

Protection of a live instance begins before storage initialization and lasts
until the instance is released; lingering runtime work may conservatively keep
a closed app protected. No app is restarted.

`size()` estimates reclaimable managed file bytes, not total storage.
`freedBytes` sums estimates for removed paths rather than diffing two usage
snapshots; neither includes WebView bytes or promises reclaimed disk blocks,
and partially removed paths are not counted. `skippedActivePaths` counts
protected cache/session directories, not apps. A partial clear resolves with
`failures`; failing to initialize or run the task rejects.
