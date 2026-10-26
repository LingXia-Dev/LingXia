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

Each download attempt owns an HTTP abort guard through body consumption;
dropping the future must stop the transport as well as release its caller.
Non-persistent shared-cache subscribers may elect a replacement when their
leader disappears (at most three attempts). Ordinary transfer failures do not
trigger this takeover; the existing retry policy still governs those failures.

## Eviction triggers

Usercache cleanup runs at host startup, on usercache writes, and app-wide when
total storage nears `appStorageMaxSizeMB`. A freshly written file is never
evicted by the write that stored it. Access time is refreshed by `LxFile`
reads, `readDir`, `stat`, `exists`, copy/move from usercache, and WebView
`lx://usercache` loads that reach the scheme handler.

On `ENOSPC`, `lx.fs` writes and `downloadFile` finalization evict LRU usercache
(never userdata) and retry once; a second failure surfaces the IO error.

## HTTP store

Bytes LingXia fetches for itself (remote GPU page images, favicons, registry
icons, preview media) go through `transfer::store`, under the OS cache dir at
`lingxia/http`. Entries are keyed by partition and URL: an appid, or
`host:<name>` for bytes shared across apps on purpose. Callers check their own
grants before a lookup. Freshness follows `Cache-Control`/`Expires`, else 10% of
the age since `Last-Modified` (at most 24 h); stale entries revalidate with
validators, and a 304 serves the stored body. Package downloads (updates,
plugins) bypass the store and resume from a `.part` file instead. The store
evicts LRU by bytes within `STORE_BUDGET_BYTES`; the OS may delete its files at
any time, which reads as a miss.

## Host cache maintenance

`lx.host.cache.clear()` removes unprotected lxapp usercache, idle session temp,
shared runtime artwork, the HTTP store, and WebView HTTP cache where supported.
The startup pass trims the HTTP store to its budget. Package
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

`DownloadBehavior.max_bytes` optionally bounds cached and transferred bytes.
Check the cumulative size before writing each chunk, including resumed data;
never publish an oversized transfer. Include the bound in concurrent-download
behavior matching so a bounded consumer cannot join an unlimited transfer.
