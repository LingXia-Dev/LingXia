---
title: CLI
description: What lingxia and lxdev each own, and why --help is the flag reference.
sidebar:
  order: 12
---

LingXia ships two binaries. Memorize the split; flags belong to `--help`.

| Binary | Owns | Typical commands |
|---|---|---|
| `lingxia` | Project lifecycle | `new`, `doctor`, `dev`, `build`, `package`, `publish`, `upgrade` |
| `lxdev` | A live `lingxia dev` session | `lxapp`, `app`, `desktop`, `browser`, `test`, `logs`, `runner`, `session` |

`lingxia` scaffolds, builds, installs or launches, and keeps the authenticated development websocket alive. `lxdev` never starts a session. It connects, runs one command, prints the result, and exits (`logs -f` is the exception).

```bash
lingxia --help
lingxia new --help
lingxia dev --background
lingxia dev stop

lxdev --help
lxdev session
lxdev lxapp nav --help
lxdev logs -f
```

The command set is dynamic by project type. `lxdev <family> <command> --help` is the version-matched list for the session you are in. This site does not duplicate those flags.

## Start, take over, stop

```bash
lingxia dev                  # interactive; owns the session in this terminal
lingxia dev --background     # returns when the runtime websocket is ready
lingxia dev stop             # from the project; stops this project's same-platform owner
```

Re-running `lingxia dev` takes over the same project's same-platform session. Different platforms run side by side. One live session is selected automatically; several require `lxdev --session <id-or-target> …` before the family name.

## Native host mains

```bash
lingxia new my-app -t native-app -p macos,windows -y
lingxia new my-terminal -t native-app --main terminal --control native -y
lingxia new my-browser -t native-app -p windows --main browser --control lxapp -y
```

`--main` and `--control` apply only to native hosts. See [What you build](../what-you-build/) and [Native host apps](../native-host-apps/).

## What to read next

- [Getting started](../getting-started/) — install and create a project
- [Development workflow](../development-workflow/) — reload, automate, and verify
- [Testing](../testing/) — `@lingxia/test` and `lxdev test`
- [Agent control](../agent-control/) — the shipped-product command line, not `lxdev`
