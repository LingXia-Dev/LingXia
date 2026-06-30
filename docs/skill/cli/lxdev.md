# `lxdev` — Drive a running dev session

`lxdev` is the **agent-facing automation client** for a running `lingxia dev` session. It ships from the same workspace as `lingxia` (crate `lingxia-devtools-cli`, binary `lxdev`) and is on `PATH` after a standard install. Use it to inspect and drive the host app's browser tabs, the lxapps inside it, the host windows, and the dev log stream — without rebuilding or restarting the session.

Every command takes plain flags, prints predictable output, and exits (the one exception is `lxdev logs -f`, which streams until stopped). Run it from anywhere inside the project tree — it resolves the session relative to the current working directory.

---

## `--help` is the source of truth for commands and flags

This file teaches the **model** of `lxdev` — how it finds sessions, the output contract, the two `eval` targets, the concurrency rules. It deliberately does **not** list every command's flags: the binary's own `--help` is exhaustive and always matches the installed version, so read it instead of trusting a hand-copied table.

```bash
lxdev --help                 # global flags + the family list
lxdev <family> --help        # commands in a family (e.g. lxdev browser --help)
lxdev <family> <cmd> --help  # exact flags, defaults, and which are required
```

The command set is also **dynamic** — e.g. a standalone lxapp project exposes fewer `lxapp` subcommands than a host-app project — so `lxdev <family> --help` is the only reliable list for the project you're in.

---

## The five families

`lxdev` exposes five command families. Four target a single resolved session over its dev websocket; `sessions` operates on the session files directly.

| Family | Scope | Reach for it to… |
|---|---|---|
| `browser` | the host app's browser tabs (Playwright-like WebView automation) | open/navigate tabs, `eval` JS, query/click/type, wait for conditions, manage cookies, screenshot web content |
| `app` | the host app as a whole | enumerate top-level windows, screenshot the full app window |
| `lxapp` | lxapps running in the session, and their pages | list/open/close apps, navigate the runtime, `eval` in Logic **or** in a page WebView, automate page elements |
| `logs` | the session's JSONL log stream | tail / follow / filter native + webview + logic logs |
| `sessions` | the session files on disk | list, probe liveness, prune stale sessions |

Run `lxdev <family> --help` for the commands and flags within each.

Plus a top-level **`build`** command (not a family): `lxdev build [--release]` builds the session's lxapp front-end bundle. It is handled by the `lingxia dev` **orchestrator** (which owns the build pipeline), not the runtime — so it works even with no app attached, and reuses the same `--session` selector. Build output streams to the `lingxia dev` terminal (see `lxdev logs`); the command itself reports only success/failure.

---

## How it finds the session

`lingxia dev` writes one file per active session under `.lingxia/sessions/<session_id>.json` and removes it on clean exit. `lxdev` scans that directory on every invocation. Each file records the `session_id`, the `pid` of the `lingxia dev` process, the `platform` (`android` / `ios` / `macos` / `harmony` / `lxapp`), `started_at`, the `ws_url` the dev websocket listens on, and the `log_file` path.

There is **no daemon**. The `lingxia dev` process that started a session owns its file; nothing else writes there. Every subcommand except `sessions` connects to the resolved session's `ws_url`, sends one command, prints the result, and exits.

---

## Selecting a session (and why it refuses to guess)

Invoked **without** `--session`:

- **One session in the project** → use it directly.
- **Multiple sessions** → probe each, drop stale ones, then: exactly one live → use it; still more than one → **refuse** and list candidates.

It refuses rather than defaulting to "most recent" because a human and an agent may both be running dev sessions in the same project, and a silent guess routes commands to the wrong target. Disambiguate with the single **global** `--session` selector — it goes *before* the subcommand (everything else follows it) and accepts either a platform name or a session-id prefix:

```bash
lxdev --session ios browser tabs            # platform name (exact, case-insensitive)
lxdev --session <id-prefix> browser tabs    # or a session_id prefix
```

---

## Output contract

Knowing this up front saves a round-trip — it is not something `--help` spells out:

- **Default is human-readable text** for most commands (aligned tables for `sessions`, `browser tabs`, `app windows`; a bare id for `open` / `current`).
- **`--json`** switches a command to compact machine output; **`--pretty`** (on the `lxapp` and `cookies list` families) prints indented JSON.
- **A few read commands *always* emit JSON** regardless of flags: `browser eval`, `browser query`, `lxapp eval`, `lxapp page eval`, `lxapp page query`. There the flag only toggles compact vs pretty, and `eval` prints just the returned value (nothing when it is `null`).
- **Mutating commands print nothing on success** (`click`, `type`, `fill`, `press`, `scroll`, `activate`, `close`, …) unless you pass `--json`. Silence means success; check the exit code, not stdout.
- Exit code is `0` on success, non-zero with a message on stderr otherwise.
- The websocket command timeout is ~120s; commands that accept `--timeout-ms` (waits, evals, navigations) get that budget plus a small buffer.

