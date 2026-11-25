# Lxapp storage

For contributors changing managed files, quotas, or cache maintenance. The
author-facing contract is [`docs/skill/lxapp/files.md`](../skill/lxapp/files.md).

LingXia manages three file areas per lxapp (userdata, usercache, temp) and two
caches it fills for itself (the HTTP store and View bytecode). Invariants:

- A final file is never partial: downloads and `lx.fs` writes publish by rename.
- Eviction touches only usercache and LingXia's own caches, never userdata.
- A cache file that does not match its key reads as a miss; the OS may delete
  any cache file at any time.
- A clear never restarts an app and never removes a live instance's files.

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

A download in `lingxia-transfer` may carry an optional bound on persisted
bytes. The cumulative size, resumed data included, is checked before each chunk
is written, and an oversized transfer is never published. The bound is part of
concurrent-download matching, so a bounded consumer cannot join an unlimited
transfer.

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
icons, preview media) go through the `lingxia-transfer` HTTP store, under the OS
cache dir at `lingxia/http`. Entries are keyed by partition and URL: an appid, or
`host:<name>` for bytes shared across apps on purpose. Callers check their own
grants before a lookup. Freshness follows `Cache-Control`/`Expires`, else 10% of
the age since `Last-Modified` (at most 24 h); stale entries revalidate with
validators, and a 304 serves the stored body. The store evicts LRU by bytes
within a 256 MB budget.

Package downloads (updates, plugins) bypass the store and resume from a `.part`
file instead: they are large and used once, so caching them would only crowd
out small reusable entries.

## View bytecode

GPU page modules compiled by QuickJS persist under the OS cache dir at
`lingxia/view-bytecode`, one `.qbc` file per key. The key hashes the source, the module specifier, the View crate version,
the QuickJS version and the UI ABI, so any engine change is a miss rather than
a bad load. The header repeats key and length; a renamed or truncated file is
removed on read. A read refreshes the file's mtime, and trimming drops the
oldest files beyond 32 MB, never the one just written. On Windows the bootstrap
names no directory, so bytecode stays in memory for the process.

## Host cache maintenance

`lx.host.cache.clear()` removes unprotected lxapp usercache, idle session temp,
shared runtime artwork, the HTTP store, and WebView HTTP cache where supported.
The startup pass trims the HTTP store to its budget. Package maintenance also
reclaims orphaned installs and staged archives, preserving referenced paths,
active work, recent writes, and paths with uncertain timestamps.

Protection of a live instance begins before storage initialization and lasts
until the instance is released; lingering runtime work may conservatively keep
a closed app protected.

`size()` estimates reclaimable managed file bytes, not total storage.
`freedBytes` sums estimates for removed paths rather than diffing two usage
snapshots; neither includes WebView bytes or promises reclaimed disk blocks,
and partially removed paths are not counted. `skippedActivePaths` counts
protected cache/session directories, not apps. A partial clear resolves with
`failures`; failing to initialize or run the task rejects.
