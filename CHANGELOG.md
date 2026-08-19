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
