---
title: Agent control
description: Let a local command line or agent drive the product you ship, with the user owning the switch.
sidebar:
  order: 10
---

`lxdev` drives a **development** session. A shipped product can expose its own
local interface instead, so a command line or an agent drives the installed app
on macOS and Windows. The product decides which surfaces exist; the user decides
whether the interface is on.

## Declare the surfaces

```yaml
capabilities:
  appUse: true       # this product's own windows
  computerUse: true  # the whole machine
  browserUse: true   # this product's in-app browser (requires `browser`)
```

The running product enforces these. `computerUse` implies `appUse`, because
machine-wide control already reaches the product's own windows. `browserUse` is
independent and never reaches an external Chrome, Edge, or Safari process —
those are ordinary machine windows and need `computerUse`. A refused namespace
is final.

## The product is its own command

The endpoint is local to the user who launched the app, answers only while the
app is running, and exposes only the surfaces the product declared. LingXia
does not expose `control enable`, `control disable`, or another agent-callable
way to grant access. The product owns that decision and its trusted settings
UI.

Call `lingxia_control_runtime::local_control::install(enabled)` from
`HostAddon::start_services`, passing the product-owned preference. A product
that has not shipped that UI yet may explicitly use `true` as its transitional
default. Later changes apply immediately through `local_control::set_enabled`,
and `local_control::is_enabled` reports the live state.

LingXia keeps the endpoint under `<app_data>/lingxia/control`; it never writes
an executable, locator, or socket into the host-owned `app_state`.

Leaf commands document their own syntax through `--help`, and prefer `--json`
where a leaf offers it. Failures use stable exit codes — 2 usage, 3 not found,
4 ambiguous, 5 timeout, 6 permission or refusal, 7 unsupported, 8 unavailable,
9 stale handle, 10 failure after the target resolved.

## Integrate the product's agent tooling

LingXia supplies local command transport, but the host product owns its Codex,
Claude, or other agent integration. Agent tooling invokes the exact product
executable with the private `--cli` marker; there is no framework launcher and
no assumption that an agent can see the user's shell `PATH`.

A release build may atomically publish `current_exe()` as one line in a
product-owned locator such as `~/.<product>/path`. Developer builds should not
replace that release locator; the product's skill can resolve one explicit
environment override such as `<PRODUCT>_PATH` first, then the release locator.
Release, preview, and developer endpoints are already isolated by their
environment-specific app-data directories.

LingXia does not choose the locator or generate a skill whose business rules
would drift from the product. The host owns and distributes both, and its agent
tooling should query the running product before advertising capabilities.

Providers declare commands with `cli.command(name, about, handler)` from
`HostAddon::install_product_cli`. LingXia runs that hook before parsing the
separate CLI process and before initializing UI, services, or databases. The
matching in-app request namespace belongs in `install_host_apis`; registering
either half from `start_services` is too late.

The product skill adds the framework-reserved `--cli` discriminator and LingXia
removes it before built-in or provider command parsing. This keeps
non-TTY agent launches in CLI mode without exposing framework plumbing to the
product command.

## Permissions and disclosure

On macOS, `computerUse` needs Accessibility and Screen Recording. Commands
execute inside the product, so macOS attributes both grants to the installed
product rather than to the terminal that invoked it. An agent should check
before machine-wide work:

```text
<product> computer permissions --json
```

The first mutating command opens a visible activity indicator that follows the
work and hides after a period of inactivity; read-only commands do not open it.
A controlled session also keeps a persistent disclosure visible for the whole
session, including read-only periods. Neither is an agent command — an agent
must never hide or dismiss them.
