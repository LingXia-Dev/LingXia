# Terminal

The built-in terminal runs a real PTY under the user's own shell, drawn by one
cross-platform engine. Turning it on and deciding where it appears belongs to
`lingxia.yaml` and is covered with the other surfaces:
[`capabilities.terminal`](./project.md#capabilities-section) and the
[terminal surface rules](./project.md#terminal-surface).

This file is what you need *after* that — how a terminal is configured, and
what a configuration change does to a session that is already running.

Desktop only (macOS and Windows), off by default.

## Configuration is a file, not `lingxia.yaml`

Font and theme belong to the user, not to the product, so they live in a file
the app watches:

```
macOS     ~/Library/Application Support/<bundle-id>/app_state/terminal.json
Windows   %APPDATA%\<bundle-id>\app_state\terminal.json
```

Every field is optional — write only what changes. A file that does not parse
is reported and ignored; whatever was in effect stays, so a half-written save
can never blank a terminal.

```json
{
  "font": {
    "family": ["JetBrains Mono", "SF Mono", "Menlo"],
    "size": 13,
    "lineHeight": 1.0,
    "ligatures": true,
    "bold": "weight"
  },
  "theme": {
    "mode": "system",
    "light": "lingxia-light",
    "dark": "lingxia-dark",
    "opacity": 1.0,
    "cursor": { "style": "block", "blink": true }
  }
}
```

- `font.family` is an **ordered candidate list** and the first family installed
  on the machine wins. No font is bundled, so a single name is a guess.
  Proportional families are skipped — a variable-width face does not merely
  look wrong in a grid, it breaks every column.
- `theme.light` and `theme.dark` are both kept, and `theme.mode`
  (`system` | `light` | `dark`) decides which is in effect. Setting one
  appearance's scheme never disturbs the other's.

## What a change costs

| Changed | Effect on running sessions |
|---|---|
| theme, opacity, cursor | repaint only — colors resolve when a frame is drawn, so nothing reflows and no process is disturbed |
| font family, size, line height, ligatures | the cell size changes, so the grid reflows and every running program is resized, exactly as on a window resize |

Changes apply live. The file is watched, so editing it by hand or syncing it
from a dotfile repository behaves the same as the command below.

## The `term` command

The product's own executable doubles as the terminal's command line, and the
command name is that executable's name **lowercased** — a host whose binary is
`MyApp` answers to `myapp`:

```
myapp term --help
```

Run `--help` for the grammar; it is the only accurate source, and it covers
status, theme, font and reset. The command is on `PATH` inside terminals the
app opens; elsewhere it is the executable inside the application bundle.

On Apple hosts this is `Lingxia.runTerminalCommandIfInvoked()`, called at the
very top of `main` **before AppKit**: a configuration command must neither open
a window nor initialize the runtime, because initialization opens the app's
databases and collides with a running instance.
