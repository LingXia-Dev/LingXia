# LingXia File Lifecycle

What an lxapp author needs to know about LingXia-managed files: which storage
class a returned path belongs to, how `downloadFile`, `getFileManager()`, and
the media APIs place files, and when the runtime may clean them up. A returned
path tells you whether the file is temporary, cache-managed, or durable.

## Storage Classes

LingXia exposes three LxApp-owned storage classes:

| Class | URI | Lifetime |
| --- | --- | --- |
| Temp | `lx://temp/<opaque_id>` | short-lived, session-scoped, auto-cleaned |
| User Data | `lx://userdata/<path>` | durable, never auto-cleaned |
| User Cache | `lx://usercache/<path>` | regenerable, auto-cleaned under capacity pressure |

User-visible downloads are exposed through
`downloadFile({ destination: "downloads" })`. They are owned by the host
downloads center, not by LxApp private storage.

LxApp code must only use `lx://` URIs — never native paths. Physically, temp
lives under the OS app-cache directory (disposable), while userdata *and*
usercache live under app data: usercache deliberately so, because LingXia owns
its cleanup policy rather than the OS. The exact on-disk layout is internal and
may change between releases.

## API Semantics

### `downloadFile`

`downloadFile` defaults to app-owned output. Final output depends on
`destination` and `filePath`.

Without `filePath`, the result is temp:

```ts
const result = await lx.downloadFile({ url, headers, timeout, signal });
result.tempFilePath; // lx://temp/<opaque_id>
```

With `filePath`, the destination must be relative or `lx://userdata/...`:

```ts
const result = await lx.downloadFile({
  url,
  filePath: "videos/video.mp4",
});
result.filePath; // lx://userdata/videos/video.mp4
```

With `destination: "downloads"`, the file is saved into the user's Downloads
directory and appears in the built-in downloads page. `filePath` is treated as
a filename or relative-name hint only; the runtime sanitizes it, prevents
directory traversal, and avoids overwriting existing files.

```ts
const task = lx.downloadFile({
  url,
  destination: "downloads",
  filePath: "video.mp4",
});
```

This requires `"downloads"` in `lxapp.json` `security.privileges`. App-owned
output (`destination: "app"`, the default) does not require that privilege.

Rejected destinations:

- `lx://usercache/...`
- native absolute paths
- drive-style paths containing `:`
- backslash paths
- empty path segments
- `.` or `..` segments
- the `lx://userdata` root itself

Downloads stage in a private location and move into place on success, so a
failed or canceled download never leaves a partial final file; pausing keeps
the staging so `resume()` can continue, and identical URLs can download
concurrently.

### `getFileManager`

`getFileManager` returns the LingXia-managed file manager.

```ts
const fs = lx.getFileManager();
```

Relative paths resolve under userdata. `lx.env.USER_DATA_PATH` and
`lx.env.USER_CACHE_PATH` provide the explicit `lx://userdata` and
`lx://usercache` roots. Read methods also accept `lx://temp/...`.

### File Copy And Move

```ts
const fs = lx.getFileManager();
await fs.copyFile({
  srcPath: result.tempFilePath,
  destPath: "media/video.mp4",          // relative → lx://userdata/media/video.mp4
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

`writeFile`, `copyFile`, and `rename` are explicit file management APIs. They
default to no overwrite and support `overwrite: true` only when requested.
Overwrite applies to files only; directories are never replaced by file writes.

`rename` is move semantics. Moving a temp download into usercache avoids a
second durable copy and hands the file to cache cleanup.

`readDir` resolves to an async iterator of directory entries with `name`,
`isFile`, `isDirectory`, and `isSymlink`.

### Media APIs

`chooseMedia`, `compressImage`, `compressVideo`, and video thumbnail APIs
return temp outputs by default. Use `copyFile` to keep a copy, or `rename` to
move it into userdata or usercache.

## `lingxia.yaml` Storage Configuration

Host apps configure storage limits in `lingxia.yaml`:

```yaml
storage:
  tempMaxSizeMB: 1024
  cacheMaxSizeMB: 2048
  dataMaxSizeMB: 4096
  appStorageMaxSizeMB: 16384
