# Automation coverage backlog

Living todo for making showcase `lx.*` contracts **complete**, so product
changes can ship without guessing what broke. Implement in waves. After each
wave, run another review and update this file (checkboxes + Review log). Do
not mark an API done because a `SHAPE-*` case or a ledger row exists.

Bar: [README.md](./README.md) §"What every public capability needs"
(shape → semantic → failure → boundary → lifecycle). `external-ui` is only
for a real OS dialog (picker, share sheet, permission sheet, dialer). In-app
chrome and feedback (toast, modal, action sheet, appearance, navigationBar,
tabBar) must be automated.

Inventory date: 2026-08-18 (review 1).

---

## How to use this file

1. Pick the next unchecked item in the current wave.
2. One `contract()` per primary behavior. `covers` lists the public members.
   Assert observable results and stable error `code`s, not “did not throw”.
3. Drive showcase UI via `data-testid` when the page already exposes the
   action. Do not `eval` past Logic just to make the case shorter.
4. After the wave lands: new review, tick items that actually meet the bar,
   add anything the review found, bump the Review log.

A case is complete only when every required level in
`tests/support/capability-ledger.ts` is asserted in that case (or a named
sibling), not merely listed in `levels:`.

---

## Review log

| Round | Date | Scope | Verdict |
|---|---|---|---|
| 1 | 2026-08-18 | Full `LX_API_NAMES` vs showcase Logic vs `contract()` bodies | Not complete. Only `lx.supports` and `lx.tabBar.update` approach the bar. Ledger over-claims. Showcase chrome/feedback/transfer mostly untested. |
| 2 | | After wave 1 | |
| 3 | | After wave 2 | |
| 4 | | After wave 3 + ledger honesty | |

---

## Snapshot (review 1)

~67 top-level `lx.*` names. Every name has a generated `SHAPE-*` typeof
check in `api/surface.test.ts`. Behavior beyond that:

| Depth | Count | Examples |
|---|---|---|
| Near-complete | 2 | `supports`, `tabBar.update` |
| Semantic, weak failure/events | ~15 | `navigate*`, `fs`, `getStorage`, device/info getters, desktop surface/shell |
| Perfunctory | ~10 | `onKey*`, `onNetworkChange`, `onWifiConnected`, `createVideoContext`, `getLocation` |
| Shape only, **showcase uses it** | ~20 | `appearance`, `navigationBar`, toast/modal/actionSheet, `downloadFile`, `share`, `navigateToApp`, `setMoreActions`, … |
| Shape only, unused or OS dialog | rest | `tray`, `uploadFile`, `chooseDirectory`, wifi module, … |

Views do not call `lx.*` (only `useLxPage` / copy). All product usage is
Logic: `lxapp.ts` + `pages/*/index.ts`.

---

## Wave 1 — in-app chrome and feedback

Logic-only, no device lab. Showcase already has the UI.

### 1.1 `lx.navigationBar.update`

Showcase: `pages/ui`, `pages/cloud`, `pages/media`.

Patch: `title`, `homeButton`, `style.{backgroundColor,foregroundColor,dividerColor}`, `style: null`.

- [ ] Semantic: title + three colors land (read host chrome / `app.info()`, not just `await`)
- [ ] `homeButton` show/hide
- [ ] `style: null` / `title: null` reset, asserted
- [ ] Failure: invalid color — stable `code`, chrome unchanged
- [ ] Boundary: drive the ui page controls, not only `eval`
- [ ] Lifecycle: survives `reLaunch` / page re-enter as specified

### 1.2 `lx.appearance.get` / `set`

Showcase: `pages/home`, `pages/ui` (`data-testid="ui-appearance-*"`).

- [ ] `auto | light | dark` round-trip via the ui buttons
- [ ] `get()` after `set`: `preference` and `resolved` relationship asserted
- [ ] Failure: invalid preference — stable `code`, previous state kept
- [ ] Lifecycle: unsubscribe / re-get after relaunch if the host persists it

### 1.3 Finish `lx.tabBar.update` (`UI-TABBAR-001`)

Already: visibility auto/visible/hidden, style colors, item 1 text/icon/badge,
index 99 does not mutate, badge→redDot, home `selected_index === 0`.

Still open:

