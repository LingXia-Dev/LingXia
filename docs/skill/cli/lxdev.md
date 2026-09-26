# `lxdev` — Drive a running dev session

`lxdev` drives a live `lingxia dev` session — a session client that connects to the dev websocket, runs one command, prints the result, and exits (except `logs -f`). It never starts a session; `lingxia dev` owns launch, install, and process lifetime. What it can drive is in **Capabilities** below.

This file says **what `lxdev` can do**. For flags and defaults, `lxdev <family> <cmd> --help` is exhaustive and always matches the installed version — the doc does not duplicate it. The command set is dynamic per project type, so `--help` is also the only reliable list for the project you're in.

## Session selection

`lingxia dev --background` treats the runtime websocket connection as the
readiness boundary, not merely dev-server registration. Once it returns, the
next runtime-backed `lxdev` command will not race Runner/app startup. Both
`lingxia dev status` and `lxdev session list` report `starting`, `ready`, or
`stale`.

**Start a session for automation with `lingxia dev --background`.** `lxdev` needs a *live* session, and a session lives only as long as its owning `lingxia dev` process — a foreground `lingxia dev` blocks the terminal, and if an agent backgrounds it and later loses that process, the session dies with it. `--background` builds, launches, and returns once the session is ready; check it with `lingxia dev status`, stop it with `lingxia dev stop` from the project. Then drive it with `lxdev`.

Each `lingxia dev` session registers with a per-user local broker and stays registered for exactly as long as its process lives; `lxdev` queries the broker, so it works from **any directory** — the session may be one you started or one already running. Without a selector, `lxdev` uses the one live session whose project contains the current directory, else the only live session. Anything else → it **refuses to guess** and prints a table of candidates (`#`, id, name, target, project, started); pick one with the global selector (before the subcommand, or after `lxdev lxapp`) or the `LXDEV_SESSION` env var:

```bash
lingxia dev --background --name demo   # name a session when you start it
lxdev --session demo ...         # its name
lxdev --session ios ...          # target name, when unique
lxdev --session macos@my-app ... # target in a project (dir name or path)
lxdev --session 2 ...            # the # column of `lxdev session list`
lxdev --session a1b2 ...         # session-id prefix
```

