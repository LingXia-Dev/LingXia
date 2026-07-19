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

Each `lingxia dev` session registers with a per-user local broker and stays registered for exactly as long as its process lives; `lxdev` queries the broker, so it works from **any directory** — the session may be one you started or one already running. One live session → used automatically. Several → it **refuses to guess** and prints the candidates; pick one with the global selector (before the subcommand) or the `LXDEV_SESSION` env var:

```bash
lxdev --session a1b2 ...         # session-id prefix
lxdev --session ios ...          # target name, when unique
```

Crashed sessions disappear from the broker automatically — there is nothing to prune. Re-running `lingxia dev` for the same target in a project stops the previous session and takes over; different targets run side by side.

`lxdev` intentionally controls sessions registered by `lingxia dev` for the
same user on the same machine. For a remote development machine, run both
commands there through SSH or the machine's CI/device-lab agent; the dev
websocket is not a remote machine-management API.

## Capabilities

**`lxapp`** — the lxapps and runtime pages in the session. Every command targets the **current** lxapp by default (`--app` to pick another); page commands likewise default to the **current page** (`--page` accepts a configured page name or the stable `instance_id` returned by `page current|list|info`):
- `list` / `current` / `info` / `pages` — what's running, and the configured pages
- `open` / `close` / `restart` / `uninstall` — lifecycle (`restart` relaunches the runtime without rebuilding)
- `reload` — rebuild the lxapp front-end bundle through the running session, then reload the running lxapp so the new bundle is live (covers Web, Logic, and `lxapp.json` changes); `--build-only` skips the runtime reload
- `device list|get|set` — inspect or switch a Runner preset without the native selector UI; use `device set --id <preset> [--landscape|--portrait]`
- `nav to|redirect|switch-tab|relaunch|back` — navigate the runtime by page name (from `pages`)
- `eval` — run JS in the **Logic runtime**; `page eval` — run JS in the **page WebView** (the two see different things — JS-contexts table below)
- `page current|list|info` — page-instance status. `page list` includes every live instance (including surface-owned pages), plus every `lxapp.json` route that is not currently open. External URL and URL-callback surfaces are browser tabs, so they appear only under `browser tabs`.
- `page wait` — wait for the lxapp lifecycle to reach `ready`, or for a CSS selector to become attached, detached, visible, hidden, enabled, or editable
- `page query|click|type|fill|press` — cross-platform element automation in the page WebView
- `page scroll` (by `--dx`/`--dy`) / `page scroll-to --css` — scroll the page DOM (nearest scroll container) or bring an element into view
- `page back` — pop the page stack
- `page screenshot` — PNG of one page's WebView

`lxapp` deliberately has no window selector: a page is the core automation target, independent of how the host embeds it.

**`app`** — the selected dev session's host surface. Use this only when the target is the host window rather than an lxapp page:
- `doctor` — report screenshot/input support, coordinate units, and keyboard-modifier reliability
- `windows` — enumerate top-level host windows; the id feeds `--window` on the other `app` commands
- `screenshot` — capture the full host surface, including native controls, overlays, and composited WebViews
- `mouse move|down|up|click|drag|scroll` — raw input in platform window-content units
- `key type|press` — keyboard input to the host window's focused control

Mobile reports one host window. Desktop hosts may report several (for example macOS AppUI surfaces); omit `--window` to use the focused/main window. App screenshot JSON always returns the resolved `window_id`, content dimensions, and pixel scale. Mouse coordinates use content pixels on Windows and content points on macOS, so Retina screenshot positions must be divided by the reported scale before feeding them back to `app mouse`.

**`browser`** — the host app's browser tabs (arbitrary web content, including external URL and URL-callback surfaces; Playwright-like):
- `open` / `tabs` / `current` / `activate` / `close` / `reload` / `back` / `forward`
- `eval` / `query` — JS and element inspection in a tab
- `wait` / `wait-url` / `wait-away` — block until a condition holds
- `click` / `type` / `fill` / `press` / `scroll` / `scroll-to`
- `cookies list|set|delete|clear`
- `screenshot` — PNG of the tab's web content only

