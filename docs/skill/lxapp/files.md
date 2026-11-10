# Files

LingXia-managed files: storage classes, `downloadFile`, `lx.fs`, uploads,
media outputs, and cleanup. A returned `lx://` path tells you its lifetime.

## Storage classes

| Class | URI | Lifetime |
| --- | --- | --- |
| Temp | `lx://temp/<opaque_id>` | short-lived, session-scoped, auto-cleaned |
| User Data | `lx://userdata/<path>` | durable, never auto-cleaned |
| User Cache | `lx://usercache/<path>` | regenerable, auto-cleaned under capacity pressure |

Use only `lx://` URIs, never native paths. Never keep a business reference to a
temp URI; copy it into userdata first. User-visible downloads belong to the
host downloads center, not to these classes.

## `downloadFile`

The result's `storage` (`temp` | `userdata` | `downloads`) says where it went.

```ts
const result = await lx.downloadFile({ url, headers, timeoutMs, signal }).result;
result.uri; // lx://temp/<opaque_id>
```

```ts
const result = await lx.downloadFile({
  url,
  filePath: "videos/video.mp4",
}).result;
result.uri; // lx://userdata/videos/video.mp4
```

```ts
const task = lx.downloadFile({
  url,
  destination: "downloads",
  suggestedName: "video.mp4",
});
```

- `filePath` is relative or `lx://userdata/...`. Rejected: `lx://usercache`,
  native absolute or drive paths, backslashes, empty, `.` or `..` segments, and
  the userdata root.
- `destination: "downloads"` saves to the user's Downloads (shown in the
  downloads page) and needs the `downloads` host grant **and** a native session
  grant ([permissions](../native/permissions.md)); without it the call fails.
  `suggestedName` is sanitized and never overwrites. A `downloads` URI is
  opaque to `lx.fs`.
- A failed or canceled download leaves no partial file; `resume()` continues a
  paused one.

## `lx.fs`

`lx.fs.file(path)` is a lazy reference; pick the representation with a method,
as with Web `Blob`:

```ts
const file = lx.fs.file("notes.json");
const text = await file.text();            // string, strict UTF-8
const value = await file.json();           // unknown; validate before use
const bytes = await file.bytes();           // Uint8Array
const buffer = await file.arrayBuffer();    // ArrayBuffer
const encoded = await file.base64();        // string
```

- Relative paths resolve under userdata; `lx.env.USER_DATA_PATH` and
  `lx.env.USER_CACHE_PATH` are the explicit roots. Reads also accept
  `lx://temp/...`.
- Each read is limited to 16 MiB. Pass larger files by path to upload,
  preview, or native APIs.
- `write(path, data, options?)` takes a string, `ArrayBuffer`, or typed array;
  `encoding: "base64"` decodes a Base64 input string.
- `readDir(path)` resolves to entries with `name`, `isFile`, `isDirectory`,
  `isSymlink`.

### Copy and move

```ts
await lx.fs.copy(
  result.uri,
  "media/video.mp4",                    // relative → lx://userdata/media/video.mp4
);

await lx.fs.rename(
  result.uri,
  `${lx.env.USER_CACHE_PATH}/previews/video.mp4`,
);
```

- Sources: temp, userdata, usercache. Destinations: userdata or usercache.
- Parent directories are created. `write`, `copy`, and `rename` never
  overwrite unless `overwrite: true`, and never replace a directory.
- `rename` moves: a temp download renamed into usercache becomes cache without
  a second copy.

## `uploadFile`

Call from Logic; the file streams without entering JavaScript memory.

- Form endpoint: default `bodyMode: 'multipart'`, method `POST`; `name`,
  `fileName`, `formData` build the envelope.
- Raw or presigned URL: `bodyMode: 'raw'` with the signed method, headers, and
  MIME type, and no form fields.

```ts
const task = lx.uploadFile({
  url: presignedUrl,
  filePath,
  method: 'PUT',
  bodyMode: 'raw',
  mimeType: contentType,
});
for await (const event of task.progress) {
  if (event.kind === 'progress') updateProgress(event.progress ?? 0);
}
const { statusCode, data } = await task.result;
```

- `task.result` settles once; `task.progress` has one consumer. Attach a
  rejection handler to `result` when consuming progress separately.
- `cancel()`, `signal`, and `timeoutMs` are available. Keep the source file
  until the transfer ends.
- Check `statusCode` and `data` against the endpoint before reporting success.

## Pickers

