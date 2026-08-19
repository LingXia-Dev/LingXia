# Changelog

Generated from the commit log — a commit subject *is* its changelog entry, so
nothing here is written by hand. Regenerate the pending section any time:

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
