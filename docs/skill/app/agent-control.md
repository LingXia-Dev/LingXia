# Driving a Shipped Product

Use this reference when a host app lets a local command line or agent drive
the product. The product decides which surfaces exist; the user decides whether
the local interface is enabled.

## Declare the surfaces

```yaml
capabilities:
  appUse: true       # this product's own windows
  computerUse: true  # the whole machine
  browserUse: true   # this product's in-app browser (requires `browser`)
```

These capabilities are available on macOS and Windows and are enforced by the
running product:

- `computerUse` implies `appUse` because machine-wide control can already reach
  the product's windows.
- `browserUse` is independent; a browser-only product need not expose its
  window chrome.
- `browserUse` never reaches an external Chrome, Edge, or Safari process.
  External browsers are ordinary machine windows and require `computerUse`.
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

The first mutating `computerUse` command on macOS or Windows opens a visible
activity indicator. It follows the work, avoids the pointer target, hides after
roughly twelve seconds of inactivity, and returns on the next mutation.
Read-only commands do not open it.

Each product process owns at most one viewer. Separate running products do not
coordinate a machine-wide viewer; the viewer always represents the mutations
performed through its own product process. Its identity bar always names both
ends of the relationship (`<product> controls <target>`), including above
an expanded preview, so the preview is never an anonymous floating capture.

On both desktop platforms, a foreground target uses a compact control bar
because the target itself is already visible. Background work expands to a
live preview; work with no window target mirrors the visible display. On
Windows, compact mode also requires the product and target to be visible on the
same monitor. When the product has a visible window, the Windows indicator
stays on that window's monitor even if the controlled window is on another
monitor. macOS follows the display containing the controlled target. Both
layouts use platform-native DPI/point sizing and keep the preview aspect ratio.

Windows input still uses the active desktop. A `--window` or unambiguous `--pid`
input target is activated before pointer or keyboard input, so the product may
remain visible on another monitor but is not the focused window while the
input is delivered. This is different from macOS process-directed background
input.

The activity indicator is not an agent command. It ignores mouse input so it
cannot block the underlying target. A product that offers a human dismiss
control calls the host-side viewer API; an agent must never hide or dismiss it.

An observed or controlled product session also keeps a persistent disclosure
visible for the whole session, including read-only periods. The activity
preview may auto-hide; disclosure does not. Only local UI or trusted host
lifecycle can end it. Ordinary snapshot and capture APIs do not turn
supervision on.
