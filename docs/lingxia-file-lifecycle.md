# LingXia File Lifecycle

This document defines LingXia-managed file lifetimes, storage locations, cleanup triggers, quota behavior, and the relationship between APIs such as `downloadFile`, `getFileManager()`, `chooseMedia`, `compressImage`, and `compressVideo`.

The design goal is simple: a returned path should tell developers whether the file is temporary, cache-managed, or durable.

## Storage Classes

LingXia exposes three LxApp-owned storage classes:

| Class | URI | Physical Owner | Lifetime |
| --- | --- | --- | --- |
| Temp | `lx://temp/<opaque_id>` | current LxApp runtime session | short-lived, auto-cleaned |
| User Data | `lx://userdata/<path>` | one LxApp | durable, never auto-cleaned |
| User Cache | `lx://usercache/<path>` | one LxApp | regenerable, auto-cleaned |

Shell or desktop-visible downloads are host product behavior and are not exposed through `downloadFile.filePath`. A future Shell-managed download API should own progress records, permissions, and user-visible cleanup separately.

## Physical Layout

LingXia identifies each LxApp storage owner by its fingermark.

```text
<app_data>/lingxia/lxapps/<lxapp_fingermark>/
  installed LxApp bundle

<app_data>/lingxia/userdata/<lxapp_fingermark>/
  durable LxApp files

<app_data>/lingxia/usercache/<lxapp_fingermark>/
  LingXia-managed regenerable cache

<app_data>/lingxia/storage/<lxapp_fingermark>.redb
  LxApp key-value storage

<app_cache>/lingxia/lxapps/temp/<lxapp_fingermark>/<session_id>/
  current runtime temp files
```

`usercache` intentionally lives under app data, not OS cache, because LingXia owns its cleanup policy. Temp lives under app cache because it is session-scoped and disposable.

## API Semantics

### `downloadFile`

`downloadFile` always stages internally first. Final output depends on `filePath`.

Without `filePath`, the result is temp:

```ts
const result = await lx.downloadFile({ url, headers, timeout, signal });
result.tempFilePath; // lx://temp/<opaque_id>
```

With `filePath`, the destination must be relative or `lx://userdata/...`:

```ts
const result = await lx.downloadFile({
  url,
  filePath: "downloads/video.mp4",
});
result.filePath; // lx://userdata/downloads/video.mp4
```

Rejected destinations:

- `lx://usercache/...`
- native absolute paths
- host download directories
- drive-style paths containing `:`
- backslash paths
- empty path segments
- `.` or `..` segments
- the `lx://userdata` root itself

### `getFileManager`

`getFileManager` returns the LingXia-managed file manager.

```ts
const fs = lx.getFileManager();
```

Relative paths resolve under userdata. `lx.env.USER_DATA_PATH` and `lx.env.USER_CACHE_PATH` provide the explicit `lx://userdata` and `lx://usercache` roots. Read methods also accept `lx://temp/...`.

### File Copy And Move

```ts
const fs = lx.getFileManager();
await fs.copyFile({
  srcPath: result.tempFilePath,
  destPath: "downloads/video.mp4",
});

await fs.rename({
  oldPath: result.tempFilePath,
  newPath: `${lx.env.USER_CACHE_PATH}/previews/video.mp4`,
});
```

Rules:

- `copyFile` copies from temp, userdata, or usercache into userdata or usercache
- `rename` moves from temp, userdata, or usercache into userdata or usercache
- relative destinations resolve under userdata
- explicit `lx://` destinations may target `lx://userdata` or `lx://usercache`
- parent directories are created automatically
- existing destination files are not overwritten
- final writes use a sibling temp file and rename/replace, so failed writes do not leave final partial files

### FileManager writes

`writeFile`, `copyFile`, and `rename` are explicit file management APIs. They default to no overwrite and support `overwrite: true` only when requested. Overwrite applies to files only; directories are never replaced by file writes.

`rename` is move semantics. It may move from temp, userdata, or usercache into userdata or usercache. Moving a temp download into usercache avoids a second durable copy and hands the file to cache cleanup.

`readDir` resolves to an async iterator of directory entries with `name`, `isFile`, `isDirectory`, and `isSymlink`, matching the Rong fs shape while keeping LingXia path lifecycle rules.

Userdata writes run userdata and appStorage quota checks. Usercache writes run usercache cleanup/quota checks and then appStorage checks.

### Media APIs

`chooseMedia`, `compressImage`, `compressVideo`, and video thumbnail APIs return temp outputs by default. Use `copyFile` to keep a copy, or `rename` to move it into userdata or usercache.

## `lingxia.yaml` Storage Configuration

`lingxia.yaml` configures storage limits. This section is only configuration; cleanup behavior is described in the next section.

```yaml
storage:
  tempMaxSizeMB: 1024
  cacheMaxAgeDays: 7
  cacheMaxSizeMB: 2048
  dataMaxSizeMB: 4096
  appStorageMaxSizeMB: 16384
```

| Setting | Default | Scope | Meaning | `0` Means |
| --- | ---: | --- | --- | --- |
| `tempMaxSizeMB` | 1024 | per LxApp runtime session | max size for returned temp files | disable temp size limit |
| `cacheMaxAgeDays` | 7 | host-wide policy applied to every LxApp usercache | max age by access metadata | disable age cleanup |
| `cacheMaxSizeMB` | 2048 | per LxApp usercache | max size for one LxApp cache directory | disable per-LxApp cache size limit |
| `dataMaxSizeMB` | 4096 | per LxApp userdata | max durable files for one LxApp | disable userdata size limit |
| `appStorageMaxSizeMB` | 16384 | whole LingXia-managed app storage | total userdata + usercache budget | disable app-wide storage limit |

