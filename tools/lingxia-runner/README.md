# LingXia Runner

LingXia Runner is the development host used by `lingxia dev` for standalone
lxapp projects.

The platform implementations are deliberately separated:

- `macos/`: SwiftPM `LingXia Runner.app` plus its macOS-only Rust static
  library in `macos/runner-lib`.
- `windows/`: Rust executable crate for the Windows dev runner. It depends on
  `crates/lingxia-windows-sdk`, the Windows host entry crate.

Do not put platform runner code directly at this directory root. Add shared
Rust code only when there is real cross-platform runner behavior to share; do
not use a shared crate as a dumping ground for platform-specific startup code.
