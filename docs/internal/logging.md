# Logging

> **Audience**: contributors working on the LingXia runtime, the dev bridge, or
> log providers. App authors have no surface here: lxapp code uses `console.*`,
> host Rust uses `log`/`tracing` macros, and the runtime captures both.

Every log line in a LingXia host, whatever its source, becomes one
`LogMessage` and goes through one synchronous dispatch that feeds an in-memory
buffer, an optional provider, and the platform logger. The model is
transport-agnostic; this doc defines no backend.

Invariants:

1. Dispatch is synchronous and lightweight.
2. A provider's realtime hook neither blocks nor emits LingXia logs.
3. Recent history is bounded in memory.
4. Collection storage, selection, encoding and transport belong to the provider.
5. Platform logger output does not depend on whether a provider is registered.
6. One level threshold gates Rust `log` records and host (SDK) logs alike.

Code: `crates/lingxia-log/src/lib.rs` (model, dispatch, buffer, provider),
`crates/lingxia/src/logging.rs` (init, level policy, `log` bridge, host FFI,
platform sinks), `crates/lingxia-lxapp/src/lib.rs` (runtime log macros).

## Pipeline

```mermaid
flowchart LR
  A["Rust log / tracing / console / host FFI"] --> B["LogManager.dispatch"]
  B --> C["In-memory LogBuffer"]
  B --> D["LogProvider.on_log"]
  B --> E["Platform logger"]
  C --> F["attach_log_stream → dev bridge"]
  G["collect_logs"] --> H["LogProvider.collect_logs"]
```

`LogManager::dispatch` pushes the message into the `LogBuffer`, calls the
provider's `on_log` on the caller's thread, then writes the platform logger.

| Source | Enters through | Tag |
| --- | --- | --- |
| Rust `log` records | `SdkLogger` (installed by `lingxia::logging::init`) | `Native` |
| `tracing` events | `lingxia_log::tracing_layer()`, installed by `LogManager::init` | the event's `log_tag` field, else `Native` |
| lxapp page console | runtime | `WebViewConsole` |
| Logic console | `tracing` events on target `rong.js.console` | `LxAppServiceConsole` |
| Built-in browser tab console | runtime | `BrowserConsole` |
| Host automation runs | automation runtime (`path` = run id) | `Automation` |
| Swift / Kotlin / ArkTS SDK code | `forwardHostLog` FFI | `Native` |
| Runtime code | `lingxia_log::log`, `LogBuilder`, `lxapp`'s `info!`/`warn!`/`error!`/`debug!`/`verbose!` | caller's |

The tracing layer is part of `LogManager::init` because Logic console output
arrives as `tracing` events, not through the `log` facade.

## Data model

`LogMessage`: `timestamp_ms`, `tag`, `level`, `appid`, `path`, `target`,
`message`. `LogLevel`: `Verbose`, `Debug`, `Info`, `Warn`, `Error`.
`LogTag::as_str()` is the display tag in platform output: `Native`, `LXView`,
`LXLogic`, `Browser`, `Automation`.

## Level policy

`HOST_LOG_LEVEL` is the single threshold. `LINGXIA_LOG_LEVEL` pins it;
otherwise a dev session starts at `debug` and everything else at `info`.
`set_log_level` changes it at run time and also pins it. On devices the dev
WebSocket URL is only known once app config loads, so
`apply_dev_session_level` re-checks then. The Android logcat and Harmony
hilog sinks fix their own cap when first built, so a later raise reaches the
buffer and `lxdev logs` but not logcat/hilog.

`SdkLogger` passes dependency crates' records only at `warn` and above; only
LingXia's own targets (`lingxia*`, `lxapp*`, `rong*`, `macos*`) pass detail.
Renderer, HTTP, TLS and WebSocket crates log every step at info/debug, which
would otherwise flood the dev log stream during a bundle sync.

## Host FFI

SDK code logs through `forwardHostLog(level, category, appId, path, message)`
and guards hot paths with `hostLogEnabled(level)` (Apple `LXLog.swift`,
Android `LxLog.kt` via `NativeApi`, Harmony `LxLog.ets`; Rust
`logging::forward_host_log`). Levels are `0` verbose … `4` error; `category`
becomes `target`. Before the pipeline exists (early cold start) the record goes
straight to the platform logger so bootstrap warnings are not lost.

Forward errors and important warnings only. These records are buffered for
upload and crash diagnosis, and routine traffic would evict the errors the
buffer exists to keep; lifecycle and debug detail stays on os_log / logcat /
hilog.

## Buffer and live attach

`LogBuffer` holds a bounded broadcast channel for live subscribers
(`DEFAULT_LOG_LIVE_CAPACITY` 1024) and a bounded history deque
(`DEFAULT_LOG_HISTORY_CAPACITY` 2048) that evicts from the front.

`attach_log_stream(recent_limit)` / `attach_log_stream_default()` (500) return
a replay window and a live receiver taken together under the history lock, so
a viewer gets recent context and then tails with no gap or duplicate at the
boundary. The dev bridge (`lingxia-control-runtime/src/bridge.rs`) uses it to
serve `lxdev logs`.

## Provider

`LogProvider` has two hooks, both no-ops by default:

- `on_log(&LogMessage)`: realtime, synchronous, on the caller's thread. It must
  enqueue quickly. It must not emit LingXia logs: same-thread re-entry is
  dropped by a dispatch guard, cross-thread re-entry is not detected.
- `collect_logs()`: triggered by `lingxia_log::collect_logs()`. The provider
  selects records from its own store and owns batching, encoding, transport,
  acknowledgement and retry. The trigger carries no payload and no limit, so
  the runtime never builds data a provider would have to decode or discard.

`lingxia_log::register_log_provider` registers one provider before SDK
initialization; a second call panics. Without one, buffering and platform
output still work. Infrastructure crates such as a cloud provider depend on
`lingxia-log` directly, not on the `lingxia` facade.

## Downstream logger

`lingxia::log::register_downstream_logger` installs a second Rust `log` logger
that receives every record `SdkLogger` accepts. It exists for hosts that mirror
Rust `log` output into their own sink. It sees only Rust `log` records, not
console or host FFI logs; structured integration is `LogProvider`.

## Platform output

`lingxia::logging` mirrors every dispatched message to `android_logger`
(Android), `ohos_hilog` (Harmony), `oslog` (Apple), and stderr elsewhere
(Windows).
