# `lxdev` — Drive a running dev session

`lxdev` is the external automation client for a running LingXia `lingxia dev` session. It ships from the same workspace as `lingxia` (crate `lingxia-devtools-cli`, binary `lxdev`) and is on `PATH` after a standard install. Use it to inspect / drive the host app's browser tabs, the lxapps inside it, the host windows, and the dev log stream — without rebuilding or restarting the dev session.

This is the **agent-facing** dev surface: every command takes plain flags, prints predictable output (compact JSON with `--json`, or pretty JSON with `--pretty` on the lxapp/sessions/cookies families), and never owns a long-running process (except `lxdev logs -f`, which streams until you stop it). Run it from anywhere inside the project tree — it resolves the session relative to the current working directory.

---

## How it finds the session

`lingxia dev` writes one file per active session under `.lingxia/sessions/<session_id>.json` and removes it on clean exit. `lxdev` scans that directory on every invocation. Each file records:

| Field | Purpose |
|---|---|
| `session_id` | `<unix-ms>-<uuid>`; stable for the whole session |
| `pid` | The `lingxia dev` process — diagnostics only |
| `platform` | `android` / `ios` / `macos` / `harmony` / `lxapp` |
| `started_at` | Unix milliseconds |
| `ws_url` | `ws://host:port` the dev websocket is listening on |
| `log_file` | Path to the session's JSONL log file |

There is no daemon. The `lingxia dev` process that started the session owns its file; nothing else writes there. Every `lxdev` subcommand (except `sessions`) connects to the resolved session's `ws_url`, sends one command over the websocket, prints the result, and exits.

---

## Selecting a session

When `lxdev` is invoked **without** `--session` or `--platform`:

- **One session file in the project** → use it directly, with no liveness probe. (If it turns out to be dead, the websocket connect fails with a clear error — cheaper than probing on the happy path.)
- **Multiple session files** → probe each, drop the stale ones, and:
  - exactly one live → use it,
  - still more than one → **refuse** and list candidates. You must add `--session <id-prefix>` or `--platform <name>`.

Why refuse rather than pick a default? In a project where a human and an agent (Claude / Codex / CI) might both be running dev sessions concurrently, "default to the most recent" silently routes commands to the wrong target. Explicit selection beats a guess.

### Global selector flags

Both are **global** — they go *before* the subcommand:

```bash
lxdev --session 20260520-abc browser tabs
lxdev --platform ios browser tabs
```

| Flag | Match rule |
|---|---|
| `--session <id>` | **prefix** match against `session_id`; a short unique fragment is enough |
| `--platform <name>` | **exact**, case-insensitive (`android`, `ios`, `macos`, `harmony`, `lxapp`) |

A selector that matches **zero** sessions errors out (`No dev session matches the given selector`); a selector that still matches multiple **live** sessions errors and lists them. These are the only two flags that precede the subcommand — everything else (`--json`, `--tab`, `--timeout-ms`, …) is per-subcommand and follows it.

---

## Output conventions

- Human-readable text is the default for most commands (aligned tables for `sessions`, `browser tabs`, `app windows`; a bare id for `open`/`current`).
- `--json` switches a command to compact machine output. `--pretty` (on the `lxapp`, `sessions`, and `cookies list` families) prints indented JSON instead.
- A few read commands **always** emit JSON regardless of flags: `browser eval`, `browser query`, `lxapp eval`, `lxapp page eval`, `lxapp page query`. The `--json`/`--pretty` flag there only toggles compact vs pretty (and `eval` prints just the returned `value`, or nothing when it is `null`).
- Mutating commands (`click`, `type`, `fill`, `press`, `scroll`, `activate`, `close`, …) print nothing on success unless you pass `--json`.
- Exit code is `0` on success, non-zero with a message on stderr otherwise.
- The websocket command timeout is ~120s; commands that take a `--timeout-ms` (waits, evals, navigations) get that budget plus a small buffer on top.

---

## `lxdev sessions`

Inspect / clean up session state. This family does **not** connect to any websocket for the list — it reads the session files and probes liveness directly.

```bash
lxdev sessions              # human-readable table
lxdev sessions --json       # JSON array — each entry adds a `stale` boolean
lxdev sessions prune        # remove session files whose WS server no longer responds
```

The table columns are `ID  STATE  PLATFORM  STARTED  WS  PID`, where `STATE` is `live` or `stale`.

