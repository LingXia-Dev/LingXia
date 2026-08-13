# The LingXia Terminal

LingXia includes a native terminal surface for macOS and Windows. A terminal
can be the product's main workspace or an aside docked above or below another
surface. It is enabled with `capabilities.terminal: true` and declared with
`native: terminal`; see [App Project Configuration](./skill/app/project.md#terminal-surface).

The terminal is implemented as one shared Rust engine over the platform PTY
(`portable-pty` + `alacritty_terminal`). The macOS and Windows hosts supply the
native view, keyboard and IME input, font shaping, and GPU renderer. There is
no selectable terminal backend in `lingxia.yaml`.

## Current capabilities

- Starts the user's interactive shell without modifying its startup files.
- New tabs and splits inherit the focused session's current directory when it
  can be resolved.
- Supports true color, the 256-color palette, common SGR text styles, wide and
  combining characters, emoji, font fallback, ligatures, selection, clipboard,
  IME, alternate-screen applications, and application-aware wheel input.
- Supports multiple terminal workspaces. Each workspace has tabs and a
  four-direction split-pane tree; panes can be focused, resized, closed, and
  tabs can be renamed. An aside terminal can expand to the full content area;
  macOS can also temporarily zoom one split pane. When several panes are open,
  the pane drag affordance includes a close action.
- **Find...** in the terminal context menu (or Command-F on macOS and Ctrl-F on
  Windows) searches retained scrollback. The search bar supports case matching,
  whole-word matching, previous/next navigation, and visible match highlights.
  Regular expressions remain an engine API and are not exposed in this UI.
- Understands OSC 8 links and tracks terminal title, working directory,
  notifications, progress, command boundaries, scrollback, and process exit as
  structured engine state. Product UI for these signals is added separately;
  an engine capability does not imply that every signal has visible chrome.
- Uses a generation-and-damage frame contract, so an unchanged terminal does
  not repaint and a changed frame identifies the rows that need updating.

## Inline images

macOS and Windows support the first static-image subset of the Kitty graphics protocol.
Terminal applications can transmit direct PNG, RGB, or RGBA payloads (including
chunked and zlib-compressed payloads), query support, create cell-anchored
placements, and delete placements or image data. Placements follow the visible
viewport and alternate-screen state. Clicking a displayed image opens a
resizable native preview window.

The shared engine keeps decoded images and placements in a separate,
generation-based snapshot beside the character-cell frame. Image bytes are
therefore transferred to the host only when image state changes, not copied
once per cell or once per quiet render poll. Per-session transfer, decoded-byte,
dimension, image-count, and placement-count limits bound untrusted payloads.

Both desktop renderers consume the shared image snapshot; macOS uses Metal and
Windows uses the terminal's D3D11 surface. Clicking a visible placement opens
a resizable, aspect-fit native preview on either desktop. The Microsoft ConPTY
redistributable is not bundled into Windows applications. On Windows, Terminal
Settings shows an Inline images switch. Enabling it downloads the fixed package
with `lx.downloadFile`, displays transfer progress, and hands the temporary file
to the restricted native terminal API. The native layer verifies the package
and selected binaries by SHA-256 before installing them under the host's app
state directory. Disabling stops selecting that cached runtime without deleting
it; new tabs and panes pick up either change immediately.

A pasted or uploaded image is only displayed when the terminal application
emits Kitty graphics output; the clipboard action itself is not an inline-image
protocol. Sixel, iTerm2 inline images, animation,
file/shared-memory transport, negative-z compositing behind terminal text, and
persistence into restored scrollback are also not supported in this milestone.

Prompt and TUI icons such as Powerline and Nerd Font symbols do not need an
image protocol. They are Unicode characters (usually private-use code points)
and follow the normal font fallback, shaping, glyph-atlas, and cell-rendering
path. A graphics protocol is needed only when a terminal application sends
actual raster image data. Both LingXia renderers draw box-drawing characters,
block elements, and the four common Powerline separators as cell-sized
procedural glyphs. Other Nerd Font symbols require a matching installed font
and are not guaranteed by the runtime today.

The cross-platform implementation path is:

1. The shared Rust Kitty parser, image store, placement model, limits, pixel
   resize data, and generation snapshot are platform-neutral.
2. macOS composites the static placements with its Metal-backed terminal view.
3. Windows composites the same snapshot through D3D11 without a second protocol
   parser or platform-specific image state; both desktop surfaces own their
   native click preview.
4. Future iTerm2 and Sixel parsers should be input adapters onto the same image
   store and placement model.

## Settings

Enabling the terminal supplies runtime defaults but does not automatically add
a settings screen. A product that exposes settings bundles
`@lingxia/terminal-settings` as the `app.lingxia.terminal-settings` lxapp. The
current settings surface controls:

- system, light, or dark appearance mode;
- the light and dark color schemes;
- installed monospaced font family, font size, line height, and ligatures;
- color-scheme import and reset to framework defaults.

There is no `terminal:` configuration section in `lingxia.yaml`; product-level
YAML enables and places the surface, while user-facing appearance settings live
in app state.

LingXia does not bundle a font. The default candidate order is JetBrains Mono,
SF Mono, Cascadia Code, Menlo, and Consolas; the first installed monospaced
family wins, otherwise the host falls back to an installed monospaced font.

The built-in choices are Default Dark, Default Light, Dim, and High Contrast.
System mode selects the configured light or dark scheme from the OS appearance.
Imports accept Windows Terminal-shaped JSON and Xresources/kitty-style
`name: value` text and make the imported scheme available to that product.

User overrides are stored as `terminal.json` in the product's app-state
directory, and imported schemes are stored under its `themes/` directory. A
missing file uses defaults. A malformed or invalid file is reported and
ignored so it cannot prevent the terminal from opening. Accepted settings are
applied to open terminal surfaces, including the surrounding terminal chrome.

## Native rendering

- macOS shapes text with CoreText and renders the grid through Metal.
- Windows shapes text with DirectWrite and renders through D3D11/DirectComposition.

The shared engine owns PTY transport, escape-sequence semantics, the grid,
scrollback, and renderer-facing frames. Platform code owns font discovery and
fallback, input methods, accessibility integration, and native presentation.
