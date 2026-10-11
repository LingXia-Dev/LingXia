# Update pipeline

For contributors changing update checks, signing, installation, or platform
update prompts. App configuration belongs in the
[host app guide](../skill/app/project.md#update); publishing belongs in
[distribution](../skill/cli/distribution.md). This document records the
cross-component rules those implementations must preserve.

## Two pipelines

| | Host app | Lxapp and plugin |
| --- | --- | --- |
| Feed identity | `lingxiaId` + platform; no channel | `appId` + channel (`release` or `draft`); platform `any` |
| Update candidate | Strictly higher semver | Strictly higher semver; `draft` also the same version with a different sha256 |
| Applies through | Platform installer or store | Runtime, when no live instance remains |

Both pipelines share signature verification. Host build environment and lxapp
channel are independent; see [env-version.md](env-version.md). Update policy
must remain separate from disk, network, and UI so ordering can be tested
without platform services.

## Trust and installation

- **The host build decides whether signatures are required, with one
  dev-service exception.** A requested lxapp channel alone cannot waive
  verification. A production host explicitly switched to the **dev service**
  accepts unsigned **lxapps/plugins** on every channel, the home lxapp
  included: home and guest lxapps share one update path. Prod-service
  packages and host self-updates retain the build environment's signature
  requirements.
- **Production direct updates require embedded trusted keys.** Store hosts
  use only the version and optional release notes; they do not verify or use
  package fields. Lxapp and plugin update verification is unchanged.
- **The signed manifest establishes package version and checksum.** Package
  verification binds `kind`, `targetId`, `channel`, `platform`, `version`, and
  `sha256`. An unsigned version-only store signal has no signed manifest: its
  version and release notes are advisory. Store opening uses only the locally
  configured listing identity and still requires a user action.
- **Signed envelope.** One compact JSON manifest plus 1–2 Ed25519
  signatures over those JSON bytes (not the base64). `signatures` is an array
  so a rotation can carry old and new keys; verification is OR against the
  embedded public keys. The CLI emits one signature. Host packages sign
  `channel: ""`.
- Providers must preserve the exact signed bytes. Request-controlled fields
  must not select the signature verification scheme.
- After unpacking an lxapp, validate its own `lxapp.json`: expected app id,
  version, and compatible runtime. Response `minRuntime` permits early refusal
  but cannot replace this check.
- **Commit install metadata last.** On unpack or validation failure, discard
  the failed candidate and its download record, retain the previous install,
  and allow the next open to fetch fresh bytes. Delete the previous install
  directory only after the replacement is recorded.

## Host updates

### Windows distributions

Preserve the `*-windows.zip` feed archive: a runnable root payload for legacy
directory installs plus `.lingxia-update/{manifest.json,setup.exe,portable.exe}`
for the selected direct formats. The manifest binds schema version, app id,
version, PE architecture, and executable name to the running installation and
verified feed version. Package from a private copy of the build payload; sign
before hashing. Generated `assets/app.json` supplies the env-specific identity.

NSIS updates run Setup against the owned install root and retain user data and
the previous payload on a failed swap. Portable updates wait for both host and
launcher, then replace the outer EXE with a backup. Helpers must start outside
the application directory so they cannot block replacement or extraction
cleanup. Legacy directory updates exclude `.lingxia-update`; Store and sideloaded
MSIX updates stay OS-managed. Never mirror into an extracted portable directory.

NSIS installs per user under `%LOCALAPPDATA%/Programs/<appId>/app`, registers
Start Menu/desktop shortcuts (named from `productNames` for the installing
user's UI language) and an uninstaller, and preserves user data. NSIS and
portable builds detect WebView2; when it is missing they offer to download
Microsoft's bootstrapper and verify its Authenticode signature before running
it, so a first install then needs internet. Signing covers payload EXEs/DLLs,
NSIS uninstallers, the final Setup/Portable EXEs, and MSIX; artifact checksums
are computed afterwards. Authenticode is independent of feed signatures. The
feed has one entry per platform: artifact architecture is recorded and checked,
but shipping more than one Windows architecture needs separate feeds or
identities. MSIX never overwrites its package directory; packaging does not
host an `.appinstaller` feed.

### Channel and version signal

Effective channel precedence is: detected store installation, per-platform
build configuration, shared build configuration, then platform default
(iOS/Harmony: store; others: direct). Configuration is baked into each binary;
changing product configuration does not rewrite installed builds.

Store detection must distinguish a store installation from sideloading or the
app installing its own update. On Android, an initiating installer counts only
when it is a known store. Re-evaluate provenance so a sideloaded app can switch
to store updates after a store installs it.

Self-update capability requires both platform support and effective `direct`
channel. Capability queries and update behavior must use that same decision.

Store builds still check the feed, but never download an installer. Publish
their feed entry **only after the store listing is live**. Automatic store
prompts are snoozed per version for three days after presentation is accepted;
a newer version may prompt again. Explicit JS apply requests are not snoozed.
The store update contract is `version` plus optional `releaseNotes`; the host
ignores other response fields. Direct builds require both `downloadUrl` and
`sha256`; production direct updates also require `authentication`. Lxapp and
plugin packages retain their download and signature requirements.

### Store destinations

Listing ids come from the existing store identity (`ios.store.appId`,
`macos.store.appId`, `harmony.store.appId`, `windows.store.appId`, the Android
package) and are baked into `app.json` as `storeListingIds`; a `store` platform
missing one warns at build time.

- Apple opens `itms-apps://apps.apple.com/app/id…`, falling back to the HTTPS
  listing; the storefront follows the signed-in Apple ID.
- Android opens the store that installed the APK (Play, Huawei, Honor, Xiaomi,
  OPPO, vivo, Samsung, Amazon, Yingyongbao): `market://details?id=` addressed
  to that store's package, then that store's own scheme, then an HTTPS page.
  The `android.*Store` blocks are publish identity, not listing URLs.
- Harmony opens `appmarket://details?id=` with the bundle name, falling back
  to the AppGallery HTTPS page.
- A store-installed process (Play, App Store, MAS receipt, Microsoft
  Store-signed package) is always `store`, so a sideloaded Android APK later
  updated from Play flips channel in place when `applicationId` and signing
  key match. Sideloaded or developer-signed MSIX keeps the build's channel.

### Automatic flow and JS ownership

- Automatic updates start on the first connected-network event. Failure permits
  retry on the next connection; completion prevents another automatic attempt
  in that process.
- A successful `lx.host.checkUpdate()` transfers update ownership to JS for the
  rest of the process. Automatic prompting and downloading stop; an already
  running download is not canceled. A failed check claims nothing.
- Automatic flow may prompt, but **opening a store requires user confirmation
  or an explicit JS apply request**. Inability to present a prompt must not
  fall back to opening the store.
- Direct updates download silently, then prompt once. Check that the candidate
  is newer both before download and before installation: another mechanism
  may update the app in between. Reuse cached bytes only on checksum match.
- `installRequested` and `storeOpened` mean handoff to the OS, not successful
  installation. The next cold start is the only confirmation. Keep the task
  result, progress termination, and public event union consistent.

### Platform prompts

An automatic update can reach the prompt stage before any UI exists. Platforms
must retain and replay the prompt when UI becomes available; otherwise the
process may never prompt again. Report presentation success only when the
prompt is visible or reliably deferred. Optimistic success followed by a modal
failure consumes the three-day store snooze.

Exclusive-tray hosts present updates through the tray; hosts with a persistent
window retain the window callout. Store destinations use the running package
identity and embedded listing ids, never App Link hosts that reopen the old
binary. Android should try the installing store first and handle launch
failure directly; package visibility makes availability preflight unreliable.

## Lxapp updates

- **First install blocks opening; updates do not.** First install has a
  15-second ceiling. Bundled apps register without network access; missing both
  bundle and provider fails the open.
- Explicit channel changes retire the old session and create one with the target
  channel and its storage scope, including the home lxapp. Wait for the old
  Logic worker ACK outside admission and transition locks before opening.
  Failed ACKs quarantine the worker without poisoning later opens. Same-channel opens
  preserve live sessions. Host-bundled assets satisfy release first installs;
  draft packages come from the draft feed. DevPath sessions remain local drafts.
  Host control surfaces cannot switch to downloaded drafts or inherit their
  authority onto an installed replacement.
- Check and download updates in the background. Apply a downloaded update only
  when no live instance exists: initial process startup, after a real close,
  or on restart. Reopening a still-running session must preserve its bundle
  and WebView. `updateManager.applyUpdate()` restarts the lxapp.
- If applying a pending update fails, the same open/restart must rebuild from
  the previous installed or bundled package. Do not leave a retired session
  without a replacement. With no usable package, the open still fails.
- Permit one background check per app id and channel at a time. Drop duplicate
  requests instead of queuing them, and ignore checks whose channel differs
  from the running instance.
- Bundles served live from a local development path are not OTA-managed. Every
  update entry point must leave them alone.
- Versions only move forward on both channels: a newer server version
  installs and an older one never does, so a server-side rollback cannot
  downgrade a device. The client decides; the server only reports its latest
  package. A host-bundled lxapp without an install record compares against its
  own `lxapp.json` version. An unparseable server version never installs over
  an existing one. An exact-version open older than the install is rejected.
- Only on the draft channel may a republish replace the same version with a
  different checksum, in every service env; `release` compares semver alone.
  A missing stored checksum counts as different, including bundled and
  sideloaded installs. Exact-version draft opens therefore still query the
  server.

## Source entry points and verification

- [Update policy and verification](../../crates/lingxia-update/src/)
- [Host update coordination](../../crates/lingxia-service/src/update.rs) and
  [host integration](../../crates/lingxia/src/update.rs)
- [Lxapp update lifecycle](../../crates/lingxia-lxapp/src/update/)

New check paths must preserve shared verification and feed eligibility rules.
Channel defaults must remain shared with project scaffolding. Test prompt
startup, JS ownership, terminal events, and failed-install recovery when
changing those behaviors. A macOS workspace check does not cover the Android,
Harmony, or Windows platform implementations; use target CI or devices.
