# Changelog

From 0.13.0 on, generated from the commit log — a commit subject *is* its
changelog entry, so nothing here is written by hand. 0.12.0 is the exception:
it is where this file starts, so it says so rather than restating the history
behind it. Regenerate the pending section any time:

```bash
scripts/release/main.sh changelog
```

LingXia is `0.x`. Anything may change in a minor release; a change that breaks
callers is listed under **Breaking** with the release that made it.

Entries are grouped by who a change lands on, not by commit type — the same fix
means different things to someone writing an lxapp and someone embedding the
SDK. Changes worth more than one line carry a `Release-Note:` trailer and are
written out in full in that release's notes on GitHub.

<!-- releases below -->

## 0.14.0 — 2026-09-01

### Breaking

- **Breaking** — **splash**: unify the fixed launch face and campaign handoff (78483d3a7)
- **Breaking** — **lxapp**: drop selectedIconPath; one icon per tab item (6cdfb363b)

### Writing an lxapp

- **splash**: close the launch-layer holes and drop the dark-face scaffolding (25aa83795)
- **theme**: let the host declare the page floor native chrome borders (89e06b191)
- **tabbar**: refine overflow navigation (3df40e429)
- **lxapp**: let a tab item declare the hosts it belongs on (5d462e0e7)
- **lxapp**: fold on the iOS phone strip, and mark "More" everywhere (4f4bcfc50)
- **lxapp**: draw the active indicator for every selected tab (29ad9726e)
- **lxapp**: make a single-icon tab read like a paired one when selected (75505dff3)
- **lxapp**: mark the active tab without a second icon (006956c36)
- **lxapp**: warm only the tab pages holding a strip slot (d4b56f068)
- **lxapp**: allow up to 10 tab items with a compact overflow split (50155df05)
- **lxapp**: expose window chrome selection (6291fe143)
- **lxapp**: load logic-disabled surface pages (0e57cd376)

### Embedding a host app

- **macos**: sync SwiftPM deployment target (b36306bb1)
- **ios**: compose the launch frame with a storyboard, not UILaunchScreen (d569d19cc)
- **android**: launch in the orientation the home page will use (09da37b3f)
- **android**: let a full-screen bottom surface reach the bottom edge (13eac1cad)
- **ios**: bundle packet tunnel extensions (7fba2ef3a)
- **config**: admit platform-specific lxapp roots (5cbdf757f)
- **windows**: key the overflow panel on declared indices (bacb9bc1f)
- **windows**: match tabbar overflow sheet (9415bfdd5)
- **macos**: make the tab overflow panel visible over the WebView (3158ee7a9)
- **windows**: fold extra tab items into a "More" slot (35b8ff85e)
- **harmony**: fold extra tab items into a "More" slot (38e016eb0)
- **apple**: fold extra tab items into a "More" slot (90096213c)
- **android**: fold extra tab items into a "More" slot (f87fa27f1)
- **macos**: restore full-chrome window behavior (b727a97ac)
- **macos**: clip tray panel corners cleanly (a9faa125c)
- **macos**: dismiss tray panel when opening window (8568b7d01)
- **windows**: realize shell before first frame (2eddcc5bb)
- **windows**: restore lxapp after browser closes (da34760ad)
- **macos**: keep capsule page chrome synchronized (d28f1afcc)
- **windows**: collapse nested if so clippy -D warnings passes (9338cf92e)
- **windows**: sync the runner capsule overlay on --capsule toggle (74016ec43)
- **platform**: present desktop file dialogs without parking the runloop (153bc940e)
- **android**: keep a tab page's WebView in the window between visits (0f8487cc1)
- **android**: actually run the launch cover's deferred restores (286273803)
- **windows**: make the runner device picker actually resize the frame (2c9300269)
- **harmony**: point the video settings button at an icon that exists (1fe06e783)

### Rust native extensions

- **lingxia**: keep page target out of root facade (0061709a7)
- **control**: let hosts extend the product CLI (74c697bf0)

### CLI and CI

