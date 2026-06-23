# lingxia-devtools-cli (`lxdev`)

`lxdev` is the automation client for a running [`lingxia dev`](../lingxia-cli) session. It connects to the session's dev websocket and lets you inspect and drive the host app's browser tabs, the lxapps inside it, the host windows, and the dev log stream — without rebuilding or restarting the session.

It is designed for **agents and scripts**: plain flags in, predictable output out (compact JSON with `--json`), one command per invocation, and an exit code that reflects success. The only long-running command is `lxdev logs -f`.

## Install / run

`lxdev` builds from this workspace and is on `PATH` after a standard `lingxia` install.

```bash
cargo build -p lingxia-devtools-cli --release   # builds the optimized `lxdev` binary
cargo run -p lingxia-devtools-cli -- --help
```

There is no setup: start a session with `lingxia dev` in a project, then run `lxdev` from anywhere inside that project tree — it resolves the session from `.lingxia/sessions/` relative to the working directory.

## Commands

The binary's `--help` is the authoritative, version-matched reference — this README does not duplicate it.

```bash
lxdev --help                 # global selectors (--session / --platform) + the family list
lxdev <family> --help        # commands in a family
lxdev <family> <cmd> --help  # exact flags, defaults, and which are required
```

The five families and what each targets: **`browser`** (the host app's browser tabs), **`app`** (the host windows), **`lxapp`** (the lxapps and their pages), **`logs`** (the dev log stream), **`sessions`** (the dev-session files).

## Conceptual guide

For the model behind the tool — how sessions are discovered, why it refuses to act under ambiguity, the output contract, the two `eval` targets (Logic runtime vs page WebView), the concurrency rules, and a symptom router — see the agent skill doc:

- [`docs/skill/cli/lxdev.md`](../../docs/skill/cli/lxdev.md)

That doc is the single source for usage guidance; keep new conceptual notes there rather than mirroring them here.