**`test`** — run bundled JavaScript/TypeScript cases in the session (`lxdev test tests/flows/checkout.test.ts`). Install `@rongjs/test`, import its `describe` / `test` / hooks / `expect`, and keep contracts in `tests/api/`, page behavior in `tests/pages/`, and journeys in `tests/flows/`.

- `const auto = lx.automation()` — select the current app with `auto.lxapp()` or a specific running app with `auto.lxapp(appid)`; the returned driver's `page`, `nav`, and `eval` surfaces all target that app. Cross-app lifecycle operations live on `auto.lxapps`.
- `test.args` — strings from repeatable `--arg key=value`; `test.attach(name, { mimeType, base64 })` — save an artifact, downloaded into `--output-dir` (default `test-results/lxdev/<run-id>`).
- `console`, timers, and host-device `fetch`

There is no appid-scoped `lx.*`, filesystem, environment, or dynamic `import()`. Cases run sequentially; async hooks/cases are awaited. `--timeout` bounds the run, Ctrl-C cancels it, `--json` emits one final report, and failures map back to source files. `lingxia dev` and the Runner include the required runtime.

**`logs`** — the session's JSONL log stream: tail or `-f` follow; filter by `--level`, `--source` (`native` host, your app's `lxview`/`lxlogic`, a `browser` tab, or `automation` output), `--path`, `--grep`, `--app <id>`; `--wide` prefixes each line with its app id.

**`session`** — list live sessions (id, target, project path). Lifecycle stays
with the owner CLI: use `lingxia dev stop` from that session's project rather
than stopping it through `lxdev`.

## The three JS contexts — don't conflate them

| Command | Runs in | Sees |
|---|---|---|
| `lxapp eval` | Logic runtime | app state, `lx.*` — no DOM |
| `lxapp page eval` | page WebView | rendered DOM, `window` — no app state |
| `browser eval` | a browser tab | that tab's DOM |

Scripts may be an expression or a function body using `return` / `await`. Surface-opening calls (`lx.surface.*`, `navigateTo`) deadlock from `lxapp eval` — trigger those via a real page interaction (`lxapp page click`) instead. To navigate, prefer `lxapp nav`; the JS APIs take `{ page }` or `{ path }`, never `url`.

**`desktop`** — local desktop inspection and automation, independent of a dev
session. It covers windows, screenshots, accessibility, pixels, clipboard,
pointer, and keyboard. Writes require `--allow-control`; destructive actions
also require `--allow-destructive`.

On Windows, pointer and key input use foreground-only `SendInput`. A `--window`
target is activated first; `--pid` requires exactly one visible window. True
background input is not implemented by the current backend. Without a target,
input goes to the foreground app. Window screenshots remain
occlusion-independent, but separate native popups may require their own capture.

Prefer `browser` or `lxapp page` for WebView content, `app` for the selected
session's native host surface, and `desktop` for arbitrary local OS chrome.
Owner-drawn Win32 controls may not expose accessibility nodes.

## Output contract

- Default is human-readable text; `--json` gives compact machine output, `--pretty` indented JSON.
- `eval` / `query` commands always emit JSON (flags only pick compact vs pretty); `eval` prints nothing for `null`.
- Mutating commands (`click`, `type`, `close`, …) print nothing by default. With `--json` they return a non-empty acknowledgement containing the action and resolved target.
- Exit `0` on success. Failures are human-readable on stderr by default; when the command uses `--json` or `--pretty`, stderr contains a structured `{error:{code,message,causes,exit_code}}` envelope.

## Symptom router

| Symptom | Fix |
|---|---|
| `No live dev session found` | Run `lingxia dev` in the project. |
| `Multiple LingXia dev sessions are live` | Add `--session <id-prefix\|target>`. |
| `eval` returns nothing / wrong scope | Wrong JS context — see the table above. |
| Commands connect but hang | Host app lost its bridge — use `lingxia dev stop` from the project, then start `lingxia dev` again. |
