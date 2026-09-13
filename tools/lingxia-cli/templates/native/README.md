# {{PROJECT_NAME}}

Native library for your LingXia app.

This crate is one Cargo lib with three crate-types (`cdylib` / `staticlib` /
`rlib`) so the same sources cover every host. `lingxia build` rustc's only the
type the current platform links (`.so` on Android/Harmony, `.a` on Apple);
Windows consumes it as an rlib from `windows/`.

## Build

This crate is automatically built by `lingxia-cli` when you run:
```bash
lingxia build
```

The compiled library will be bundled into your app automatically.
