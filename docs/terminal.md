# The LingXia Terminal

LingXia products can host a native terminal as a main workspace or docked
surface on macOS and Windows. A shared Rust engine owns PTY transport, terminal
semantics, scrollback, and frames; each platform host owns input, shaping, and
GPU presentation.

For declaring the surface, see [App Project Configuration](./skill/app/project.md#terminal-surface).

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

## Settings

`capabilities.terminal: true` enables the terminal engine; it does not add
configuration commands to product control. Products that expose terminal
settings should provide a normal Web UI/lxapp resource for that experience.
The Settings UI writes user overrides and applies accepted changes to open
terminal surfaces.

Framework defaults are intentionally enough for a product to ship without
`lingxia.yaml` terminal configuration:

- font candidates start with JetBrains Mono, then platform coding fonts;
- appearance follows the system by default;
- light and dark schemes default to `lingxia-light` and `lingxia-dark`.

There is no `terminal:` section in `lingxia.yaml` for defaults or themes.
User changes live in the product's app state as `terminal.json`; a malformed
file is reported and ignored instead of preventing the terminal from opening.

## Fonts

LingXia does not bundle a font. `font.family` is an ordered candidate list, and
the first installed monospaced family wins. The Settings UI should list
installed monospaced families, mark detected ligature/Nerd Font support, and
show which configured candidate resolved.

## Themes

The engine includes four original schemes:

- `lingxia-dark`
- `lingxia-light`
- `lingxia-dim`
- `lingxia-contrast`

User imports accept Windows Terminal-shaped JSON with ANSI colors, and
Xresources/kitty-style `name: value` text. Imported names override an engine
theme with the same name.

## Rendering

Both desktop hosts consume the engine's compact generation/damage frame
contract. An unchanged frame causes no terminal repaint.

- macOS shapes with CoreText and renders through Metal.
- Windows shapes with DirectWrite, caches glyphs by their resolved font face,
  and presents through D3D11/DirectComposition.

The engine keeps terminal meaning platform-neutral while each renderer uses
the native font fallback and composition stack.
