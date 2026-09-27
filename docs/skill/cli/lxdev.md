# `lxdev` CLI

`lxdev` works on a session started by [`lingxia dev`](./lingxia.md#lingxia-dev):
it connects, runs one command, prints the result, and exits (except
`logs -f`). It never starts or stops a session.

`lxdev <family> <cmd> --help` is the source of truth for flags and defaults,
and the only reliable command list for the project you are in.

## Session selection

Start a session for automation with `lingxia dev --background` and end it with
`lingxia dev stop`. A session lives as long as its `lingxia dev` process;
closing the Runner or quitting a desktop host ends it, while hiding a host to
the tray or closing a mobile app does not.

`lxdev` works from any directory. Without a selector it uses the live session
whose project contains the current directory; inside a project it never falls
back to another project's session. Outside any project it uses the only live
session; otherwise it refuses to guess and prints the candidates. `lxdev session` lists
them (`#`, id, name, target, state, versions, project; `--json` for scripts).
Pick one with `--session` (before the subcommand) or `LXDEV_SESSION`:

```bash
lingxia dev --background --name demo   # name a session when you start it
lxdev --session demo ...         # its name
lxdev --session ios ...          # target name, when unique
lxdev --session macos@my-app ... # target in a project (dir name or path)
lxdev --session 2 ...            # the # column of `lxdev session`
lxdev --session a1b2 ...         # session-id prefix
```

Only sessions of the same user on the same machine are reachable.

## `lxdev lxapp`

The lxapps and pages in the session. Commands target the current lxapp
(`--app` for another) and the current page (`--page` takes a page name or an
`instance_id` from `page current|list|info`).

- `list` / `current` / `info` / `pages` — what is running, and configured pages
- `open` / `close` / `restart` / `uninstall` — lifecycle (`restart` relaunches
  without rebuilding)
- `nav to|redirect|switch-tab|relaunch|back` — navigate by page name
- `eval` — JS in the Logic runtime; `page eval` — JS in the page WebView
  ([JS contexts](#js-contexts))
- `page current|list|info` — page instances, including surface pages and
  unopened routes
- `page wait` — for `ready`, or for a selector to be attached, detached,
  visible, hidden, enabled, or editable
- `page query|click|type|fill|press|scroll|scroll-to|back`
- `page screenshot` — PNG of one page's WebView

## `lxdev runner`

The simulated device (Runner sessions only).

- `presets` — the device presets
- `get` — current preset, orientation, appearance
- `set` — partial update of preset, orientation, `--appearance
  system|light|dark`, and `--capsule on|off` (off for a home-style lxapp)

Switching between desktop and handheld presets restarts Logic, so app state
resets.

## `lxdev host`

The selected session's native host window, when the target is not a page.

- `doctor` — screenshot/input support and coordinate units
- `windows` — host windows; the id feeds `--window`
- `focus` — bring the host window to the front and make it key; the app
  raises its own window, so no Accessibility grant is needed
- `screenshot` — the full host surface, native controls included
- `mouse move|down|up|click|drag|scroll`, `key type|press`
- `applink <url>` — inject an inbound link ([App links](../app/applinks.md#testing))

Screenshot JSON returns `window_id`, content size, and pixel scale. Mouse
coordinates are content pixels on Windows and content points on macOS; divide
Retina screenshot positions by the scale.

## `lxdev browser`

The host app's browser tabs, including URL surfaces:

- `open` / `tabs` / `current` / `activate` / `close` / `reload` / `back` / `forward`
- `eval` / `query`
- `wait` / `wait-url` / `wait-away`
- `click` / `type` / `fill` / `press` / `scroll` / `scroll-to`
- `ua show|set|reset`, `cookies list|set|delete|clear`
- `screenshot` — the tab's web content

## `lxdev mock` and `lxdev network`

Who answers the running app's calls — `mocks/` handlers, the real backend,
or a scenario state on top — and what each call got (development hosts and
the Runner):

```bash
lxdev mock                        # the selection, its source, hits, errors
lxdev mock all | none [TARGET…]   # live, until lxdev mock reset
lxdev mock use checkout:empty-cart
lxdev network status              # who answered each call
```

Everything else: [Mocks](../lxapp/mock.md).

## `lxdev test`

Runs `@lingxia/test` specs against the session. Writing, running, presets,
profiles, secrets, reports, and exit codes: [Testing](../lxapp/testing.md).

## `lxdev logs`

The session's JSONL log stream: tail, or `-f` to follow. Filter by origin
prefix, `--level`, `--path`, `--grep`, or `--app <id>`; `--origins` lists the
origins; `--json` keeps whole events. `-f` exits when the session ends and
does not follow a later session.

## `lxdev desktop`

Local desktop inspection and input, independent of a dev session: windows,
screenshots, accessibility, pixels, clipboard, pointer, keyboard. Destructive
actions need `--allow-destructive`. On Windows, input goes to the foreground
window (a `--window` target is activated first).

Prefer `browser` or `lxapp page` for web content, `host` for the session's
native host, and `desktop` for other OS chrome.

## JS contexts

| Command | Runs in | Sees |
|---|---|---|
| `lxapp eval` | Logic runtime | app state, `lx.*` — no DOM |
| `lxapp page eval` | page WebView | rendered DOM, `window` — no app state |
| `browser eval` | a browser tab | that tab's DOM |

A script is an expression or a function body using `return` / `await`; return
a serializable value. Navigate with `lxapp nav`, and trigger user behaviour
with `lxapp page click`.

## Output

- Text by default; `--json` compact, `--pretty` indented.
- `eval` / `query` always emit JSON; `eval` prints nothing for `null`.
- Mutating commands print nothing unless `--json`, which returns an
  acknowledgement with the resolved target.
- Exit `0` on success. With `--json`/`--pretty`, stderr carries
  `{error:{code,message,causes,exit_code}}`.

## Symptoms

| Symptom | Fix |
|---|---|
| `No running app session for this project` | Start one with `lingxia dev` in the project. |
| `Several dev sessions could be meant` | Add `--session <name\|target\|target@dir\|#>` from the printed table. |
| `version skew: … — fix: …` | Run the printed fix; see [version skew](./lingxia.md#lingxia-dev). |
| `eval` returns nothing / wrong scope | Wrong JS context — see [JS contexts](#js-contexts). |
| Commands connect but hang | `lingxia dev stop` from the project, then `lingxia dev` again. |
| `lxdev desktop window focus` asks for Accessibility again after `lxdev` was rebuilt | For the session's own window use `lxdev host focus`, which needs no grant. |