```

| Setting | Default | Scope | `0` Means |
| --- | ---: | --- | --- |
| `tempMaxSizeMB` | 1024 | per LxApp runtime session | disable temp size limit |
| `cacheMaxSizeMB` | 2048 | per LxApp usercache | disable usercache size enforcement |
| `dataMaxSizeMB` | 4096 | per LxApp userdata | disable userdata size limit |
| `appStorageMaxSizeMB` | 16384 | total userdata + usercache budget | disable app-wide storage limit |

## When files get cleaned up

**Temp** is session-scoped: stale sessions are removed when the lxapp opens,
and the current session's temp is removed on lxapp destroy. Size cleanup
(oldest-first) runs as temp files are produced; if it can't free enough space
under `tempMaxSizeMB`, the operation fails with `TEMP_QUOTA_EXCEEDED`. The OS
may also clear app cache at any time.

**User cache** eviction is capacity-driven LRU — there is **no age cutoff**
(files are never deleted just for being old). Cleanup triggers when a cache
would reach 80% of `cacheMaxSizeMB` and evicts least-recently-used files down
to 50%, so writes don't thrash the cleaner. It runs at host startup, on
usercache writes, and app-wide when total storage nears
`appStorageMaxSizeMB`. A freshly written file is never evicted by the write
that stored it. If cleanup can't make room, the write fails with
`USERCACHE_QUOTA_EXCEEDED`.

What counts as "recently used": FileManager reads (`readFile`, `readDir`,
`stat`, `exists`, copy/move *from* usercache) and WebView `lx://usercache`
resource loads refresh a file's access time. **Gotcha:** a WebView that keeps
an asset in its internal resource cache never re-hits the scheme handler, so
that asset's access time goes stale and it becomes the first LRU candidate
under pressure. If a long-lived asset must survive, put it in userdata — or
refresh it explicitly with `fs.stat(path)` / `fs.exists(path)` at session
start.

**User data** is never auto-cleaned to satisfy quota. It is deleted only by
explicit delete APIs, lxapp uninstall, or the user clearing app data. Writes
that would exceed `dataMaxSizeMB` fail with `USERDATA_QUOTA_EXCEEDED`; writes
that would exceed `appStorageMaxSizeMB` first trigger usercache cleanup, then
fail with `APP_STORAGE_QUOTA_EXCEEDED`. Quota failures are ordinary errors to
handle in app code — existing data is never silently deleted to make a write
succeed.

**Physical disk full**: quotas are logical caps — the device can hit `ENOSPC`
first. FileManager writes and `downloadFile` finalization then evict LRU
usercache (never userdata) and retry once; if the retry still fails, the IO
error surfaces to the caller and the lxapp should tell the user.

## Storage Summary

```text
tempFilePath  -> lx://temp/<opaque_id>
                 short-lived, session/size scoped

filePath      -> lx://userdata/<path>
                 durable owner-private data

usercache     -> lx://usercache/<path>
                 regenerable cache, LRU-evicted under capacity pressure
```

## Rules for Developers

- Use temp files for immediate preview, upload, transform, or save flows.
- Use `fs.writeFile({ filePath: lx.env.USER_CACHE_PATH + "/..." })` for developer-generated regenerable files.
- Use `fs.copyFile` when a temp file must be copied into userdata or usercache.
- Use `fs.rename({ oldPath: tempFilePath, newPath: "lx://usercache/..." })` when a temp file should become auto-cleaned cache without a second copy.
- Use `downloadFile({ filePath })` only for durable userdata destinations.
- Do not pass `lx://usercache`, host download directories, or native paths to `downloadFile.filePath`.
- Do not store business-critical references to `tempFilePath`.
