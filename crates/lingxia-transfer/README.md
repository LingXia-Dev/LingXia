# lingxia-transfer

Shared file transfer domain/runtime for LingXia. The current implementation
contains download management, and the crate name is intentionally generic so
upload support can be added later without another crate migration.

Shared download domain/runtime for LingXia.

## What it provides

- Persistent download records and snapshots
- Active download tracking and event subscription
- Download directory configuration helpers
- Retry/cancel/remove flows for browser-owned and user-cache downloads

## Key APIs

- `snapshot(...)`, `subscribe(...)`, `record(...)`
- `set_dir(...)`, `configured_dir(...)`, `reset_dir(...)`
- `cancel(...)`, `retry(...)`, `remove(...)`, `clear_completed(...)`
- `runtime::*` for integration hooks used by browser/lxapp layers
- `user_cache::*` for resumable lxapp-owned downloads

## Notes

This crate owns the download state machine and persistence layer. UI-facing host
APIs are registered from higher-level crates such as `lingxia-shell`.