Printed hints (a failed spec's `Rerun:` line, the stop command) never carry a session id: they omit `--session` when it is not needed, and otherwise use the name or target.

Crashed sessions disappear from the broker automatically — there is nothing to prune. Closing the Runner or quitting a desktop host ends that session the same way; hiding a host window to the tray does not. Re-running `lingxia dev` for the same target in a project stops the previous session and takes over; different targets run side by side.

`lxdev` intentionally controls sessions registered by `lingxia dev` for the
same user on the same machine. For a remote development machine, run both
commands there through SSH or the machine's CI/device-lab agent; the dev
websocket is not a remote machine-management API.

## Capabilities

**`lxapp`** — the lxapps and runtime pages in the session. Every command targets the **current** lxapp by default (`--app` to pick another); page commands likewise default to the **current page** (`--page` accepts a configured page name or the stable `instance_id` returned by `page current|list|info`):
- `list` / `current` / `info` / `pages` — what's running, and the configured pages
- `open` / `close` / `restart` / `uninstall` — lifecycle (`restart` relaunches the runtime without rebuilding)
- `nav to|redirect|switch-tab|relaunch|back` — navigate the runtime by page name (from `pages`)
- `eval` — run JS in the **Logic runtime**; `page eval` — run JS in the **page WebView** (the two see different things — JS-contexts table below)
- `page current|list|info` — page-instance status. `page list` includes every live instance (including surface-owned pages), plus every `lxapp.json` route that is not currently open. External URL and URL-callback surfaces are browser tabs, so they appear only under `browser tabs`.
- `page wait` — wait for the lxapp lifecycle to reach `ready`, or for a CSS selector to become attached, detached, visible, hidden, enabled, or editable
- `page query|click|type|fill|press` — cross-platform element automation in the page WebView
- `page scroll` (by `--dx`/`--dy`) / `page scroll-to --css` — scroll the page DOM (nearest scroll container) or bring an element into view
- `page back` — pop the page stack
- `page screenshot` — PNG of one page's WebView

`info` also carries `logic_features`: a read-only snapshot keyed by Logic context
id, with sorted feature names. Runner desktop/handheld preset changes recreate
Logic contexts; rotation and resizing keep the existing feature set.

`lxapp` deliberately has no window selector: a page is the core automation target, independent of how the host embeds it.

**`runner`** — the simulated environment (Runner sessions only): device preset, orientation, appearance:
- `presets` — the device presets the Runner can simulate
- `get` — current preset, orientation, and appearance in one line
- `set [--id <preset>] [--landscape|--portrait] [--appearance system|light|dark] [--capsule on|off]` — partial update: only the given properties change. `--capsule off` hides the simulated host capsule (phone presets draw it by default, as a real host does for a non-home lxapp) — use it when developing a home-style lxapp. Appearance pins the simulated screen's `prefers-color-scheme` at the host level (never injected into page DOM); `system` follows the OS. Example: `lxdev runner set --appearance dark` before dark-mode assertions or screenshots.

Switching between desktop and handheld presets replaces live Logic contexts, including background apps, and waits for retained documents to reload. This resets app Logic state so frozen feature snapshots match the new host.

**`host`** — the selected dev session's host surface. Use this only when the target is the host window rather than an lxapp page:
- `doctor` — report screenshot/input support, coordinate units, and keyboard-modifier reliability
- `windows` — enumerate top-level host windows; the id feeds `--window` on the other `host` commands
- `screenshot` — capture the full host surface, including native controls, overlays, and composited WebViews
- `mouse move|down|up|click|drag|scroll` — raw input in platform window-content units
- `key type|press` — keyboard input to the host window's focused control
- `applink <url>` — inject an inbound link (same entry as OS and push; scans are narrower). Warm `onShow`, `scene === 8003`. Any path works — pass the real product URL. Product host must match `appLinks.hosts`; Runner has none, so any URL is accepted. Returns when accepted, not when navigation finishes.

Mobile reports one host window. Desktop hosts may report several (for example macOS AppUI surfaces); omit `--window` to use the focused/main window. App screenshot JSON always returns the resolved `window_id`, content dimensions, and pixel scale. Mouse coordinates use content pixels on Windows and content points on macOS, so Retina screenshot positions must be divided by the reported scale before feeding them back to `host mouse`.

**`browser`** — the host app's browser tabs (arbitrary web content, including external URL and URL-callback surfaces; Playwright-like):
- `open` / `tabs` / `current` / `activate` / `close` / `reload` / `back` / `forward`
- `eval` / `query` — JS and element inspection in a tab
- `wait` / `wait-url` / `wait-away` — block until a condition holds
- `click` / `type` / `fill` / `press` / `scroll` / `scroll-to`
- `ua show|set|reset` (alias `user-agent`) — browser-session UA; `--reload` refreshes open tabs
- `cookies list|set|delete|clear`
- `screenshot` — PNG of the tab's web content only

**`test`** — bundle and run `@lingxia/test` specs in the session's isolated
host automation runtime. See [Product testing](../lxapp/testing.md) for a
starter spec, fixtures, assertions, cross-app/browser/external HTTP journeys,
test layout, and reports; selection (`--tag`), coverage (`--covers-manifest`),
contract (`--openapi`) and recording (`--record-network`) are covered there.
`lxdev test` attaches to a live session; for CI, `lingxia test` starts a
session, runs it and stops it in one command.
- `--preset NAME` — prepend a named argument list from `lxdev.json`
  (`test.presets`) in the project root; the command line's own flags come
  after it and win. A preset's relative paths are relative to `lxdev.json`,
  not the current directory. `--list-presets` lists them, `--print-args`
  prints the effective arguments with secrets masked. See
  [presets](../lxapp/testing.md#presets).
- `--profile empty|NAME|PATH` / `--profile-save[=pass|always]` — run on an
  isolated data profile ([App data](../lxapp/testing.md#isolated-app-data)).
- `--secrets-file .env.test`, `LXDEV_SECRET_<KEY>` / `LXDEV_ARG_<KEY>` —
  secret and plain args from outside the command line; `<KEY>` keeps its
  case (`LXDEV_SECRET_PASSWORD` is `t.arg('PASSWORD')`). `--print-args` lists
  them with their source, values of secrets masked.
- `--format text|json|jsonl` (`--pretty` for json) — output.
- `--output-root DIR` — the results root: each run gets `DIR/<run-id>/`, and
  `DIR/latest` points at the last run. Default: `test.outputDir` of
  `lxdev.json`, else `test-results/` beside `lxdev.json`, else
  `./test-results`. `--output-dir PATH` writes one run into PATH as is.
- `--last-failed` — rerun what failed in the last run (`latest` in the
  results root), with that run's `--preset` and `--profile`; with nothing
  failed it runs nothing and exits 0.
- `report [DIR|latest] [--failures] [--format json|junit]` — print an
  earlier run's summary, failures and `Rerun:` lines again, no session needed.

While a run is active the session's watcher is paused: a save neither
rebuilds nor reloads the app under a spec; saves made meanwhile rebuild once
when the run ends. The run starts and ends with the app under test running.

`lxdev test --help` groups the flags (Selection, Execution, Inputs, App data,
Contract, Output) and lists examples and exit codes: `0` passed, `1` a spec
failed or timed out / the run was incomplete / it could not run, `2` invalid
arguments, `130` interrupted.

**`scenario`** — put the running app into a named product state from
`tests/scenarios/`, outside a test run (development hosts and the Runner
only); see [Scenarios](../lxapp/scenarios.md):
- `list` — each usable `name` and `name:variant`, with descriptions
- `use <name[:variant]|file[:variant]> [--appid] [--watch]` — validate, then
  install until `clear`, another `use`, or the end of the session; `--watch`
  reinstalls on every save and keeps the last valid version
- `status` — the active `name:variant`, each rule's hits, a hint when no
  request reached it, and the last one cleared (why and when)
- `clear`

**`network`** — the network panel for the running lxapp's Logic `fetch` and
`Rong.SSE` (development hosts and the Runner only):
- `status` — the active scenario, the recording in progress, and recent
  calls with who answered each (`rule k (name:variant)`, a test route,
  `real`, or `companion default`)
- `record start [--match <glob>]` / `record stop --out <file.json> [--name <name>] [--redact <value>]`
  — capture real traffic into a scenario file for `lxdev scenario use`

**`logs [ORIGIN]`** — the session's JSONL log stream: tail or `-f` follow;
filter by a dynamic origin prefix plus `--level`, `--path`, `--grep`, or
`--app <id>`. `lxdev logs --origins` lists the origins present in the selected
session. Text output omits the selected origin and other context fixed by the
session; host sessions include an app id when logs from multiple apps may be
mixed. `--json` keeps the complete event. `-f` exits when the `lingxia dev`
owner process is gone; mobile app closure and hiding to the tray keep it alive. It does not
follow a later session's new log file — start `lxdev logs -f` again.

**`session`** — list live sessions: `#` (the ordinal `--session` accepts), id,
name, target, state, the host's LingXia version and dev protocol, and the
mounted project. Lifecycle stays with the owner CLI: use `lingxia dev stop`
from that session's project rather than stopping it through `lxdev`.

**`desktop`** — local desktop inspection and automation, independent of a dev
session. It covers windows, screenshots, accessibility, pixels, clipboard,
pointer, and keyboard. Destructive actions (closing a window, quitting an app,
killing a process, clearing the clipboard) require `--allow-destructive`.

On Windows, pointer and key input use foreground-only `SendInput`. A `--window`
target is activated first; `--pid` requires exactly one visible window. True
background input is not implemented by the current backend. Without a target,
input goes to the foreground app. Window screenshots remain
occlusion-independent, but separate native popups may require their own capture.

Prefer `browser` or `lxapp page` for WebView content, `host` for the selected
session's native host surface, and `desktop` for arbitrary local OS chrome.
Owner-drawn Win32 controls may not expose accessibility nodes.

## The three JS contexts — don't conflate them

| Command | Runs in | Sees |
|---|---|---|
| `lxapp eval` | Logic runtime | app state, `lx.*` — no DOM |
| `lxapp page eval` | page WebView | rendered DOM, `window` — no app state |
| `browser eval` | a browser tab | that tab's DOM |

Scripts may be an expression or a function body using `return` / `await`. A
native workspace can be opened directly for host verification, for example
`await lx.shell.openDeclared('terminal', { key: 'project-a', as: 'main' })`.
Use a function body and return a serializable assertion value rather than the
surface handle itself. For page navigation, prefer `lxapp nav`; when the behavior
under test is a user interaction, trigger it through `lxapp page click`. The JS
navigation APIs take a configured page name in `{ page }`; route paths are not
accepted.

## Output contract

- Default is human-readable text; `--json` gives compact machine output, `--pretty` indented JSON.
- `lxdev test` picks its output with `--format text|json|jsonl` (`--pretty` indents `json`); its 0.18 `--json` / `--jsonl` still work but are deprecated.
- `eval` / `query` commands always emit JSON (flags only pick compact vs pretty); `eval` prints nothing for `null`.
- Mutating commands (`click`, `type`, `close`, …) print nothing by default. With `--json` they return a non-empty acknowledgement containing the action and resolved target.
- Exit `0` on success. Failures are human-readable on stderr by default; when the command uses `--json` or `--pretty`, stderr contains a structured `{error:{code,message,causes,exit_code}}` envelope.

## Symptom router

| Symptom | Fix |
|---|---|
| `No live dev session found` | Run `lingxia dev` in the project. |
| `Several dev sessions could be meant` | Add `--session <name\|target\|target@dir\|#>` from the printed table. |
| `version skew: … — fix: …` | Run the printed fix (`npm install …` or `lingxia upgrade`); `lingxia doctor --project` shows every version. |
| `eval` returns nothing / wrong scope | Wrong JS context — see the table above. |
| Commands connect but hang | Host app lost its bridge — use `lingxia dev stop` from the project, then start `lingxia dev` again. |
