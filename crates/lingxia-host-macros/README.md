# lingxia-macro

Procedural macros for registering LingXia host APIs.

## What it provides

- `#[host("namespace.method")]` for unary host handlers
- `#[host("namespace.method", stream)]` for streaming host handlers

## What the macro generates

- A hidden `HostHandler` implementation
- A hidden `*_host()` registration helper
- Input decoding and result serialization glue for `lxapp::host`

## Notes

This crate is internal infrastructure for LingXia host extensions. Most users
consume it indirectly through the top-level `lingxia` or `lingxia-shell` crates.
