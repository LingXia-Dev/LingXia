# Driving a shipped product

Let a command line or agent on the same machine drive a macOS/Windows host
app. The product declares the surface and owns user consent.

## Capabilities

```yaml
capabilities:
  appUse: true       # this product's windows
  computerUse: true  # the machine; implies appUse
  browserUse: true   # this product's in-app browser; requires browser
```

- `appUse` — screenshot, window list, mouse and keyboard on this product's
  windows; the executable becomes its own command line.
- `computerUse` — any window, synthetic input, the accessibility tree. macOS
  asks the user for Accessibility and Screen Recording for this product.
- `browserUse` never reaches external browsers; those need `computerUse`.
- Declaring ships the ability; the endpoint stays closed until the user turns
  it on. A refused namespace is final. Do not route around it.

## Enable and disable

From `HostAddon::start_services`, call `local_control::install(enabled)` with
the product preference. Use `set_enabled(bool)` for live changes and
`is_enabled()` for the live state. LingXia exposes no agent-callable toggle.

Pass `--allow-destructive` only when the user asked for the destructive effect.

## Agent sessions

`local_control::subscribe` reports each request as a `ControlEvent`: session
start, one `Activity` per request (method, `Reads`/`Changes`/`Unclassified`),
and session end (20 s idle, `stop_current_session()`, or access switched off).
Host namespaces arrive `Unclassified`; the product classifies its own methods.
The shell shows its own in-control indicator with a Stop button that calls
`stop_current_session()`. Draw nothing for it.

## Product command line

Invoke the product executable as `<executable> --cli ...`. LingXia has no
launcher. The product owns its agent skill and locator: a prod build may write
`current_exe()` to `~/.<product>/path`; dev builds must not replace it (use
`lingxia::app::{env, AppEnv}`).

Register a command and its request namespace before services start:

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

The handler receives `product_cli::Transport` and the arguments after its
command name. `start_services` is too late for registration.

## Agent rules

- Read `<product> --help` and leaf `--help`; prefer `--json`.
- Exit codes: 2 usage, 3 not found, 4 ambiguous, 5 timeout, 6 permission or
  refusal, 7 unsupported, 8 unavailable, 9 stale handle, 10 resolved-target
  failure.
- Before `computerUse` on macOS, run `<product> computer permissions --json`.
  If a grant is missing, ask the user and stop retrying.
- Never hide or dismiss activity indicators or control disclosure.
