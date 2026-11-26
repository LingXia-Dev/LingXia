# Terminal Settings (showcase demo)

The showcase's demo settings app for LingXia's desktop terminal: font, size,
line height, ligatures, color schemes, and on Windows the inline-image runtime.
It is a GPU View page over the `lx.host.terminal` Logic API, which only the
host's own `app.lingxia.terminal-settings` control surface can call.

A host with `capabilities.terminal` supplies its own settings app through
`terminal.settings.path` in `lingxia.yaml`; the showcase points that at this
folder. Run `npm install` here before the first build.