A session is considered **stale** when a 200ms devtools probe fails — `lxdev` opens the websocket, sends a `Hello`, sends an `echo` command, and waits for an `ok` reply; any failure (no TCP connect, no handshake, no echo) marks it stale. Stale entries are normally cleaned up automatically — by the next `lingxia dev` startup (which prunes before checking same-platform conflicts), and by `lxdev` when ambiguity forces it to probe. `lxdev sessions prune` is the manual escape hatch after a hard crash (`kill -9`, IDE force-stop, etc.); it also removes malformed/unreadable session files.

> `--json` applies to the list only; it is ignored when the `prune` subcommand is given.

---

## Subcommand families

`lxdev` exposes **five** families. Four of them (`browser`, `app`, `lxapp`, `logs`) target a single resolved session over the websocket; `sessions` (above) operates on the session files.

| Family | Scope |
|---|---|
| [`browser`](#lxdev-browser-) | the host app's browser tabs (Playwright-like automation, WebView content) |
| [`app`](#lxdev-app-) | the host app as a whole — window enumeration + full-window screenshot |
| [`lxapp`](#lxdev-lxapp-) | lxapps running inside the session, and their pages |
| [`logs`](#lxdev-logs) | the session's JSONL log stream |
| [`sessions`](#lxdev-sessions) | session-file inspection / cleanup |

Across families, common flags share meaning: `--tab <id>` (browser, default `current`), `--app <id>` / `--page <name>` (lxapp, default `current`), `--css <selector>` (the CSS selector for element commands), `--timeout-ms <ms>` (default `5000` for waits/evals/navigations), `-o`/`--output <path>` and `--json` (screenshots).

---

### `lxdev browser …`

Drive the host app's browser tabs — roughly a subset of Playwright's automation surface, scoped to a single LingXia browser instance. Every subcommand below accepts `--tab current|<tab-id>` (default `current`) unless noted, and most accept `--json`.

#### Navigation & tab lifecycle

| Command | Args / flags | Notes |
|---|---|---|
| `open <url>` | `--tab <id>` (reuse/create a stable id), `--json` | Prints the resolved `tab_id` (or JSON). |
| `tabs` | `--json` | Table: `TAB ID  SESSION  TITLE  URL`. |
| `current` | `--json` | Prints the current tab id (or JSON), errors if none. |
| `activate` | `--tab`, `--json` | Bring a tab to front. |
| `close` | `--tab`, `--json` | |
| `reload` | `--tab`, `--json` | |
| `back` | `--tab`, `--json` | History back. |
| `forward` | `--tab`, `--json` | History forward. |

#### Inspect & evaluate

| Command | Args / flags | Notes |
|---|---|---|
| `eval` | `--tab`, `--js <code>` *(required)*, `--wait-navigation`, `--complete`, `--timeout-ms <ms>` | Runs JS in the tab and **always prints the result as JSON**. `--wait-navigation` waits for navigation the script triggers; `--complete` additionally waits for `document.readyState === 'complete'`. |
| `query` | `--tab`, `--css <selector>` *(required)*, `--full`, `--max-text <n>` (default `4096`) | Returns structured info for the first match as JSON. `--full` returns untruncated text/value; otherwise text/value is capped at `--max-text` chars. |

#### Waiting

`wait` requires **exactly one** condition flag; `wait-url` requires **exactly one** of `--url` / `--contains`.

| Command | Args / flags | Notes |
|---|---|---|
| `wait` | `--tab`, one of `--loaded` \| `--exists <sel>` \| `--visible <sel>` \| `--hidden <sel>` \| `--editable <sel>` \| `--js <expr>`, `--timeout-ms`, `--json` | `--loaded` = readyState complete; `--editable` = visible + enabled + editable; `--js` = wait until the expression is truthy. |
| `wait-url` | `--tab`, `--url <exact>` \| `--contains <substr>`, `--timeout-ms`, `--json` | Wait until the tab's URL matches. |
| `wait-navigation` | `--tab`, `--from-url <url>`, `--complete`, `--timeout-ms`, `--json` | Wait until the tab navigates away from `--from-url` (or its current URL); `--complete` also waits for readyState. |

On success, non-JSON output is `ok in <n>ms` (with the resulting URL when relevant).

#### Interact

| Command | Args / flags | Notes |
|---|---|---|
| `click` | `--tab`, `--css <sel>` *(required)*, `--wait-navigation`, `--complete`, `--timeout-ms`, `--json` | Click; optionally wait for the navigation it causes. |
| `type` | `--tab`, `--css <sel>` *(required)*, `--text <str>` *(required)*, `--json` | Append/type text into the element. |
| `fill` | `--tab`, `--css <sel>` *(required)*, `--text <str>` *(required)*, `--json` | Replace the element's current value. |
| `press` | `--tab`, `--key <key>` *(required)*, `--wait-navigation`, `--complete`, `--timeout-ms`, `--json` | Send a key (e.g. `Enter`, `Tab`). |
| `scroll` | `--tab`, `--dx <f>` (default `0`), `--dy <f>` (default `0`), `--json` | Scroll by a delta; negative values allowed. |
| `scroll-to` | `--tab`, `--css <sel>` *(required)*, `--json` | Scroll an element into view. |

#### `browser cookies …`

Manage the tab's WebView cookie store. `--tab` is shared across the subgroup and may appear before or after the subcommand:

```bash
lxdev browser cookies --tab current list
lxdev browser cookies list --tab current        # equivalent
```

| Subcommand | Args / flags |
|---|---|
| `list` | `--visible` (only cookies visible to the tab's current URL), `--pretty` |
| `set` | `--name <n>` *(required)*, `--value <v>` *(required)*, `--url <url>` (defaults to the tab URL), `--domain <d>` (omit for host-only), `--path <p>` (default `/`), `--secure`, `--http-only`, `--expires-unix-ms <ms>`, `--same-site Lax\|Strict\|None`, `--json` |
| `delete` | `--name <n>` *(required)*, `--domain <d>` *(required)*, `--path <p>` (default `/`), `--json` — name/domain/path must match a listed cookie exactly |
| `clear` | `--json` — clears the entire shared WebView cookie store |

#### `browser screenshot`

```bash
lxdev browser screenshot                       # current tab → .lingxia/screenshots/<tab>-<ts>.png
lxdev browser screenshot --tab <id>            # specific tab
lxdev browser screenshot -o ~/shot.png         # custom path
lxdev browser screenshot -o -                  # PNG bytes to stdout
lxdev browser screenshot --json                # JSON envelope: { tab_id, size_bytes, data_base64 }
```

| Flag | Meaning |
|---|---|
| `--tab <id>` | default `current` |
| `-o, --output <path>` | output path; `-` writes raw PNG bytes to stdout. Default `.lingxia/screenshots/<tab>-<ts>.png` under the project root |
| `--json` | print the base64 JSON envelope instead of writing a file |

This is the **WebView-scope** screenshot — only the tab's web content, no host UI overlays. For the entire host window (native sidebars, surfaces, composited overlays) see [`lxdev app screenshot`](#lxdev-app-).

---

### `lxdev app …`

Operate at the **host-app** scope (not lxapp-scope): window enumeration and a full-window screenshot.

| Command | Args / flags | Notes |
|---|---|---|
| `windows` | `--json` | List the host app's top-level windows. Table: `ID  FOCUS  MAIN  VISIBLE  SIZE  TITLE`. |
| `screenshot` | `--window <id>`, `-o`/`--output <path>`, `--json` | Capture the app's window. Default path `.lingxia/screenshots/app-<platform>-<ts>.png`; `-o -` for stdout; `--json` for the `{ format, size_bytes, data_base64 }` envelope. |

There is **no `--scope` flag** — `app screenshot` always captures the app's own window (the full window: native controls, host overlays, and any composited WebViews), via the platform's in-app rendering path. It does **not** capture the whole device display, and it excludes the system status bar / navigation bar / IME. (To verify a layout against an on-screen IME, capture from the device's own tooling, e.g. `adb shell screencap`, separately — `lxdev` does not shell out to host screen-capture tools.)

`--window <id>` selects a specific window (the `id` from `app windows` — `NSWindow.windowNumber` on macOS). It matters on **macOS**, where `lx.surface({kind:'window'})` creates a separate NSWindow; enumerate with `app windows`, then target one. Mobile platforms have a single foreground window and ignore `--window`. When omitted, the platform picks key → main → first window.

#### Per-platform capture mechanism

Both `app screenshot` (window) and `browser screenshot` (WebView content) are produced entirely in-app — no Screen Recording permission, consent prompt, or foreground service. All platforms time out after ~5 seconds and return the PNG bytes base64-encoded in the JSON envelope; `lxdev` decodes and writes the file.

| Platform | WebView screenshot (`browser screenshot`) | App-window screenshot (`app screenshot`) |
|---|---|---|
| macOS | WKWebView `takeSnapshotWithConfiguration:` → PNG | `NSView.cacheDisplayInRect:toBitmapImageRep:` on the window content view |
| iOS | WKWebView `takeSnapshotWithConfiguration:` → PNG | key `UIWindow.layer.renderInContext:` |
| Android | `WebView` `PixelCopy` / `View.draw(Canvas)` → PNG | `PixelCopy.request(window.decorView)` (API 26+) → `View.draw` fallback |
| HarmonyOS | `WebviewController.webPageSnapshot` → PNG | `window.snapshot` → `image.PixelMap` → PNG |

---

### `lxdev lxapp …`

Manage lxapps running inside the session, and inspect/automate their pages. Defaults: `--app current`, and for page commands `--page` defaults to the current page.

> **Dynamic command set.** In a **standalone lxapp project** (`lxapp.json` present, no `lingxia.yaml`) only `info`, `pages`, `page`, `nav`, and `eval` are offered — there's nothing to open/close/uninstall because the project *is* the running lxapp. In a **host-app project** the full set below is available. Run `lxdev lxapp` (or `lxdev lxapp --help`) to see the set for the current project.

#### Top-level lxapp commands

| Command | Args / flags | Notes |
|---|---|---|
| `list` | `--all` (include closed/inactive instances), `--pretty` | List open lxapps as JSON. |
| `current` | `--pretty` | The current lxapp. |
| `info [app]` | positional `app` (default `current`), `--pretty` | Runtime summary. |
| `pages [app]` | positional `app` (default `current`), `--pretty` | Configured pages. |
| `nav <subcommand>` | see below | Navigate the lxapp runtime by **page name**. |
| `eval <script>` | positional `script` *(required)*, `--app`, `--timeout-ms`, `--pretty` | Run JS in the lxapp **logic runtime (AppService)**. `script` is an expression or a function body that uses `return`/`await`. Prints the returned value as JSON (nothing if `null`). Useful for inspecting state or invoking actions without a UI round-trip. |
| `open <appid>` | positional `appid` *(required)*, `--path <p>` (initial page), `--release-type release\|preview\|developer` (default `release`), `--json` | Launch an lxapp; prints its `appid`. |
| `close [app]` | positional `app` (default `current`), `--json` | |
| `restart [app]` | positional `app` (default `current`), `--json` | |
| `uninstall [app]` | positional `app` (default `current`), `--json` | Uninstall the lxapp **and its data**. |

#### `lxdev lxapp nav …`

Drive lxapp runtime navigation. The `<page>` positional argument is the page `name` declared in `lxapp.json`, not the runtime page path. Use `lxdev lxapp page list` when you need to discover page names.

| Subcommand | Args / flags | Notes |
|---|---|---|
| `to <page>` | `--app`, repeatable `--query KEY=VALUE`, `--json` | Push a configured page onto the page stack. |
| `redirect <page>` | `--app`, repeatable `--query KEY=VALUE`, `--json` | Replace the current page. Tab-bar pages are rejected. |
| `switch-tab <page>` | `--app`, repeatable `--query KEY=VALUE`, `--json` | Switch to a configured tab page; use this for the `lx.switchTab` route. |
| `relaunch <page>` | `--app`, repeatable `--query KEY=VALUE`, `--json` | Clear the stack and relaunch at the page. |
| `back` | `--delta <n>` (default `1`), `--app`, `--json` | Navigate back in the lxapp page stack. |

```bash
lxdev lxapp nav to details --query id=42
lxdev lxapp nav switch-tab profile --json
lxdev lxapp nav back --delta 2
```

#### `lxdev lxapp page …`

Inspect and automate the **page WebView** of an lxapp. All page commands take `--app` (default `current`) and `--page <name>` (default current page) unless the table notes otherwise.

| Subcommand | Args / flags | Notes |
|---|---|---|
| `current` | `--app`, `--pretty` | The current page. |
| `list` | `--app`, `--pretty` | Configured pages. |
| `info` | `--page`, `--app`, `--pretty` | Page status. |
| `eval <script>` | positional `script` *(required)*, `--page`, `--app`, `--timeout-ms`, `--pretty` | Run JS in the **page WebView** (vs `lxapp eval`, which targets the logic runtime). Prints the returned value. |
| `query` | `--css <sel>` *(required)*, `--all`, `--index <n>`, `--full`, `--max-text <n>` (default `4096`), `--page`, `--app`, `--pretty` | Element info as JSON. `--all` returns every match; `--index` returns the nth (mutually exclusive with `--all`). |
| `click` | `--css <sel>` *(required)*, `--index <n>`, `--page`, `--app`, `--json` | Click; `--index` selects the nth match. |
| `type` | `--css <sel>` *(required)*, `--text <str>` *(required)*, `--index <n>`, `--page`, `--app`, `--json` | |
| `fill` | `--css <sel>` *(required)*, `--text <str>` *(required)*, `--index <n>`, `--page`, `--app`, `--json` | Replace the element's value. |
| `press` | `--key <key>` *(required)*, `--page`, `--app`, `--json` | |
| `back` | `--delta <n>` (default `1`), `--app`, `--json` | Navigate back in the lxapp page stack. |
| `screenshot` | `--page` (default `current`), `--app`, `-o`/`--output`, `--json` | PNG of the page WebView. Default path `.lingxia/screenshots/<app>-<page>-<ts>.png`. |

---

### `lxdev logs`

Tail and filter the session's JSONL log stream (read straight from `log_file` — no websocket).

```bash
lxdev logs                                    # last 200 matching lines
lxdev logs -f                                 # follow (Ctrl+C to exit)
lxdev logs -f --limit 0                        # follow only — skip the backlog
lxdev logs -f --level error                   # filter by level
lxdev logs --source webview --grep CSP        # filter by source + text
lxdev logs --path pages/home --json           # JSONL output
lxdev logs --pretty                            # colorized for a TTY
```

| Flag | Meaning |
|---|---|
| `--grep <text>` | keep entries whose message / page path / appid contains `<text>` (case-insensitive) |
| `--level <level>` | one of `verbose`, `debug`, `info`, `warn`, `error` |
| `--source <source>` | one of `native`, `webview`, `logic` (aliases: `web_view_console` → `webview`, `lx_app_service_console` → `logic`). The flag itself also aliases to `--tag`. |
| `--path <text>` | keep entries whose page path contains `<text>` |
| `--limit <n>` | most-recent N matching backlog entries (default `200`; `0` with `--follow` skips the backlog entirely) |
| `-f, --follow` | keep running and stream new matching entries as they're appended (survives log rotation/truncation) |
| `--json` | emit matching entries as JSONL (mutually exclusive with `--pretty`) |
| `--pretty` | colorize by level for terminal viewing (not for machine consumption) |

Default line format: `HH:MM:SS.mmm  LEVEL    source                 [path]  message`.

---

## Concurrency rules

The whole design is built around "a human and one or more agents may be doing dev in parallel." The rules are:

1. **`lingxia dev` refuses to start a second same-platform session** unless `--parallel` is passed. This is the primary defense — if you never trip this, you'll never have an ambiguous `lxdev` call.
2. **`lxdev` refuses to act when ambiguity exists.** It will not silently pick a session for you; it prints the candidates and asks for a `--session` / `--platform`.
3. **Stale sessions don't count.** Pruning happens in three places (`lingxia dev` startup, `lxdev sessions prune`, and `lxdev` itself when ambiguity forces a probe), so a hard-crashed `lingxia dev` won't keep blocking future runs.
4. **`--parallel` is per-launch.** It does not persist; each subsequent `lingxia dev` for that platform must opt in again.

---

## Typical agent flow

```bash
# Discover what's available
lxdev sessions

# Single session, no ambiguity → zero-arg invocation
lxdev browser open https://example.com
lxdev browser eval --js "document.title"
lxdev browser wait --visible "#app .ready"
lxdev browser click --css "#login"
lxdev logs -f --level warn

# Inspect an lxapp without a UI round-trip
lxdev lxapp list
lxdev lxapp eval "return appService.getState().user"
lxdev lxapp page query --css ".cart-item" --all

# Visual check — what does the user see
lxdev app windows                  # all host windows
lxdev app screenshot               # the focused window (full app window)
lxdev browser screenshot           # just the web content of the current tab

# Two sessions running (e.g. ios + harmony) → disambiguate by platform
lxdev --platform ios browser tabs
lxdev --platform harmony logs --grep BridgeError

# After a crash leaves residue
lxdev sessions prune
```

---

## Symptom router

| Symptom | Fix |
|---|---|
| `No active dev session found` | Run `lingxia dev` in this project. |
| `No dev session matches the given selector` | Check `lxdev sessions`; fix the `--session` prefix / `--platform` name. |
| `Multiple active dev sessions match` | Add `--session <prefix>` or `--platform <name>`. |
| `All matching dev sessions are unreachable` | Run `lxdev sessions prune`, then start a fresh `lingxia dev`. |
| `Existing <platform> dev session is already running` (from `lingxia dev`) | Stop the other one, or pass `--parallel` if you genuinely want two. |
| `pass exactly one wait condition` / `pass exactly one of --url or --contains` | A `wait` / `wait-url` invocation supplied zero or several conditions — give exactly one. |
| `lxdev` connects but commands hang | The host app likely lost its bridge — restart `lingxia dev`. |

For the underlying design rationale (why no daemon, why this specific file layout), see the project draft `docs/draft/dev-session-multi.md` in the LingXia repo.
