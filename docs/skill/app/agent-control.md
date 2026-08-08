# Driving a Shipped Product

Use this reference when a host app lets a local command line or agent drive
the product. The product decides which surfaces exist; the user decides whether
the local interface is enabled.

## Declare the surfaces

```yaml
capabilities:
  appUse: true       # this product's own windows
  computerUse: true  # the whole machine
  browserUse: true   # the in-app browser (requires `browser`)
```

These capabilities are available on macOS and Windows and are enforced by the
running product:

- `computerUse` implies `appUse` because machine-wide control can already reach
  the product's windows.
- `browserUse` is independent; a browser-only product need not expose its
  window chrome.
- A refused namespace is final. An agent must not route around it.

## User control and command discovery

The endpoint is local to the user who launched the app, answers only while the
app is running, and stays closed until the user enables it.

The product executable is also its command. Enabling the interface writes a
small launcher into the product's state directory. `<product> control enable`
prints that path and the shell-profile line needed to put it on `PATH`.

The control commands report and change real state:

- `control status` distinguishes a live listener from a setting that takes
  effect at the next start.
- `control disable` stops a live listener when present, persists the disabled
  state, and removes its socket and launcher.

Use `<product> --help` and leaf-command `--help` for exact syntax. Prefer
`--json` when the leaf offers it. Failures use stable exits: 2 usage, 3 not
found, 4 ambiguous, 5 timeout, 6 permission or refusal, 7 unsupported,
8 unavailable, 9 stale handle, and 10 failure after target resolution.

`--allow-control` is an acknowledgement, not a permission grant. Add it only
when the user's current request authorizes the state change. Add
`--allow-destructive` only when that request explicitly authorizes the
destructive effect, such as closing a window, quitting an app, or clearing the
clipboard or cookies.

## Generate an agent skill

Run `show` and `install` while the product is open so it can report its declared
capabilities. `remove` only touches the installed files and also works offline.

```text
<product> skills show
<product> skills install --agent claude   # or --agent codex
<product> skills remove --agent claude
```

The generated skill contains only agent-facing entry points allowed by that
running build. It excludes human administration (`control` and `skills`) and
uses `--help` for leaf syntax instead of copying the full CLI tree. If the
product cannot be reached, `show` and `install` fail rather than writing a skill
that guesses its capabilities.

Installation writes into another agent's configuration directory, so it is an
explicit user command rather than a side effect of enabling automation.

## Desktop permissions and viewer

On macOS, `computerUse` needs Accessibility and Screen Recording. Commands
execute inside the product, so macOS attributes both grants to the installed
product rather than the terminal that invoked it.

Before machine-wide work, the agent should run:

```text
<product> computer permissions --json
```

If a grant is missing, the agent asks the user to grant it and stops retrying.
Screen capture without Screen Recording can otherwise look like an empty
desktop. Signed builds retain grants across matching updates; unsigned rebuilt
apps may prompt again.

The first mutating `computerUse` command on macOS or Windows opens a small
viewer showing what is being driven and the last acted point. It follows the
work, avoids the pointer target, hides after roughly twelve seconds of
inactivity, and returns on the next mutation. Read-only commands do not open it.

Each product process owns at most one viewer. Separate running products do not
coordinate a machine-wide viewer; the viewer always represents the mutations
performed through its own product process. On Windows it mirrors the visible
desktop pixels for the target, so another window covering the target is shown
as the person actually sees it.

The viewer is not an agent command. It ignores mouse input so it cannot block
the underlying target. A product that offers a human dismiss control calls the
host-side viewer API; an agent must never hide or dismiss it.