- [ ] Failure: assert stable `code` (not `catch { rejected = true }`) for index 99, negative index, missing icon, bad color
- [ ] Boundary: click the native tab after a text/badge change; assert selected item
- [ ] Drive showcase buttons (redDot / badge / item text / style). Add `data-testid` where missing
- [ ] `index: 0` (selected tab) and a multi-item patch
- [ ] `style: null` and field `null` reset **asserted**, not only in `defer`
- [ ] Concurrent `update`s: last-write or documented merge; no torn state
- [ ] Drop the fixed `sleep(500)` in `pages/ui.test.ts` — wait on a real signal

### 1.4 In-app feedback (not OS UI)

- [ ] `lx.showToast` + `lx.hideToast`: show, hide before timeout, duration expiry if observable
- [ ] `lx.showModal`: confirm vs `canceled: true`; `showCancel: false`
- [ ] `lx.showActionSheet`: pick an item; cancel; mixed-language labels if showcase has them
- [ ] Failure: empty title / no items — stable `code`

---

## Wave 2 — showcase main paths still planned or unregistered

### 2.1 Navigation, for real

- [ ] Call **`lx.reLaunch`** from Logic. Do not count `NavDriver.relaunch` as coverage
- [ ] `lx.navigateTo` failure: unknown page, `duplicate_route`, stack-full (ui page already toasts this)
- [ ] `lx.navigateTo` query + `PageMessagePort` if that is a public contract
- [ ] `lx.navigateBack` `delta`, fail at root (stable `code`)
- [ ] `lx.redirectTo` same-route `onLoad`; invalid url
- [ ] `lx.switchTab` to a non-tab page rejects; selected tab + `tabBar.selected_index` agree
- [ ] `lx.navigateToApp` (API page → chat): success or documented reject `code`

### 2.2 Transfer and files the file page already implements

- [ ] `lx.downloadFile` start + progress + completed path
- [ ] `pause` / `resume` / `cancel` on the task
- [ ] Failure: bad URL / denied destination — `code`, no leftover file
- [ ] `lx.openFile` / `lx.chooseFile`: JS-side cancel and error `code` (OS picker stays external)

### 2.3 Surface / shell — register what exists, fill holes

Desktop `surface-workspace.test.ts` and `terminal.test.ts` are deep but mostly
**not** `contract()` cases, so the ledger still says `external-fixture`.

- [ ] Promote existing hide/show/close/idempotent/event-count cases to `contract()` with honest `covers`
- [ ] Re-home `lx.surface` / `lx.shell` owners off `DESKTOP-BROWSER-001` (that case never calls `lx.surface`)
- [ ] `lx.surface.openUrl` + `TabSurface` (`activate` / `scope` as specified)
- [ ] `lx.surface.onContext` subscribe, first call, viewport change, unsubscribe
- [ ] `PageSurface.postMessage` / `onMessage` (ui ↔ surface page)
- [ ] `lx.shell.reconfigure`
- [ ] `lx.shell.sidebarActions.replace` / `update` / `remove` / `clear` (home-only, `E_NOT_FOUND`, `replace([])`)

### 2.4 Video context the video page already implements

- [ ] Replace `NATIVE-VIDEO-001` empty-fixture pause click
- [ ] `play` / `pause` / `stop` / `seek` with an observable position or event
- [ ] `requestFullScreen` / `exitFullScreen` (or documented `unsupported`)

---

## Wave 3 — honesty + remaining public surface

### 3.1 Ledger and coverage map

`tests/support/capability-ledger.ts` and `tests/logic-api-coverage.mjs` must
match the cases, not the titles.

- [ ] `LOGIC-002` must not own lifecycle for five listener APIs
- [ ] `DESKTOP-BROWSER-001` must not own `lx.surface` / `lx.shell`
- [ ] `COMPONENTS-001` must not be the sole owner of `lx.navigateTo`
- [ ] Align `logic-api-coverage.mjs` modes with workspace tests (`openDeclared` / `get` / `openApp` are no longer `external-fixture` once wave 2 registers them)
- [ ] `automated` + `requiredLevels` ⇒ those levels exist as assertions in the owner case
- [ ] Split `SHAPE_ONLY` that actually have semantics (`app.getBaseInfo`, `env`) from those that do not (`tray` until wave 3.3)

### 3.2 Storage / fs holes

- [ ] `Storage.clear`, `list(prefix)`, get-missing vs stored `null`, persist across `lx.reLaunch`
- [ ] Failure: oversized `set` — `code` and key absent
- [ ] `fs.readDir`, `LxFile.exists` / `path`, directory `stat`, overwrite flags
- [ ] Path escape / non-`lx://` deny with stable `code`