`pickFile`, `pickFiles`, and `pickDirectory` open the system picker and resolve
`{ status: 'ok', uri, name }` (`pickFiles`: `files: [{ uri, name }]`) or
`{ status: 'canceled' }`.

- `uri` is opaque; never parse it. Pass it to the APIs that take a picked
  file (`lx.share`, the clipboard image write); a platform reference such as
  an Android `content://` URI is not a managed path, so other file APIs may
  reject it.
- `name` is display-only: not unique, and it may lack an extension or disagree
  with the content type. Do not use it as a path or key.
- `filters[].name` labels the filter on desktop only.
- A picked `uri` is read-only and valid for the current session only. To keep
  the file, `lx.fs.copy` it into userdata or usercache.
- A `pickDirectory` `uri` is a read-only root: `readDir` it, then read a child
  as `` `${uri}/${relative}` `` (percent-encode each segment) with `exists`,
  `stat`, `readDir`, `file()`, or as a `copy` source. Empty, `.`, `..`
  segments, `:`, `\`, and paths leaving the directory through a symlink do
  not resolve; `lx.fs.file` throws on a missing child, so check `exists`
  first. `copy` takes one file; it is not recursive.
- On Windows, a failed system dialog may resolve as `canceled`, and `pickFiles`
  rejects with `E_NOT_FOUND` if the user types a name that does not exist.
- Whether the file is copied differs by host, so do not assume either:

| | Mobile | Desktop |
| --- | --- | --- |
| `pickFile` / `pickFiles` | iOS: the system hands over a copy. Android, Harmony: no copy, a session-scoped reference to the original | No copy; the live original, so later edits or deletion show |
| `pickMedia` | Copied into the lxapp temp directory and counted against its temp quota | No copy; the live original |
| `pickDirectory` | iOS: no copy, a session-scoped grant on the directory. Android, Harmony: see below | No copy; a session-scoped grant on the directory |

`pickDirectory` on Android and on HarmonyOS phones and tablets has no usable
system folder picker. Pass a `defaultPath` directory in lxapp storage (for
example `lx://userdata/`) to get the in-app chooser; without one the call
rejects with `E_NOT_SUPPORTED`, so catch it and fall back to `pickFile(s)`.

## Media outputs

`pickMedia`, `compressImage`, `compressVideo`, and video thumbnails return
temp files. `lx.fs.copy` keeps a copy; `lx.fs.rename` moves it into userdata or
usercache.

## Cleanup and quotas

Limits come from host [`storage`](../app/project.md#storage):

| Setting | Default | Scope | `0` Means |
| --- | ---: | --- | --- |
| `tempMaxSizeMB` | 1024 | per LxApp runtime session | disable temp size limit |
| `cacheMaxSizeMB` | 2048 | per LxApp usercache | disable usercache size enforcement |
| `dataMaxSizeMB` | 4096 | per LxApp userdata | disable userdata size limit |
| `appStorageMaxSizeMB` | 16384 | total userdata + usercache budget | disable app-wide storage limit |

- **Temp** is removed with its session (stale sessions at open, the current one
  at destroy) and oldest-first under the cap; the OS may also clear it.
  Overflow fails with `TEMP_QUOTA_EXCEEDED`.
- **Usercache** is LRU-evicted from 80 % down to 50 % of the cap, with no age
  cutoff. Assets a WebView keeps in its own cache stop refreshing their access
  time; refresh them with `lx.fs.stat(path)` at session start, or keep them in
  userdata. Overflow fails with `USERCACHE_QUOTA_EXCEEDED`.
- **Userdata** is never evicted; only explicit deletes, uninstall, or the user
  clearing app data remove it. Overflow fails with `USERDATA_QUOTA_EXCEEDED`,
  or `APP_STORAGE_QUOTA_EXCEEDED` after usercache cleanup.
- On a full disk, writes evict usercache and retry once, then fail; tell the
  user.

## Host cache

`lx.host.cache` exists only in the [Control app](../app/control-app.md), for a
product-wide "clear cache" setting:

```ts
const cache = lx.host.cache;
if (!cache) return; // not the Control app
const reclaimableBytes = await cache.size();
const result = await cache.clear();
// result: { freedBytes, skippedActivePaths, webview, failures }
```

- Clears usercache, idle temp, and the WebView HTTP cache; never userdata, KV
  storage, Downloads, cookies, or installs.
- Running lxapps (including the caller) keep their cache; to clear one, use its
  host "clear cache and restart" action.
- Sizes are estimates. `webview` is `cleared`, `unsupported`, or `failed`; show
  skipped and failed work, and never promise everything was cleared.
