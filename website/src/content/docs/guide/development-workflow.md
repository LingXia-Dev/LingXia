---
title: Development workflow
description: Start a LingXia dev session, reload the changed layer, automate behavior, and verify the result.
sidebar:
  order: 8
---

LingXia separates session lifetime from live automation:

- `lingxia dev` builds, installs or launches, and owns the dev session.
- `lxdev` connects to that session to inspect, reload, automate, test, and read logs.

## Start a session

For an interactive terminal:

```bash
lingxia dev
```

For scripts and agents, start in the background. The command returns only after the runtime websocket is ready:

```bash
lingxia dev --background
lingxia dev status
```

Re-running `lingxia dev` takes over the same project's same-platform session. Different platforms can run side by side. Stop the owner from the project with `lingxia dev stop`.

## Reload the layer that changed

| You changed | Run |
|---|---|
| View, Logic, or `lxapp.json` | `lxdev lxapp reload` |
| `lingxia.yaml`, native Rust, or platform project | re-run `lingxia dev` |

`lxdev lxapp reload` rebuilds the lxapp bundle and reloads the running lxapp without creating a new native session.

## Close the verification loop

A successful build is only the start:

1. Navigate to the changed page with `lxdev lxapp nav ...`.
2. Exercise the behavior with `lxdev lxapp page click`, `type`, `fill`, or `press`.
3. Assert the result with page DOM inspection (`page eval` / `query`) or Logic evaluation (`lxapp eval`).
4. Check `lxdev logs` for new warnings and errors.

Use `lxdev app screenshot` for the full native host surface and `lxdev lxapp page screenshot` for one page WebView. Prefer assertable values over screenshots when the expected result is not visual.

## Six command families

| Family | Target |
|---|---|
| `lxapp` | lxapp lifecycle, navigation, page automation, Logic and View evaluation |
| `app` | native host windows, full-surface screenshots, raw mouse and keyboard input |
| `browser` | in-app browser tabs, DOM automation, cookies, screenshots |
| `test` | repeatable API, page, and cross-page test cases |
| `logs` | combined native, lxview, lxlogic, browser, and automation logs |
| `session` | discovery and selection of live sessions |

The command set is dynamic by project type. Use `lxdev <family> <command> --help` for exact, installed-version flags.

## Multiple sessions

One live session is selected automatically, even when `lxdev` runs outside the project directory. If several are live, `lxdev` refuses to guess:

```bash
lxdev session list
lxdev --session ios lxapp current
```

The global selector must appear before the command family.

## Keep repeatable behavior as tests

Use `lxdev test` with `@lingxia/test`. Keep API contracts in `tests/api/`, page behavior in `tests/pages/`, and user journeys in `tests/flows/`. One-off visual polish still deserves live interaction and screenshots, but not necessarily a permanent test.
