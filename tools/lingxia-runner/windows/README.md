# LingXia Windows Runner

Windows dev runner launched by `lingxia dev` for standalone lxapp projects.

This crate is intentionally separate from `tools/lingxia-runner/macos`.
The macOS runner links `macos/runner-lib` as a Swift static library; this crate
builds the Windows executable and depends on the `lingxia-windows` host entry
crate.
