# Terminal

The built-in terminal is a native macOS/Windows surface backed by the shared
Rust terminal engine. Enable it with `capabilities.terminal`; declare where a
terminal appears under `surfaces:`. See [App Project Configuration](./project.md#terminal-surface).

## User settings

`capabilities.terminal` enables the engine; it does not silently add a UI.
Products that expose Terminal Settings bundle `@lingxia/terminal-settings` as
an explicit lxapp resource:

```yaml
capabilities:
  terminal: true

resources:
  bundles:
    - type: lxapp
      appId: app.lingxia.terminal-settings
      package: "@lingxia/terminal-settings"
```

For monorepo development, replace `package` with a project-relative `path`.
The settings screen is the primary user interface for:

- light/dark/system appearance with mode-filtered color schemes;
- installed font family, size, line height, and ligatures;
- importing compatible color-scheme files (including Windows Terminal JSON,
  Xresources, and kitty formats).

The screen previews themes without saving and applies accepted changes to open
terminal surfaces. Its Logic worker receives `lx.terminal` only when the app id
is `app.lingxia.terminal-settings`, the app is host-bundled, and the host
declares terminal capability. Product control can remain disabled.

A product home lxapp may expose the Settings resource as an aside:

```ts
{
  id: "terminal-settings",
  placement: "footer",
  icon: "public/settings.svg",
  label: "Terminal Settings",
  onActivate: () => {
    void lx.openSurface({
      appId: "app.lingxia.terminal-settings",
      as: "aside",
      edge: "right",
    });
  },
}
```

The icon belongs to the home lxapp; sidebar actions cannot reach into the SDK
settings package for assets.

During host development, rebuild this resource in place instead of rebuilding
the home lxapp:

```bash
lxdev lxapp reload --app app.lingxia.terminal-settings
```

For a native Terminal product, Settings may itself be the control/home lxapp
while the native terminal remains the declared main surface:

```yaml
app:
  homeAppId: app.lingxia.terminal-settings
features:
  appService: true
capabilities:
  terminal: true
resources:
  bundles:
    - type: lxapp
      appId: app.lingxia.terminal-settings
      package: "@lingxia/terminal-settings"
surfaces:
  - native: terminal
    role: main
    launch: true
```

`homeAppId` identifies the trusted control app; it does not redefine which
surface is main. A package source must contain `lxapp.json` and a prebuilt
`dist/` directory.

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
    theme:
      mode: system
      light: lingxia-light
      dark: lingxia-dark
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

## Automation

Trusted desktop test contexts expose the native workspace through
`lx.automation().terminal` on macOS and Windows:

```ts
const terminal = lx.automation().terminal;
const before = await terminal.snapshot({ surface: handle.id });
const after = await terminal.split({ surface: handle.id, direction: 'right' });
```

The driver publishes pane-tree, grid, config, and chrome state. It is distinct
from the host-bundled Settings app's scoped `lx.terminal` API and is not
available to ordinary lxapps.

## Runtime cost

| Change | Effect on open sessions |
|---|---|
| color scheme or appearance | repaint only; no process or grid resize |
| font family, size, line height, ligatures | recompute cells and resize/reflow each open PTY grid |

A malformed user file is reported and ignored; framework/product defaults
remain usable.
