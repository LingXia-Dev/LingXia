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
| Update candidate | Strictly higher semver | `release`: different version; `draft`: different version or sha256 |
| Applies through | Platform installer or store | Runtime, when no live instance remains |

Both pipelines share signature verification. Host build environment and lxapp
channel are independent; see [env-version.md](env-version.md). Update policy
must remain separate from disk, network, and UI so ordering can be tested
without platform services.

## Trust and installation

- **The host build decides whether signatures are required.** A requested lxapp
  channel cannot waive verification. A production host may open a draft lxapp,
  but the package must be signed for that channel.
- **Production direct updates require embedded trusted keys.** Store hosts
  use only the version and optional release notes; they do not verify or use
  package fields. Lxapp and plugin update verification is unchanged.
- **The signed manifest establishes package version and checksum.** Package
  verification binds `kind`, `targetId`, `channel`, `platform`, `version`, and
  `sha256`. An unsigned version-only store signal has no signed manifest: its
  version and release notes are advisory. Store opening uses only the locally
  configured listing identity and still requires a user action.
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
- Check and download updates in the background. Apply a downloaded update only
  when no live instance exists: initial process startup, after a real close,
  or on restart. Reopening a still-running session must preserve its bundle
  and WebView. `updateManager.applyUpdate()` restarts the lxapp.
- Permit one background check per app id and channel at a time. Drop duplicate
  requests instead of queuing them, and ignore checks whose channel differs
  from the running instance.
- Bundles served live from a local development path are not OTA-managed. Every
  update entry point must leave them alone.
- Draft republishing may replace the same version with a different checksum.
  A missing stored checksum counts as different, including bundled and
  sideloaded installs. Exact-version draft opens therefore still query the
  server. Release checks compare versions only; the registry enforces
  monotonically increasing releases.

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
