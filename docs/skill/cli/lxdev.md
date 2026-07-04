# `lxdev` — Drive a running dev session

`lxdev` inspects and drives a live `lingxia dev` session — the host app's browser tabs, the lxapps inside it, the host window, and the log stream. It is a session client: it connects to an existing dev websocket, sends one command, prints the result, and exits (except `logs -f`). It does not start new platform sessions; `lingxia dev` owns launch, install, process lifetime, and background mode.

This file says **what `lxdev` can do**. For flags and defaults, `lxdev <family> <cmd> --help` is exhaustive and always matches the installed version — the doc does not duplicate it. The command set is dynamic per project type, so `--help` is also the only reliable list for the project you're in.

## Session selection

`lingxia dev` writes `.lingxia/sessions/<id>.json` per session; `lxdev` scans it on every run. One live session → used automatically. Several → it **refuses to guess** and lists candidates; pick one with the global selector, which goes **before** the subcommand:

```bash
lxdev --session ios ...          # platform name, or a session-id prefix
```

Stale files from crashed sessions are pruned automatically (or via `lxdev sessions prune`). `lingxia dev` refuses a second same-platform session unless launched with `--parallel`.

## Capabilities

**`lxapp`** — the lxapps in the session. Every command targets the **current** lxapp by default (`--app` to pick another); page commands likewise default to the **current page** (`--page` to pick):
- `list` / `current` / `info` / `pages` — what's running, and the configured pages
- `open` / `close` / `restart` / `uninstall` — lifecycle
- `rebuild` — rebuild the lxapp front-end bundle through the running session
- `restart --build` — rebuild, then restart the lxapp runtime
- `nav to|redirect|switch-tab|relaunch|back` — navigate the runtime by page name (from `pages`)
- `eval` — run JS in the **Logic runtime** (AppService: `lx.*`, `Page({})` state — no DOM)
- `page current|list|info` — page stack status
- `page eval` — run JS in the **page WebView** (rendered DOM — no AppService state)
- `page query|click|type|fill|press|scroll|scroll-into-view` — element-level automation in the page WebView
- `page back` — pop the page stack
- `page screenshot` — PNG of one page's WebView

**`browser`** — the host app's browser tabs (arbitrary web content, Playwright-like):
- `open` / `tabs` / `current` / `activate` / `close` / `reload` / `back` / `forward`
- `eval` / `query` — JS and element inspection in a tab
- `wait` / `wait-url` / `wait-away` — block until a condition holds
- `click` / `type` / `fill` / `press` / `scroll` / `scroll-into-view`
- `cookies list|set|delete|clear`
- `screenshot` — PNG of the tab's web content only

**`app`** — the host app as a whole:
- `windows` — enumerate top-level windows (macOS surfaces make separate windows; target with `--window`)
- `screenshot` — the full host window: native controls, overlays, composited WebViews
- `mouse move|down|up|click|drag|scroll` — raw input at window coordinates
- `key text|press` — keyboard input to the focused control

**`logs`** — the session's JSONL log stream: tail or `-f` follow, filter by level, source (native / webview / logic), page path, or text.

**`sessions`** — list, probe liveness, prune stale session files, or request that the owning `lingxia dev` process stop (`sessions stop`). Force-kill remains on `lingxia dev stop --force`, because `lingxia` owns the platform process lifecycle.

## The three JS contexts — don't conflate them

| Command | Runs in | Sees |
|---|---|---|
| `lxapp eval` | Logic runtime | app state, `lx.*` — no DOM |
| `lxapp page eval` | page WebView | rendered DOM, `window` — no app state |
| `browser eval` | a browser tab | that tab's DOM |

Scripts may be an expression or a function body using `return` / `await`. Surface-opening calls (`lx.surface.*`, `navigateTo`) deadlock from `lxapp eval` — trigger those via a real page interaction (`lxapp page click`) instead. To navigate, prefer `lxapp nav`; the JS APIs take `{ page }` or `{ path }`, never `url`.

## Output contract

- Default is human-readable text; `--json` gives compact machine output, `--pretty` indented JSON.
- `eval` / `query` commands always emit JSON (flags only pick compact vs pretty); `eval` prints nothing for `null`.
- Mutating commands (`click`, `type`, `close`, …) print nothing on success — check the exit code, not stdout.
- Exit `0` on success; non-zero with a message on stderr.

## Symptom router

| Symptom | Fix |
|---|---|
| `No active dev session found` | Run `lingxia dev` in this project. |
| `Multiple active dev sessions match` | Add `--session <id-prefix\|platform>`. |
| `All matching dev sessions are unreachable` | `lxdev sessions prune`, then restart `lingxia dev`. |
| `eval` returns nothing / wrong scope | Wrong JS context — see the table above. |
| Commands connect but hang | Host app lost its bridge — use `lxdev sessions stop` or `lingxia dev stop`, then start `lingxia dev` again. |
