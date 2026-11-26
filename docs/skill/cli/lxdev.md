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
them (`#`, id, name, target, state, host version, start time, content, and the session's build directory; `--json` for scripts).
The host version ends with its View wire (`view 1/231`): GPU pages need its ABI and at most its newest field.
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

- `list` / `current` / `info` / `pages` — what is running, and configured pages;
  `doctor` — what automation the session supports
- `open` / `close` / `restart` / `uninstall` — lifecycle (`restart` relaunches
  without rebuilding)
- `nav go <page> [--param K=V] [--replace]` / `nav go --tab <tab>` / `nav reset <page>` / `nav back` — navigate by page name
- `eval` — JS in the Logic runtime; `page eval` — JS in the page WebView
  ([JS contexts](#js-contexts))
- `page current|list|info` — page instances, including surface pages and
  unopened routes
- `page wait [SELECTOR]` — for `ready`, or for the selector to be attached,
  detached, visible, hidden, enabled, or editable
- `page query|click|scroll-to SELECTOR`, `page type|fill SELECTOR TEXT`,
  `page press KEY [SELECTOR]`, `page scroll|back`
- `page query|click|type|press|screenshot --feedback` — the toast and dialogs over the app's window, in place of `--page`
- `page tree`, `page find ROLE [--name NAME]` — the accessibility tree (role, name, test id,
  state, bounds); every page command reaches GPU pages too
- `page frame` — a GPU page's presentation counters and frame pacing; `failure` and `stalled`
  say when it stopped rendering. `page hud on|off` draws the counters over the page
- `page screenshot` — PNG of one page. On a Runner a GPU page's is the simulated device's pixels,
  viewport × the preset's pixel ratio (iPhone 15 Pro: 1179 × 2556), on any Mac screen

## `lxdev runner`

The simulated device and display language (Runner sessions only).

- `presets` — the device presets: viewport in points and pixel ratio
- `get` — current preset, orientation, appearance, effective language, text
  scale, contrast
- `set` — partial update of preset, orientation, `--appearance
  system|light|dark`, and `--language TAG|auto`. The capsule follows the host:
  none on its home lxapp, always on every other.
- `set --text-scale 1.3` / `--contrast standard|high` — the simulated system text size (0.5 to 3)
  and contrast, which GPU pages read as `env`'s.
- `set --preset phone|fold|trifold|pad|desktop [--posture outer|open|dual|tabletop]` — pick a
  device by form factor instead of `--id`; a posture the device lacks is an error. Folding
  devices are on the macOS and Windows Runners. `lxdev device …` is the same command.

`--language` shadows the product's display language until the session ends and
is not saved.

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
  — on a GPU page `key` goes to the focused element without raising the window;
  only Shift chords exist there, and `--window` takes the native path
- `applink <url>` — inject an inbound link ([App links](../app/applinks.md#testing))

Screenshot JSON returns `window_id`, content size, and pixel scale. Mouse
coordinates are content pixels on Windows and content points on macOS; divide
Retina screenshot positions by the scale.

## `lxdev browser`

The host app's browser tabs, including URL surfaces:

- `open` / `tabs` / `current` / `activate` / `close` / `reload` / `back` / `forward`
- `snapshot` — roles, names, states, and a ref (`e12`) per element; `--since-last` prints only changes
- `click` / `hover` / `drag` / `type` / `fill` / `press` / `scroll-to` / `select-option` / `check` / `uncheck` / `set-input-files` — take `--ref` or `--css`
- `text` (main content as Markdown), `console`, `screenshot [--full-page | --ref | --annotate]`
- `eval` / `query`, `wait` / `wait-url` / `wait-navigation`, `scroll`, `ua`, `cookies`

```bash
lxdev browser snapshot
lxdev browser fill --ref e3 --text ada@example.com
lxdev browser click --ref e9 --snapshot diff   # prints what the click changed
```

Actions wait until the element is visible, enabled, stable and uncovered
(`--timeout-ms`). Failures carry a `code`: `not_found`, `stale_ref`,
`not_visible`, `disabled`, `covered` (names the cover's ref), `not_editable`,
`timeout`. A ref dies with its document: snapshot again after navigating.
On macOS, `hover` fires the events but cannot apply `:hover` styles.

## `lxdev mock` and `lxdev network`

`lxdev mock` sets who answers the running app's calls (development hosts and the
Runner): [Mocks → Commands](../lxapp/mock.md#commands). Its changes affect the whole
session; `use --app` picks which lxapp's HTTP calls the session's one scenario answers.

```bash
lxdev network status              # who answered each call; what reached the network
lxdev network capture start       # then: capture stop r.json | r.har
```

`capture stop` writes HAR to a `.har` path and the request JSON to any other.
Read the JSON: bodies are parsed and secrets redacted. Page WebViews do not
reach the network (`connect-src 'none'`).

## `lxdev test`

Runs the project's `tests/*.test.ts` in the session (`lingxia dev` starts it).
`lxdev test list` lists tests, `presets` lists the `lxdev.json` presets, `args` prints
the effective arguments, `cancel`
releases a run left behind, `report` reprints one. Writing tests, presets,
profiles, secrets, and exit codes: [Testing](../lxapp/testing.md).

## `lxdev logs`

The session's log stream: `-f` follows, `--level`, `--app` and `--path` filter, `--jsonl` for
scripts. An app filter matches the recorded app id, not the current live app.
`--grep` matches text in any case; add `-E` to read it as a regular expression (`--grep 'ready|failed' -E`).
Following ends with this session; it never attaches to a later session.

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

Use each command's `--help` to choose its machine-readable output. Eval and
query return JSON; eval emits nothing for `null`. Structured failures retain
the host error in `error.domain`; inspect it when CLI classification alone
is insufficient. A nonzero exit can mean partial completion: read the result
before retrying a mutation.

## Symptoms

| Symptom | Fix |
|---|---|
| `No running app session for this project` | Start one with `lingxia dev` in the project. |
| `Several dev sessions could be meant` | Add `--session <name\|target\|target@dir\|#>` from the printed table. |
| `minRuntime` / `host SDK` / `dev protocol` outside what this lxdev speaks | Run the printed fix; if the check is wrong, `LINGXIA_SKIP_COMPAT=1`. See [compatibility](./lingxia.md#lingxia-dev). |
| `eval` returns nothing / wrong scope | Wrong JS context — see [JS contexts](#js-contexts). |
| Commands connect but hang | `lingxia dev stop` from the project, then `lingxia dev` again. |
| `lxdev desktop window focus` asks for Accessibility again after `lxdev` was rebuilt | For the session's own window use `lxdev host focus`, which needs no grant. |
