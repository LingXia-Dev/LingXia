# lingxia-native-macros

Procedural macros for registering LingXia native APIs.

## What it provides

- `#[native("namespace.method")]` for unary View-to-native handlers
- `#[native("namespace.method", stream)]` for streaming View-to-native handlers
- `#[native("namespace.method", channel)]` for bidirectional channels

## What the macro generates

- A hidden `HostHandler` implementation
- A hidden `*_host()` registration helper
- Input decoding and result serialization glue for `lxapp::host`

## Notes

This crate is internal infrastructure for LingXia native extensions. Most users
consume it indirectly through the top-level `lingxia` or `lingxia-browser-shell` crates.
