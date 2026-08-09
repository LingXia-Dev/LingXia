# Terminal

The built-in terminal is a native macOS/Windows surface backed by the shared
Rust terminal engine. Enable it with `capabilities.terminal`; declare where a
terminal appears under `surfaces:`. See [App Project Configuration](./project.md#terminal-surface).

## User settings

Enabling the terminal also bundles `@lingxia/terminal-settings` as a standard
desktop workspace. That screen is the primary user interface for:

- light/dark/system appearance and independent light/dark color schemes;
- font candidates, size, line height, bold treatment, and ligatures;
- background opacity and cursor style/blink;
- importing Windows Terminal JSON or Xresources/kitty color files.

The screen previews themes without saving and applies accepted changes to open
terminal surfaces. Settings routes are restricted to the SDK-owned settings
app, so the screen works even when the product's local control endpoint is off.

Products normally use the SDK package. To develop or replace it, select one
source in `lingxia.yaml`:

```yaml
terminal:
  settings:
    path: ../my-terminal-settings
    # Or: package: "@example/terminal-settings"
    #     version: 1.0.0
```

A package source must contain `lxapp.json` and a prebuilt `dist/` directory.

## Configuration precedence

Terminal configuration has three layers, lowest precedence first:

1. LingXia framework defaults;
2. product defaults from `lingxia.yaml`;
3. user overrides in the product's `app_state/terminal.json`.

Product defaults are a partial terminal configuration:

```yaml
terminal:
  defaults:
    font:
      family: ["JetBrains Mono", "SF Mono", "Cascadia Code", "Consolas"]
      size: 14
      lineHeight: 1.05
      ligatures: true
      bold: weight
    theme:
      mode: system
      light: lingxia-light
      dark: lingxia-dark
      opacity: 1
      cursor:
        style: block
        blink: true
```

The build rejects unknown fields and invalid ranges. A user save stores only
the values that differ from framework/product defaults, so later product
default changes still reach users who did not override those fields.

`font.family` is an ordered candidate list; the first installed monospaced
family wins. No font is bundled. The settings screen reports the resolved
family and missing candidates.

`theme.light` and `theme.dark` stay independent. `theme.mode` is `system`,
`light`, or `dark`. LingXia includes four original engine themes and the
settings package includes licensed Dracula, Nord, and Solarized Dark choices
with their notices.

## Product command

The product executable exposes scriptable terminal configuration through the
same local, user-controlled endpoint as other product-control commands:

```text
myapp terminal config get --json
myapp terminal config apply --patch '{"font":{"size":15}}' --json
myapp terminal config reset font
myapp terminal themes list --json
myapp terminal themes import ./scheme.json --name my-scheme
myapp terminal fonts list --json
```

Run `myapp terminal --help` and leaf-command `--help` for exact syntax. The
endpoint stays closed until the user enables it with `myapp control enable`;
declaring `capabilities.terminal` compiles the command path but does not grant
automation access.

`terminal config get` returns the resolved config, resolved font, current
appearance, and exact user-file path. Use the settings screen or command for
live changes. Directly editing `terminal.json` is supported as persistent
input, but it is not watched; the edit is adopted on the next product start.

## Runtime cost

| Change | Effect on open sessions |
|---|---|
| theme, opacity, cursor | repaint only; no process or grid resize |
| font family, size, line height, ligatures | recompute cells and resize/reflow each open PTY grid |

A malformed user file is reported and ignored; framework/product defaults
remain usable.
