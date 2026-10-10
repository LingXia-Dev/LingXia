# Servo patches

The workspace and standalone showcase native manifest apply the same patches.
Servo 0.7.0 uses the system allocator unless `use-jemalloc` is enabled, so no
allocator override is needed.

| Crate | Upstream version | Remaining patch |
| --- | --- | --- |
| `servo-fonts` | 0.7.0 | Register Android language-only fallback families and honor TTC face indices. |
| `servo-script` | 0.7.0 | Handle hidden iframe observers and present query-generated empty scenes at the normal frame boundary. |
| `servo-paint` | 0.7.0 | Batch iframe display lists and preserve scroll offsets across pipeline changes. |
| `servo-net` | 0.7.0 | Expose read-only network diagnostics and top-level failure/download hooks; align `rusqlite` with 0.40. |
| `servo-storage` | 0.7.0 | Align `rusqlite` with 0.40 so LingXia links one SQLite library. |
| `sea-query-rusqlite` | 0.8.0 | Align its `rusqlite` dependency with the same 0.40 line. |

The network and font directories describe their hooks in `README.lingxia.md`.
Remove each patch when the corresponding upstream API or dependency is compatible.