### 3.3 Listeners that currently only subscribe

Replace `LOGIC-002` with one case per event that **fires**:

- [ ] `onNetworkChange` — real or injected change, then unsubscribe is inert
- [ ] `onDeviceOrientationChange` — after `setDeviceOrientation` or host rotate
- [ ] `onKeyDown` / `onKeyUp` — via `PageDriver.key` / host key
- [ ] `onWifiConnected` — or demote to `external-fixture` honestly

### 3.4 Showcase-used OS / media (JS half)

Each item: Logic starts the call, cancel/deny has `code`, state unchanged.
System dialog itself stays device-lab.

- [ ] `lx.share` (text / page / files) — `completed | dismissed | unknown`
- [ ] `lx.getLocation` — coords or deny `code` (macOS sheet helper is not enough)
- [ ] `lx.chooseMedia` / `lx.scanCode` / `previewMedia` handle (`presented`, `onChange`, `completed`)
- [ ] `lx.compressImage` / `compressVideo` / `extractVideoThumbnail` / `getVideoInfo`
- [ ] `lx.saveImageToPhotosAlbum` / `saveVideoToPhotosAlbum`
- [ ] `lx.setDeviceOrientation`
- [ ] `lx.openExternal`
- [ ] wifi suite (`startWifi` … `connectWifi`)
- [ ] `lx.vibrateShort` / `vibrateLong` / `makePhoneCall` — call + platform `unsupported` `code`
- [ ] `lx.app.autostart.isEnabled` / `setEnabled`
- [ ] `lx.app.exit` — isolated fixture only (destructive)
- [ ] `lx.getUpdateManager` + `onUpdateReady` / `applyUpdate` / `onUpdateFailed` — fixture

### 3.5 Public but unused in showcase

Keep shape. Add behavior only if the product starts using them, or when we
decide they are required for every host:

- [ ] `lx.tray.*` (7 methods)
- [ ] `lx.uploadFile`
- [ ] `lx.chooseDirectory`
- [ ] `lx.navigateBackApp`
- [ ] `lx.app.screenshot` / `checkUpdate` / `setBadge`

---

## Per-API checklist (review 1)

Tick only when the wave item above is done **and** a later review agrees.
`S` = shape exists. Other columns are behavior.

