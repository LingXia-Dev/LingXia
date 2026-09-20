# JavaScript API migration

These changes require the matching LingXia runtime and `@lingxia/types` package.

| Previous API | Replacement |
| --- | --- |
| `lx.app` | `lx.host` (host application; `App()` still defines the current lxapp) |
| `await task`, `task.wait()`, `task.then(...)` | `await task.result`, `task.result.then(...)` |
| `for await (... of task)`, `task.next()` | `task.progress`, with a single consumer |
| `DownloadTask.abort()` | `await task.cancel()` |
| `canceled: boolean`, clipboard `empty: boolean` | `status: 'ok' \| 'canceled'` (clipboard also `'empty'`) |
| Download `tempFilePath` / `filePath`, `size` | `uri`, `storage: 'temp' \| 'userdata' \| 'downloads'`, `sizeBytes` |
| Media and screenshot `tempFilePath` | `uri` |
| Downloads destination `filePath` | `suggestedName`; app destinations retain `filePath` |
| `lx.surface.get(keyOrId)` | `lx.surface.getByKey(key)`; only explicit caller keys are searchable |
| `lx.surface.onContext` | `lx.surface.watchContext` (initial snapshot and changes) |
| Action-sheet `itemList`, result `index` | `items: [{ id, label }]`, result `id` |
| Transfer `timeout`, toast `duration` | `timeoutMs`, `durationMs` |
| Location `type`, `highAccuracyExpireTime` | `coordinateSystem`, `timeoutMs` |
| Media picker `maxDuration` | `maxDurationSeconds` |
| Stream video `duration` | `durationSeconds` |
| `SDKVersion`, Wi-Fi `SSID` / `BSSID` | `sdkVersion`, `ssid` / `bssid` |
| `page.setData(updates, callback)` | `page.setData(updates); await page.flush()` |

Tasks are ordinary handles, not thenables. Attach error handling to `result`
when observing progress separately. Ending progress iteration only detaches the
observer; it does not cancel the operation. `cancel()` requests cancellation;
`result` determines the terminal outcome. Download tasks additionally support
`pause()` and `resume()`. Host update tasks do not expose cancellation.

```ts
const task = lx.downloadFile({ url, signal });
const observe = (async () => {
  for await (const event of task.progress) renderProgress(event);
})();
const [file] = await Promise.all([task.result, observe]);
if (file.storage === 'temp') await lx.fs.copy(file.uri, 'lx://userdata/saved.bin');
```

Treat file references as opaque. System Downloads references cannot be passed to
managed `lx.fs` methods; the types reject that branded result. Temporary files
remain temporary even though their field is now `uri`.

`preview.presented` resolves `{ status: 'presented' }` only after presentation,
or `{ status: 'notPresented', reason }` when the session ends first. Await
`preview.completed` for the final playback/dismissal result.

Only owned tab handles expose `activate`, `close`, and `onClose`. Narrow
`scope === 'tab'` before using those members; browser-group and builtin handles
do not advertise lifecycle operations they cannot perform.

Use `lx.alert()` for acknowledgement and `lx.confirm()` for a boolean decision.
`lx.pickFile()` returns one `uri`; `lx.pickFiles()` returns nonempty `paths` on
success. The lower-level `showModal()` and `chooseFile()` remain available.
`showToast()` resolves a handle after presentation is accepted; `dismiss()`
does not dismiss a newer toast. `hideToast()` remains an unconditional hide.

Page data must contain finite JSON values, plain objects and arrays; `undefined`
removes a field. Functions, class instances and cycles are rejected. Runtime
members such as `flush`, `setData` and `surface` are reserved. `flush()` includes
prior in-flight batches and waits for the attached View's acknowledgement; it
is not a browser paint barrier. Unloading rejects outstanding flushes.

`storage.get(key, decode)` validates or migrates an existing value at the read
boundary; missing keys return `undefined` without invoking the decoder.

Automation evaluation with `captureCalls: true` returns `{ value, calls }`.
Browser click/press with `waitNavigation: true` returns navigation details;
otherwise it returns `null`. Desktop selectors require exactly one target.
Automation keeps its host wire DTO spelling rather than silently translating
fields. Synchronous process options reject asynchronous callbacks and signals.

Native `LxFile` and `DirEntry` are types, not runtime imports. Page interfaces,
nullable nested paths, tuple positions, binary-write options and stream
controllers now retain their intended TypeScript constraints.
