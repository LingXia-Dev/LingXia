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
app is running, and stays closed until the user enables it.

```text
<product> control enable    # prints the launcher path and the PATH line to add
<product> control status    # live listener vs. a setting that applies next start
<product> control disable   # stops the listener, persists it, removes the socket
```

Leaf commands document their own syntax through `--help`, and prefer `--json`
where a leaf offers it. Failures use stable exit codes — 2 usage, 3 not found,
4 ambiguous, 5 timeout, 6 permission or refusal, 7 unsupported, 8 unavailable,
9 stale handle, 10 failure after the target resolved.

## Generate a skill from the running build

```text
<product> skills show
<product> skills install --agent claude   # or --agent codex
```

The generated skill contains only the entry points the running build actually
allows, so it cannot advertise a capability the product refuses. If the product
cannot be reached, `show` and `install` fail rather than guessing. Installing
writes into another agent's configuration directory, so it stays an explicit
user command.

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
