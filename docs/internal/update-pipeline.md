# Update pipeline

> **Audience**: contributors touching `crates/lingxia-update`, the host
> auto-update flow, the lxapp installer, update signing, or a platform's
> install / store prompt. If you're **building an app on LingXia**, the yaml
> surface is in [`docs/skill/app/project.md#update`](../skill/app/project.md#update)
> and publishing in [`docs/skill/cli/distribution.md`](../skill/cli/distribution.md).
> This doc states the rules the code relies on but does not spell out.

## Two pipelines, one trust path

| | Host app | Lxapp (and plugin) |
| --- | --- | --- |
| Identity on the feed | `lingxiaId` + **platform**, no channel | `appId` + **channel** (`release` \| `draft`), platform `any` |
| Newer means | strictly higher semver | `release`: different version. `draft`: different version **or** different sha256 |
| Who applies | platform installer, or the store | the runtime, on the next open of that lxapp |
| Code | `lingxia-update/src/app.rs`, `lingxia-service/src/update.rs`, `lingxia/src/update.rs` | `lingxia-update/src/lxapp.rs`, `lingxia-lxapp/src/update/` |

Both go through the same `UpdateProvider::check_update` and the same
`verify_checked_update`. Env vs channel is covered in
[`env-version.md`](env-version.md); this doc assumes it.

`lingxia-update` holds policy behind the `AppUpdateHost` / `LxAppUpdateHost`
traits and never touches disk, network, or UI. Keep it that way: the traits
are what make the ordering rules below unit-testable.

## Trust

1. **Whether a signature may be waived belongs to the build, never the
   request.** `host_requires_signature()` reads the host env only. An App Link
   or `navigateToApp({ channel })` chooses the lxapp channel; keying the waiver
   on that would let anyone who can hand a prod device a link ask for `draft`
   and be served an unsigned package. The requested channel still *binds* the
   manifest — a prod host may open a draft lxapp, but only one signed for
   `draft`.
2. **Prod without embedded keys never queries the feed.**
   `check_update_enabled` returns false, for host and lxapp alike. This is why
   the yaml `update:` table requires `trustedPublicKeys` even when every
   platform is `store`: no keys means no version signal, silently. Dev always
   queries and verifies only when keys are present.
3. **The signed manifest is the only source of `version` and `sha256`.** It
   binds `kind`, `targetId`, `channel`, `platform`, `version`, `sha256`. After
   verification the manifest's version and sha256 overwrite whatever the
   response carried. Everything else on the response — `url`, `size`,
   `releaseNotes`, `minRuntime` — is unsigned and only a hint.
4. **Providers must not reserialize `signed`.** The signature covers the exact
   bytes. There is no scheme identifier outside the signature on purpose: a
   field the caller controls must not select how it gets verified.
5. **What the archive says about itself is checked after unpack.** A
   downloaded lxapp's own `lxapp.json` must name the expected `appId` and
   version and satisfy `minRuntime`, before install metadata commits. On
   failure the unpacked dir and the download record are removed so the
   previous install stays and the next open fetches fresh bytes.
   The response's `minRuntime` is only an early refusal (`6002`) that saves
   the download; the archive's manifest is the gate that counts.

## Host app

### Effective channel

```
store-installed process ──► store
otherwise: update.platforms[p] ► update.channel ► compile default
           (ios / harmony: store, the rest: direct)
```

- The yaml value is baked into **this build's** `app.json`. It describes the
  binary, not the product; shipping new yaml never rewrites installed builds.
- `installed_from_store()` wins over yaml so a sideloaded Android build flips
  in place once the user updates it from a store. Android trusts
  `installingPackageName`, and `initiatingPackageName` only when it is a known
  store, so a sideload session cannot pose as one. A self-update makes the app
  its own installer, which is not a store.
- `self_update_supported()` on the service is `platform can` ∧ `effective
  channel is direct`. Use the service, not the platform trait, when deciding
  behavior. `lx.supports({ capability: 'selfUpdate' })` reports the same value.

### The feed is the version signal on every channel

`store` still calls `check()`. It just never downloads. Consequence that is
not visible in code: **a store platform's feed package must be published only
after the store listing is live**, or users are sent to a listing with nothing
to update. The store prompt is snoozed per version for 3 days
(`app_state/host-update-store-prompt.json`) because review and staged rollout
lag anyway; a newer version asks again. The snooze is written only when a
prompt was actually presented, and only by the auto-flow — JS `apply()` is
never snoozed.

### Auto-flow vs JS

- The auto-flow runs once per process, on the first `isConnected: true`. A
  failed attempt re-arms on the next connect; a finished one does not.
- `lx.app.checkUpdate()` **claims the process after a successful check**, for
  good. From then on the auto-flow neither prompts nor downloads — JS owns
  `apply()`. A failed check claims nothing. A claim does not cancel an
  auto-flow download already in flight.
- **The auto-flow only prompts. It never opens a store.** Opening the listing
  is always a user action: the prompt's confirm, or JS `apply()`. A platform
  that cannot present a prompt returns `false` and nothing happens; do not add
  an open-the-store fallback.
- Terminal events mean "handed to the OS", not "updated":
  `installRequested` (direct) and `storeOpened` (store). Nothing reports
  whether the user went through with it; the next cold start's check is the
  only feedback.

### Direct apply

Download is silent; the single user-visible moment is the post-download
prompt. The downgrade guard (`ensure_app_update_candidate_version`) runs
twice — before download and again before install — because the app can be
updated by other means in between. `check` only surfaces a strictly newer
version, otherwise a provider that re-offers the installed version loops the
prompt forever. The cached package is reused only when its sha256 matches.

### Platform prompt contract

`present_store_update` / `install_update` are called from the auto-flow as
soon as the network is up, which on a cold start is **before any UI exists**
(no Android activity, no iOS key window, no mounted Harmony page).

- A platform must **remember the prompt and replay it when UI arrives**, not
  drop it. Android does this for both prompts (`init` / `onResume`). Dropping
  it loses the prompt for the whole process, since the auto-flow will not run
  again.
- Return `true` only if the prompt was shown or is safely deferred — the
  caller records the snooze on `true`. Harmony awaits its modal off-thread and
  returns `true` optimistically; a modal that then fails costs that version
  its prompt for 3 days.
- Exclusive-tray desktop hosts have no lasting window: the tray owns the
  prompt (menu item + balloon / badge). Dock + tray keeps the window callout.
- Store URLs are built from the running package id plus the baked
  `storeListingIds`. Product `appLinks` hosts are never used — they open this
  app, which would bounce the user back into the old binary.
- Android opens `market://details?id=` **pinned to the installing store**
  first; OEM private schemes are fallbacks. Use `startActivity` inside
  try/catch, never `resolveActivity` (package visibility).

## Lxapp

### When things happen

```
open / navigate ──► ensure_first_install ──► (foreground, blocks the open, 15s ceiling)
                └─► schedule_lxapp_update_check ──► background: check ► download ► UpdateReady
next open of that lxapp ──► apply_downloaded_update ──► unpack ► validate ► commit metadata
```

- **First install blocks the open; updates never do.** A bundled lxapp is
  registered from assets without touching the network. With no provider and no
  bundle, the open fails rather than hanging.
- **A downloaded update is applied only when no live instance exists**:
  process bootstrap (home lxapp), a real close, or `restart()`. A session that
  chose "later" keeps running the old bundle even if it is re-opened — applying
  would tear down its WebView. `updateManager.applyUpdate()` is exactly
  `restart()`.
- One background check per `lxapp:<id>@<channel>` at a time; a second request
  while one runs is dropped, not queued.
- The background check compares against the instance's channel. A check
  scheduled for a different channel than the running instance is a no-op.
- `is_ota_managed()` is false for a bundle served live from a local path (dev).
  Every update entry point must bail out on it; there is no installed package
  to replace.

### Draft republish

`draft` treats same-version + different sha256 as an update, so a republish
needs no version bump. An install with **no stored checksum** (bundled or
sideloaded) counts as different, so the first OTA after it still lands. An
exact-version open on `draft` therefore always asks the server, even when that
version is installed. `release` compares versions only and must only go up —
that rule lives in the registry, not the client.

### Failure must not stick

Anything that fails after download — corrupt archive, wrong `appId`/version,
`minRuntime` — removes the download record, so the next open refetches instead
of failing on the same bytes forever. Metadata commits last; the previous
install dir is deleted only after the new one is recorded.

## Checklist when changing this area

- New check path? It must go through `verify_checked_update` and honor
  `check_update_enabled`.
- New platform prompt? Defer-and-replay before UI exists; return value feeds
  the snooze.
- New terminal state? Add it to `AppUpdateEvent`, the iterator's `done` check,
  `is_terminal_event`, and the TS union together — a missed one hangs
  `for await` or resolves the task as canceled.
- Touching channel resolution? `default_update_channel` is also what
  `lingxia new` renders into the scaffold; keep one source.
- Mobile targets do not compile on macOS `cargo check`; Android JNI, Harmony
  and Windows legs need CI or a device.
