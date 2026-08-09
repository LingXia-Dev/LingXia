# The LingXia Terminal

LingXia products can host a native terminal as a main workspace or docked
surface on macOS and Windows. A shared Rust engine owns PTY transport, terminal
semantics, scrollback, and frames; each platform host owns input, shaping, and
GPU presentation.

For project configuration, see the [terminal app skill](./skill/app/terminal.md).

## Everyday behavior

- The terminal starts the user's shell with its aliases, prompt, and dotfiles.
- New tabs and splits inherit the focused session's current directory when it
  can be resolved.
- True color, common SGR styles, wide cells, combining characters, emoji,
  ligatures, selection, clipboard, and IME are represented by the shared
  engine/native renderer contract.
- Workspaces support tabs, four-way splits, drag/resize, pane zoom, and custom
  tab titles.
- Terminal and surrounding native chrome use the same active scheme.

The engine also exposes structured shell events, command boundaries, search,
links, and restore data. Host UX for those semantic APIs remains incremental.

## Settings workspace

`capabilities.terminal: true` enables the terminal engine. A product that ships
the SDK's Terminal Settings lxapp declares it explicitly in
`resources.bundles`; a product home lxapp can then open it as an aside from a
sidebar action. It is the normal way a person changes:

- system/light/dark appearance;
- independent light and dark color schemes;
- ordered font candidates, size, line height, ligatures, and bold treatment;
- opacity and cursor behavior;
- imported Windows Terminal JSON or Xresources/kitty color files.

Hovering a scheme previews it across open terminal surfaces without saving.
Applying or resetting updates open terminals immediately. Theme-only changes
repaint; font metrics resize and reflow PTY grids.

The settings app has a Logic worker and uses its capability-scoped
`lx.terminal` API. The API is installed only for a host-bundled
`app.lingxia.terminal-settings`, so it does not require the product's automation
endpoint. The home lxapp can expose that app id through
`lx.shell.sidebarActions`.

```yaml
resources:
  bundles:
    - type: lxapp
      appId: app.lingxia.terminal-settings
      package: "@lingxia/terminal-settings"
```

## Configuration layers

The resolved configuration is merged in this order:

1. framework defaults;
2. product defaults from `lingxia.yaml`;
3. user overrides in `app_state/terminal.json`.

Products set only their defaults:

```yaml
capabilities:
  terminal: true

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

User saves contain only overrides, so a future product default still reaches
any field the user did not change. Writes are atomic. A malformed file is
reported and ignored instead of preventing the terminal from opening.

Direct file editing is supported for persistent setup but is not watched.
Use Terminal Settings or the product command for live updates; a manual file
edit is adopted on the next product start. The Settings snapshot deliberately
does not expose the user's filesystem path.

## Fonts

LingXia does not bundle a font. `font.family` is an ordered candidate list, and
the first installed monospaced family wins:

```json
{
  "font": {
    "family": ["JetBrains Mono", "SF Mono", "Cascadia Code", "Consolas"]
  }
}
```

The settings workspace lists installed monospaced families, marks detected
ligature/Nerd Font support, and reports which configured candidate resolved.
Missing or proportional candidates are visible rather than silently accepted.

## Themes

The engine includes four original schemes:

- `lingxia-dark`
- `lingxia-light`
- `lingxia-dim`
- `lingxia-contrast`

The settings package also presents Dracula, Nord, and Solarized Dark. Their
source metadata and MIT license text ship with the package. Selecting one
imports its colors into the product's theme store before it becomes the active
user override.

User imports accept:

- Windows Terminal-shaped JSON with foreground/background and ANSI colors;
- Xresources/kitty-style `name: value` text.

Imported names override an engine theme with the same name.

## Product command

For scripts and agents, the product executable exposes terminal configuration
through product control:

```text
myapp terminal config get --json
myapp terminal config apply --patch '{"theme":{"mode":"dark"}}' --json
myapp terminal config reset theme
myapp terminal themes list --json
myapp terminal themes import ./scheme.json --name my-scheme
myapp terminal fonts list --json
```

Run `myapp terminal --help` for the current command tree. The local endpoint is
user controlled and stays closed until `myapp control enable`; declaring the
terminal capability compiles this command path but does not turn automation on.
`terminal config get` includes the exact user configuration path.

## Rendering

Both desktop hosts consume the engine's compact generation/damage frame
contract. An unchanged frame causes no terminal repaint.

- macOS shapes with CoreText and renders through Metal.
- Windows shapes with DirectWrite, caches glyphs by their resolved font face,
  and presents through D3D11/DirectComposition.

The engine keeps terminal meaning platform-neutral while each renderer uses
the native font fallback and composition stack.