`cacheMaxAgeDays` is a host-wide policy value, not a per-LxApp config. LingXia applies the same retention rule to every LxApp usercache directory.

`cacheMaxSizeMB` is per LxApp. Normal per-LxApp cleanup should not evict another LxApp's cache. Cross-LxApp cleanup happens only for app-wide storage pressure.

## Cleanup Policy

Cleanup is triggered by runtime events, not by developers calling arbitrary cleanup code.

| Trigger | Scope | May Delete | Never Deletes |
| --- | --- | --- | --- |
| LxApp startup/open | stale temp sessions for that LxApp | old temp session dirs | current active temp session |
| Temp output registration/finalization | current runtime temp session | old unpinned temp files | active `.download-staging`, current keep file |
| LxApp destroy | current runtime temp session | current temp session dir | userdata |
| App startup maintenance | all LxApp usercache dirs | expired/LRU usercache | userdata, temp staging |
| Usercache access/write | current LxApp usercache | expired/LRU files in that LxApp cache | other LxApp caches in normal per-LxApp cleanup |
| `downloadFile({ filePath })` / FileManager managed writes under appStorage pressure | all LxApp usercache dirs | usercache across LxApps | userdata |
| LxApp uninstall | that LxApp storage | its userdata, usercache, KV storage, bundle | other LxApps |

Global invariants:

- LingXia may delete temp and usercache automatically.
- LingXia must not delete userdata automatically to satisfy quota.
- Quota failures are business errors, not internal runtime errors.
- Failed writes must not leave final partial files.

## Temp Policy

Temp is session-scoped. Returned temp URIs are opaque and should not reveal filesystem layout.

Cleanup:

- stale sessions are removed when the LxApp opens
- current session temp is removed on runtime destroy best-effort
- size cleanup runs when temp files are returned or finalized
- OS may also clear app cache

Quota:

- each active runtime session uses `tempMaxSizeMB`
- size cleanup deletes oldest unpinned files first
- if cleanup cannot free enough space, LingXia deletes the current output and returns `TEMP_QUOTA_EXCEEDED`

## User Cache Policy

Usercache is for regenerable data only. LxApps may explicitly place files there through FileManager when the file can be downloaded or generated again.

Cleanup modes:

- app-wide maintenance cleanup scans `<app_data>/lingxia/usercache/*`
- per-LxApp opportunistic cleanup runs when that LxApp accesses or writes usercache
- appStorage pressure cleanup may delete usercache across LxApps

Deletion order:

1. delete files older than `cacheMaxAgeDays`
2. if still over `cacheMaxSizeMB`, delete least-recently-used files by access metadata
3. under appStorage pressure, continue deleting LRU usercache across LxApps until app storage fits or no cache files remain

Protection rules:

- do not recurse into symlink directories
- skip `.lock`, `.part`, `.ok`
- skip data files with an active sibling `.lock`
- when deleting a data file, delete its `.ok` marker and remove empty parent directories

If cleanup cannot make room for a cache write, return `USERCACHE_QUOTA_EXCEEDED`.

## User Data Policy

Userdata is durable owner-private data.

Cleanup:

- explicit delete APIs
- LxApp uninstall
- app data clearing
- host-admin reset tools

LingXia must not apply age-based or LRU cleanup to userdata.

Write checks:

- `dataMaxSizeMB` applies to one LxApp userdata directory
- `appStorageMaxSizeMB` applies to all LingXia-managed userdata + usercache
- appStorage pressure may clean usercache before rejecting the write

Failure behavior:

- exceeding `dataMaxSizeMB` returns `USERDATA_QUOTA_EXCEEDED`
- exceeding `appStorageMaxSizeMB` after usercache cleanup returns `APP_STORAGE_QUOTA_EXCEEDED`
- existing userdata is not silently deleted

## Download Staging

`downloadFile` writes to staging before finalization.

Current physical staging location:

```text
<app_cache>/lingxia/lxapps/temp/<lxapp_fingermark>/<session_id>/.download-staging/<task_id>
```

Behavior:

- default temp downloads use a unique staging id per call, so identical URLs can download concurrently
- `filePath` downloads reserve the userdata destination while running or paused
- pause keeps staging so resume can continue
- cancel deletes staging
- failure deletes staging when possible
- success moves staging to temp or userdata final location

## Storage Summary

```text
tempFilePath  -> lx://temp/<opaque_id>
                 short-lived, session/size scoped, physically under app cache

filePath      -> lx://userdata/<path>
                 durable owner-private data, physically under app data

usercache     -> lx://usercache/<path>
                 regenerable cache, physically under app data and owned by LingXia cleanup
```

## Rules for Developers

- Use temp files for immediate preview, upload, transform, or save flows.
- Use `fs.writeFile({ filePath: lx.env.USER_CACHE_PATH + "/..." })` for developer-generated regenerable files.
- Use `fs.copyFile` when a temp file must be copied into userdata or usercache.
- Use `fs.rename({ oldPath: tempFilePath, newPath: "lx://usercache/..." })` when a temp file should become auto-cleaned cache without a second copy.
- Use `downloadFile({ filePath })` only for durable userdata destinations.
- Do not pass `lx://usercache`, host download directories, or native paths to `downloadFile.filePath`.
- Do not store business-critical references to `tempFilePath`.

## Rules for LingXia Internals

- Do not return `lx://usercache` as `tempFilePath`.
- Do not store default downloads in usercache.
- Keep temp URI values opaque.
- Keep usercache cleanup inside `lingxia-lxapp` cache management.
- Keep userdata outside automatic cleanup.
