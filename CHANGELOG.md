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
