# Driving a Shipped Product

Use this reference when a host app exposes local automation on macOS or
Windows. The product declares the surface and owns user consent.

## Capabilities

```yaml
capabilities:
  appUse: true       # this product's windows
  computerUse: true  # the machine; implies appUse
  browserUse: true   # this product's in-app browser; requires browser
```

- `browserUse` never reaches external browser processes; those require
  `computerUse`.
- A refused namespace is final. Do not route around it.

## Access lifecycle

LingXia exposes no agent-callable access toggle. From
`HostAddon::start_services`, call `local_control::install(enabled)` with the
product preference. Use `set_enabled(bool)` for live changes and `is_enabled()`
for live state. A product without settings UI may temporarily default to
`true`. IPC lives under `<app_data>/lingxia/control`, never under host-owned
`app_state`.

`--allow-control` acknowledges an authorized mutation; it does not grant
access. Use `--allow-destructive` only when the request explicitly authorizes
the destructive effect.

## Product command discovery

Invoke the exact product executable as `<executable> --cli ...`; LingXia has no
launcher and does not rely on shell `PATH`.

The product owns its agent skill and locator. A release build may atomically
write `current_exe()` to `~/.<product>/path`. Developer builds must not replace
that locator; the skill resolves one product-owned environment override first,
then the release locator. Use `lingxia::app::{env_version, EnvVersion}` to
distinguish builds. Their app-data paths already isolate their IPC endpoints.

Register a command and its matching request namespace before services start:

```rust
impl lingxia::HostAddon for AppHostAddon {
    fn install_product_cli(&self, cli: &mut lingxia::product_cli::ProductCli) {
        cli.command("cloud", "Manage cloud workspaces", cloud_cli);
    }

    fn install_host_apis(&self) {
        lingxia_control_runtime::register_control_namespace("cloud", handle_cloud_control);
    }
}
```

The CLI handler receives `product_cli::Transport` and arguments after its
command name. `start_services` is too late for registration; use it to start
local control and publish the release locator.

## Agent behavior

- Read `<product> --help` and leaf `--help`; prefer `--json`.
- Stable exits are: 2 usage, 3 not found, 4 ambiguous, 5 timeout, 6 permission
  or refusal, 7 unsupported, 8 unavailable, 9 stale handle, 10 resolved-target
  failure.
- Before `computerUse` on macOS, run
  `<product> computer permissions --json`. If a grant is missing, ask the user
  and stop retrying.
- Never hide or dismiss activity indicators or persistent control disclosure.
