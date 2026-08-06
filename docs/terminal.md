# The LingXia Terminal

A native terminal an app can host as a surface: a real PTY running your own
shell, GPU rendering, and a command line for configuring it.

This guide is about using and configuring one. Enabling it in a project is in
the [app skill](./skill/app/terminal.md).

---

## What it is

One cross-platform engine owns everything that decides *meaning* — process
transport, the VT state machine, scrollback, what a byte sequence does.
Platform code only draws the result and collects input. A session therefore
behaves identically wherever it runs; only the drawing differs.

- **Your shell, not ours** — your prompt, aliases and dotfiles, plus OSC 7
  working directory and OSC 133 command boundaries when the shell reports
  them, so new tabs and splits open where you already are.
- **Text** — true color, the full SGR set including strikethrough and the
  underline styles, wide characters, combining marks, ZWJ emoji as single
  glyphs, and coding-font ligatures.
- **Box art drawn to the cell** — box drawing, block elements and powerline
  separators are drawn rather than taken from the font, so borders meet
  exactly at any size and in any face.
- **Tabs and splits** — four-way splits, drag to rearrange, zoom a pane,
  rename a tab.
- **Selection, clipboard, IME**, including CJK input over the grid.
- **Links** — URLs and file paths are recognized in output, and explicit
  OSC 8 hyperlinks are honoured.

---

## Configuring it

Configuration is a command, and the command is the app's own executable. Its
name is that executable's name in lowercase — a product shipped as `MyApp`
answers to `myapp`. It is on `PATH` inside any terminal the app opens.

```
myapp term --help
```

`--help` is the reference; it is generated from the command itself and cannot
drift. What follows is the part `--help` cannot tell you — why the commands
are shaped the way they are.

The file behind them:

```
macOS     ~/Library/Application Support/<bundle-id>/app_state/terminal.json
Windows   %APPDATA%\<bundle-id>\app_state\terminal.json
```

`term path` prints it. Every field is optional, so writing only what you
change is the normal way to keep one. Editing it by hand is equally supported —
the file is watched, and a dotfile repository that syncs it works without any
further ceremony. A file that does not parse is reported and ignored, and
whatever was in effect stays; a save in progress cannot blank the screen.

---

## Fonts

Nothing is bundled. A framework that ships a typeface ships someone's license
with it, and the fonts people actually want for a terminal are the ones they
have already installed.

So `font.family` is an ordered list of candidates and the first installed one
wins:

```json
"family": ["JetBrains Mono", "SF Mono", "Menlo"]
```

Proportional families are skipped. A terminal is a grid; a variable-width face
does not merely look wrong in one, it breaks every column.

`term status` reports which candidate was used and which were missing, so a
name you spelled wrong is visible rather than a silent downgrade.

`term font list` shows the installed monospace families with the two properties
that actually decide a choice:

```
JetBrains Mono                   ligatures
FiraCode Nerd Font Mono          ligatures   nerd icons
Menlo
```

**Ligature support is measured, not read.** The obvious implementation asks the
font for its feature tables, and that answer is wrong for exactly the fonts
people choose for ligatures: the tables report Menlo as having them and
JetBrains Mono as not. Glyph *count* is no better, because a monospace ligature
substitutes two glyphs for two. So the terminal shapes a probe string twice —
once with contextual alternates disabled, once with the font's defaults — and
compares the glyph ids. Different ids mean the font really does it.

Fonts have no preview. No terminal protocol carries a typeface, and changing
one reflows the grid, so a preview would resize every running program. The
list above is what decides the choice instead.

---

## Themes

Two schemes are kept at once, one per system appearance, and `mode` decides
which is in effect: `system` follows the OS, `light` and `dark` pin it.

```
myapp term theme mode system
myapp term theme lingxia-dim              # applies to the appearance in effect
myapp term theme lingxia-contrast --light # or to the one you name
```

Keeping both matters more than it sounds. A single stored scheme means a
choice made at night silently destroys the one made in daylight — and the
person only finds out the next morning.

### What ships

Four schemes are built in: `lingxia-dark`, `lingxia-light`, `lingxia-dim` for
long sessions in a dim room, and `lingxia-contrast` for accessibility and
bright rooms. They are original.

**Well-known schemes are deliberately not bundled.** Solarized, Dracula,
Nord, Gruvbox and the rest are each someone's work under someone's license.
Redistributing them means carrying those licenses correctly, and a framework
that gets that wrong hands the problem to every product built on it. Importing
yours is one command, and leaves the choice unlimited rather than curated:

```
myapp term theme import ~/Downloads/whatever.json
myapp term theme import ~/.Xresources as solarized
```

### Import formats

Two shapes are accepted, chosen because between them they cover essentially
every scheme collection published:

- **JSON** in the Windows Terminal scheme shape — `background`, `foreground`,
  `cursorColor`, and the sixteen ANSI names. This is also what
  `term theme show` prints, so a scheme can be moved between machines by
  piping one into the other.
- **`name: value` text**, which is what Xresources and kitty configuration
  both are. Keys are matched loosely enough that `*.color4`, `color4` and
  `color4:` all land in the same place.

A file in any other shape is reported along with the shapes that do work,
rather than guessed at — a scheme silently imported wrong is worse than one
that refused.

Imported schemes land beside the configuration, one file per scheme, and an
imported name shadows a built-in one of the same name. So a `lingxia-dark` of
your own simply replaces ours.

### Choosing by eye

`term theme` with no name opens a picker inside the terminal you ran it in.
Arrow through the list and the terminal repaints as you go; enter keeps the
highlighted scheme, escape leaves everything as it was.

The preview is not a sample swatch — it is the terminal you are already
looking at, with your own output still in it, which is the only honest way to
judge whether a scheme is comfortable. It is done with the standard OSC 4, 10
and 11 palette sequences, so it affects that one session and writes nothing
until you commit. Abandoning the picker cannot leave you with a scheme you did
not choose.

---

## Behaviour worth knowing

**Theme and font changes are not equally cheap.** A theme change is a repaint:
colors resolve when a frame is drawn, so nothing reflows and no running process
notices. A font or line-height change alters the cell size, so the grid reflows
and running programs are resized — exactly as if you had resized the window.

**Profiles do not apply retroactively.** Changing the shell or its environment
affects sessions started afterwards. A process that is already running cannot
be re-launched under a new command.

---

## Rendering

macOS renders on the GPU. Glyphs are rasterized once into an atlas and drawn as
instanced quads, and only changed rows are uploaded, so a busy `tail -f` costs
about as much as an idle prompt. Text is shaped per run rather than per cell,
which is what makes ligatures possible while every glyph still lands on the
terminal grid.

Windows currently renders through GDI, which cannot do ligatures, color emoji
or complex shaping. Those follow its GPU renderer.