| API | S | Sem | Fail | Bound | Life | Notes |
|---|---|---|---|---|---|---|
| `app` | x | partial | | | | `getBaseInfo`/`envVersion` only |
| `appearance` | x | | | | | showcase uses get/set — wave 1.2 |
| `automation` | x | x | partial | x | | nested drivers still shape |
| `chooseDirectory` | x | | | | | unused |
| `chooseFile` | x | | | | | showcase file/share — 3.4 |
| `chooseMedia` | x | | | | | showcase media — 3.4 |
| `compressImage` | x | | | | | showcase media — 3.4 |
| `compressVideo` | x | | | | | showcase media — 3.4 |
| `connectWifi` | x | | | | | 3.4 |
| `createVideoContext` | x | | | | | empty-fixture pause — 2.4 |
| `downloadFile` | x | | | | | showcase file — 2.2 |
| `env` | x | partial | | | | cache path used; data path is “string” |
| `extractVideoThumbnail` | x | | | | | 3.4 |
| `fs` | x | x | weak | x | x | no `readDir` — 3.2 |
| `getConnectedWifi` | x | | | | | 3.4 |
| `getDeviceInfo` | x | x | | x | | `osName` only |
| `getImageInfo` | x | x | weak | | | any-throw |
| `getLocation` | x | | | | | macOS sheet helper only |
| `getLxAppInfo` | x | x | | x | | `appId` only |
| `getNetworkInfo` | x | x | | x | | snapshot |
| `getScreenInfo` | x | x | | x | | w/h/scale > 0 |
| `getStorage` | x | x | weak | x | partial | no `clear` / relaunch — 3.2 |
| `getSystemSetting` | x | x | | x | | `wifiEnabled` boolean |
| `getUpdateManager` | x | | | | | showcase launch — 3.4 |
| `getVideoInfo` | x | | | | | 3.4 |
| `getWifiList` | x | | | | | 3.4 |
| `hideToast` | x | | | | | wave 1.4 |
| `makePhoneCall` | x | | | | | 3.4 |
| `navigateBack` | x | x | | x | x | no `delta` / root fail — 2.1 |
| `navigateBackApp` | x | | | | | unused |
| `navigateTo` | x | x | | x | x | no failure contract — 2.1 |
| `navigateToApp` | x | | | | | showcase API page — 2.1 |
| `navigationBar` | x | | | | | showcase ui/cloud/media — 1.1 |
| `onDeviceOrientationChange` | x | | | | | subscribe only — 3.3 |
| `onKeyDown` | x | | | | | subscribe only — 3.3 |
| `onKeyUp` | x | | | | | subscribe only — 3.3 |
| `onNetworkChange` | x | | | | | listen flag only — 3.3 |
| `onWifiConnected` | x | | | | | subscribe only — 3.3 |
| `openExternal` | x | | | | | 3.4 |
| `openFile` | x | | | | | 2.2 |
| `previewMedia` | x | | | | | 3.4 |
| `reLaunch` | x | | | | | never called; suites use NavDriver — 2.1 |
| `redirectTo` | x | x | | x | | 2.1 |
| `saveImageToPhotosAlbum` | x | | | | | 3.4 |
| `saveVideoToPhotosAlbum` | x | | | | | 3.4 |
| `scanCode` | x | | | | | 3.4 |
| `setDeviceOrientation` | x | | | | | 3.4 |
| `setMoreActions` | x | | | | | `lxapp.ts` — wave 2/3 |
| `share` | x | | | | | render smoke only — 3.4 |
| `shell` | x | x | partial | x | x | desktop tests unregistered; sidebar/reconfigure empty — 2.3 |
| `showActionSheet` | x | | | | | wave 1.4 |
| `showModal` | x | | | | | wave 1.4 |
| `showToast` | x | | | | | wave 1.4 |
| `startPullDownRefresh` | x | x | | | | no failure / leak — later |
| `startWifi` | x | | | | | 3.4 |
| `stopPullDownRefresh` | x | x | | | | paired with start |
| `stopWifi` | x | | | | | 3.4 |
| `supports` | x | x | x | x | | **meets bar** |
| `surface` | x | x | partial | x | x | `openUrl`/`onContext`/messages empty; owner case is wrong — 2.3 |
| `switchTab` | x | x | | x | x | 2.1 |
| `tabBar` | x | x | partial | partial | partial | wave 1.3 |
| `terminal` | x | partial | | x | | settings only, desktop |
| `tray` | x | | | | | unused — 3.5 |
| `uploadFile` | x | | | | | unused — 3.5 |
| `vibrateLong` | x | | | | | 3.4 |
| `vibrateShort` | x | | | | | 3.4 |

---

## Cases that must not stay as owners

Perfunctory bodies that currently satisfy the ledger:

| ID | File | Problem |
|---|---|---|
| LOGIC-002 | `api/runtime.test.ts:58` | Five listeners: subscribe/unsubscribe only |
| DESKTOP-BROWSER-001 | `platform/desktop/browser-cover-restore.test.ts:5` | Listed as `lx.surface` + `lx.shell` owner; never calls `lx.surface` |
| NATIVE-VIDEO-001 | `pages/native-components.test.ts:16` | Pause on empty video fixture |
| COMPONENTS-001 | `pages/components.test.ts:5` | Sole `navigateTo` owner; stack name only |
| UI-NAV-001 | `pages/ui.test.ts:8` | Four nav APIs; stack length only |
| LOGIC-005 | `api/io-contracts.test.ts:4` | Any throw = pass |
| DEVICE-002 | `pages/device.test.ts:79` | Network listener flag, no event |
| AUT-002 | `api/automation.test.ts:84` | `21 * 2 === 42` |
| PULL-001 | `pages/pull-to-refresh.test.ts:40` | Buttons only |
| `pages/render.test.ts` | | Page copy mentioning `lx.share` is not coverage |
| `pages/ui.test.ts:215` | | View-side size string; not `lx.surface` reject |

---

## Definition of done (program)

The program is done when a review round can say:

1. Every `LX_API_NAMES` entry is either complete at the required levels, or
   explicitly `external-fixture` / `planned` / unused with a reason in the
   ledger that matches the cases.
2. Every API showcase Logic calls has a `contract()` owner that exercises
   that call through the real boundary.
3. No `automated` ledger row is satisfied by typeof, subscribe-only, or
   “did not throw”.
4. `logic-api-coverage.mjs` and `capability-ledger.ts` agree with each other
   and with the cases.
5. This file’s Review log has a round that found no new holes at the bar.
