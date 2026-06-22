# lingxia-transfer

Shared file transfer domain/runtime for LingXia. The current implementation
contains persistent download management, resumable user-cache downloads, and
multipart file upload primitives.

## What it provides

- Persistent download records and snapshots
- Active download tracking and event subscription
- Download directory configuration helpers
- Retry/cancel/remove flows for browser-owned and user-cache downloads
- Multipart upload with progress events, timeout tuning, and cancellation hooks

## Key APIs

- `snapshot(...)`, `subscribe(...)`, `record(...)`
- `set_dir(...)`, `configured_dir(...)`, `reset_dir(...)`
- `cancel(...)`, `retry(...)`, `remove(...)`, `clear_completed(...)`
- `runtime::*` for integration hooks used by browser/lxapp layers
- `user_cache::*` for resumable lxapp-owned downloads
- `upload_file_with_behavior(...)` for multipart upload primitives

## Notes

This crate owns the transfer state machines and persistence layer. UI-facing
host APIs are registered or wrapped from higher-level crates such as
`lingxia-browser-shell` and `lingxia`.