---

## The two `eval` targets — don't conflate them

`lxdev` can run JS in **three** distinct contexts. Picking the wrong one is the most common mistake, because the call looks the same but the global scope differs:

| Command | Runs in | Sees |
|---|---|---|
| `lxdev lxapp eval <script>` | the lxapp **Logic runtime** (AppService) | `Page({})` / `App({})` state, `lx.*`, the logic-side world — **no DOM** |
| `lxdev lxapp page eval <script>` | the lxapp's **page WebView** | the rendered DOM, `window`, the View — **no AppService state** |
| `lxdev browser eval --js <code>` | a **browser tab's** page | that tab's DOM (arbitrary web content, not an lxapp) |

So to read app state use `lxapp eval`; to inspect the rendered UI use `lxapp page eval`. `script` may be an expression or a function body using `return` / `await`. (See [`../lxapp/guide.md`](../lxapp/guide.md) for the View/Logic split these mirror.)

> Surface-opening calls (`lx.surface.*`, `navigateTo`) can deadlock when driven from `lxapp eval`; verify those via a real page interaction (`lxapp page click`) instead.

> To navigate the runtime, prefer `lxdev lxapp nav to|relaunch|redirect|switch-tab <page-name>` (names + routes come from `lxdev lxapp pages`). The JS APIs (`navigateTo` / `redirectTo` / `switchTab` / `reLaunch`) take `{ page }` **or** `{ path }` — there is **no `url`** field.

---

## Visual checks — two screenshot scopes

Both are produced **in-app** — no Screen Recording permission, consent prompt, or foreground service — and exclude the system status/navigation bar and IME.

- `lxdev browser screenshot` → **WebView content only** (the tab's web page, no host UI).
- `lxdev app screenshot` → the **full host window** (native controls, host overlays, composited WebViews). On macOS, `lx.surface({kind:'window'})` makes separate windows — enumerate with `lxdev app windows` and target one with `--window`.

To verify a layout against an on-screen IME, capture from the device's own tooling (e.g. `adb shell screencap`) — `lxdev` does not shell out to host screen-capture tools.

---

## Concurrency rules

The whole design assumes a human and one or more agents may do dev in parallel:

1. **`lingxia dev` refuses a second same-platform session** unless `--parallel` is passed — the primary defense against ambiguity.
2. **`lxdev` refuses to act when ambiguity exists** — it lists candidates and asks for `--session` rather than guessing.
3. **Stale sessions don't count.** Pruning happens at `lingxia dev` startup, on `lxdev sessions prune`, and inside `lxdev` when ambiguity forces a liveness probe — so a hard-crashed `lingxia dev` won't keep blocking future runs.
4. **`--parallel` is per-launch** — it does not persist; each `lingxia dev` for that platform must opt in again.

---

## Typical agent flow

```bash
lxdev sessions                                   # discover what's running

# Single session → zero-arg invocation just works
lxdev browser open https://example.com
lxdev browser eval --js "document.title"
lxdev browser wait --visible "#app .ready"
lxdev browser click --css "#login"
lxdev build                                      # rebuild the session's lxapp bundle
lxdev logs -f --level warn

# Inspect an lxapp without a UI round-trip
lxdev lxapp eval "return appService.getState().user"   # Logic runtime
lxdev lxapp page query --css ".cart-item" --all        # page WebView

# What does the user actually see
lxdev app screenshot                             # full app window
lxdev browser screenshot                         # just the tab's web content

# Two sessions running → disambiguate
lxdev --session ios browser tabs
lxdev sessions prune                             # after a hard crash leaves residue
```

Use `lxdev <family> <cmd> --help` whenever you need the exact flags for one of these.

---

## Symptom router

| Symptom | Fix |
|---|---|
| `No active dev session found` | Run `lingxia dev` in this project. |
| `No dev session matches the given selector` | Check `lxdev sessions`; fix the `--session` value (id prefix or platform name). |
| `Multiple active dev sessions match` | Add `--session <id-prefix\|platform>`. |
| `All matching dev sessions are unreachable` | `lxdev sessions prune`, then start a fresh `lingxia dev`. |
| `Existing <platform> dev session is already running` (from `lingxia dev`) | Stop the other one, or pass `--parallel` to run two. |
| `pass exactly one wait condition` / `pass exactly one of --url or --contains` | A `wait` / `wait-url` got zero or several conditions — give exactly one (`lxdev browser wait --help`). |
| `eval` returns nothing / wrong scope | You likely targeted the wrong context — see [The two `eval` targets](#the-two-eval-targets--dont-conflate-them). |
| `lxdev` connects but commands hang | The host app likely lost its bridge — restart `lingxia dev`. |