- **ci**: stabilize main test suite (297a4a34e)
- **ci**: exclude the private provider checkout from the workspace (db442c9fa)
- **ci**: keep cloud checkout inside workspace (0550dd859)
- **runner**: fold tab items only in the phone shapes (2bbf51b21)
- **cli**: skip page action audit without logic (8fdf17e8f)
- **cli**: close upgrade review gaps (f8d8d38d2)
- **cli**: gate Windows upgrade rerun helper (7898ddc42)
- **cli**: close project upgrade safety gaps (5864c2be8)
- **cli**: harden project upgrade boundaries (611958fdc)
- **cli**: always upgrade CLI and prompt for project SDK line (b09a6a926)
- **cli**: project upgrade, version-train guard, sdk drift checks (c0be6294d)
- **cli**: harden credential readiness and rotation (d1dc6fb66)
- **cli**: separate LingXia auth from publish actions (d15fc86ce)
- **cli**: include all wallet entries in JSON status (2a80ff95d)
- **cli**: wallet-backed store credentials, artifact identity precheck, publish tokens per server (2f95b9bf8)
- **cli**: store Harmony AGC credentials per identity (e5d5fc1f5)
- **cli**: identity wallet, project binding, unified auth surface (cd311abcf)
- **cli**: route per-user state through lingxia_dir() (3fb8aa32b)
- **runner**: look up the built app by its actual bundle name (841998c02)
- **runner**: report the simulated capsule's geometry and make it optional (096f877f3)
- **cli**: catch a page entry that forwards only some of its actions (13249a68a)
- **cli**: add `lingxia browser-shell eject` (1c408a141)
- **release**: stop publishing the browser shell webui to npm (8b8005d71)
- **cli**: find the installed Runner bundle by extension, not by name (a39ccabf6)

### Docs and examples

- **showcase**: demo responsive tab bar overflow (e9130c988)
- **showcase**: keep the four declared tab items (c161e7f9d)

## 0.13.0 — 2026-08-26

### Writing an lxapp

- **update**: spell the developer channel `developer` everywhere (a9f06b5b8)
- **update**: install and update lxapps on the host build's channel (8d24d5f73)
- **upload**: keep the writer's own diagnosis when it is the cause (018c6a786)
- **upload**: report a denied host as denied, not as a dropped connection (1cb71e85f)
- **test**: add numeric ordering matchers, and report both numbers (bf5d50e72)
- **upload**: let lx.uploadFile send a raw body with PUT (5780ae992)

### Embedding a host app

- **android**: keep the launch canvas on the cover's colour until it is gone (e80f0b67a)
- **android**: let the launch cover own the system bars until it lifts (74e9e2b3e)
- **android**: reserve the bottom inset only while the TabBar is on screen (be7155fdf)
- **windows**: drop unused ReleaseType import (40f7fb11a)
- **media**: stop the capture before announcing that it stopped (#284) (d4cfc211c)

### CLI and CI

- **release**: find the Runner artifacts the CLI actually wrote (3cab7c928)
- **cli**: name artifacts from project name (b3d7170b3)
- **ci**: recycle flaky macOS automation once (e04326a01)
- **ci**: let cancelled PR automation exit promptly (4717559ec)
- **ci**: retry showcase dependency installs (484516304)
- **ci**: keep Windows cache uploads from failing PRs (a80c698af)
- **cli**: keep the Android 12 splash icon when no cover is configured (#285) (fce0466d1)
- **cli**: cover both Swift target layouts in the Apple ignore rules (cea8cd33b)
- **cli**: declare the lxapp icon the scaffold already writes (8d18e498f)
- **cli**: harden SDK dependency detection in Package.swift (9f5444892)
- **cli**: stop Apple build output landing in git status (ccf8c1b37)
- **cli**: inject the Apple SDK dependency for macOS builds too (2540f1901)
- **cli**: import Lingxia from com.lingxia.app in the Android template (9dc5059b2)
- **release**: let a CLI-only publish leave the workspace behind (a27590cc2)
- **cli**: stop asking to install a skill that is already there (cf32976f1)
- **cli**: name an embedded lxapp after itself, not only its host (7db55100a)
- **cli**: add lingxia upgrade (869332554)
- **cli**: derive the reported Rong version from the workspace (c9987418f)
- **release**: restore the CLI-only version bump (48eefa012)
- **cli**: route the docs at the built-in skill installer (aa94d5bfa)
- **cli**: address the review of the embedded skill (6fbdbfc41)
- **cli**: ship the agent skill inside the binary (b80ddb70c)
- **release**: install the packages workspace before building a member (0427f535c)

### Docs and examples

- **docs**: name @lingxia/test as the test SDK, not @rongjs/test (2032ca501)

## 0.12.0 — 2026-08-19

The first release this file covers. What came before it is the git log, not a
change list: entries begin here, and every release after this one records its
own changes in full.

0.12.0 is the whole of LingXia at one version — the runtime for standalone
lxapps and native host apps on Android, iOS, macOS, HarmonyOS, and Windows, the
`lingxia` and `lxdev` CLIs, the Android, Apple, and HarmonyOS SDKs, 29 crates on
crates.io, and 12 `@lingxia/*` npm packages. One of those is new: `@lingxia/test`,
an authoring SDK for lxapp tests.

LingXia is `0.x` and makes no compatibility promise. 0.12.0 breaks callers of
0.11 in places, and ships no migration guide — pin the version you build
against.
