# Driving a Shipped Product

A host app can let a command line — or an agent running one — drive it from
the same machine. This page is the model: what a product declares, who
decides, and where the commands come from. For the command list, ask the
product: `<command> --help`, or `<command> skills show`, which prints the
skill it would install. Both are generated from the build in front of you, so
neither can drift the way a list copied into a page does.

This is not a network API. The endpoint is local-only, reachable by the user
who launched the app, and answers nothing while the app is closed.

---

## Declare what an agent may drive

```yaml
capabilities:
  appUse: true       # this product's own windows
  computerUse: true  # the whole machine
  browserUse: true   # the in-app browser (requires `browser`)
```

macOS and Windows only. There is no capability for the transport — the local
socket these need is derived. A capability list says what a product can do,
not which IPC carries it.

`computerUse` **implies** `appUse`. Not for symmetry: it already contains it.
An agent that may screenshot any window and post input to any window can reach
this product's windows through the wider door, so requiring both would add no
protection and one confusing failure — a product that declared only
`computerUse` would find machine-wide commands working while its own were
refused.

`browserUse` does not imply `appUse`. "Open pages, don't touch my chrome" is a
real choice.

**These names are enforced at runtime.** The endpoint refuses a namespace the
product did not declare, whatever happens to be compiled in. If an agent is
refused, that is the answer — not something to route around.

---

## Declaring is not enabling

Declaring ships the ability. Whether the endpoint listens is the user's
decision, the same way `autostart` works: the product can register itself at
login, and only the user says it should.

The endpoint stays closed until they turn it on, and turning it off leaves
nothing behind — no socket, no command on `PATH`. Nothing that suggests it is
still on.

---

## Where the command comes from

The product's executable *is* the command line. There is no second binary to
install and therefore no way for the two to be different versions of each
other. Typing a subcommand, or arriving through the launcher, runs the command
and exits; launching the app normally starts the app.

When the capability is switched on, the product writes a small launcher into
its own state directory and that directory goes on `PATH` for terminals it
spawns. So:

- **an agent running in the product's own terminal** needs no installation at
  all — the command is already on its `PATH`
- **an agent in the user's own terminal** gets it once the user has switched
  the capability on

Every command takes `--json`. Failures carry an exit code, not just a message:

| | |
|---:|---|
| 2 | usage |
| 3 | not found |
| 4 | ambiguous |
| 5 | timed out |
| 6 | permission |
| 7 | unsupported |
| 8 | unavailable |
| 9 | stale handle (an id that no longer exists) |
| 10 | failed after the target was resolved |

Commands that change something need `--allow-control`; destructive ones also
need `--allow-destructive`.

---

## Agent skills

```
<command> skills show                     # read it first
<command> skills install --agent claude   # or --agent codex
<command> skills remove --agent claude
```

The skill is rendered from the running product: every command it actually has,
and the capability list it actually declared. A skill packaged separately can
only describe what its author imagined, and falls behind the first version bump.

Installing writes into another tool's configuration directory, which is why it
is a command the user runs and names an agent for, rather than something a
switch does quietly.

If the product is not running when the skill is written, the capability list is
recorded as unknown rather than guessed. A generated file stating something
false reads exactly as confidently as one stating something true.

---

## `computerUse` and the operating system

Machine-wide automation needs the OS's permission — on macOS, Accessibility and
Screen Recording. Two things follow, and both matter:

**The user is asked by name.** macOS attributes these grants to the responsible
process. Commands run *inside the app*, not in the terminal that typed them, so
the entry the user sees in System Settings is the product they installed — one
row, its name, revocable where they would look for it.

Had the commands run in the calling process instead, the grant would attach to
whatever terminal launched it: a different answer from iTerm than from
Terminal, and a Privacy pane naming a terminal the user never meant to give
control of their computer to.

**Without the grant, results are quietly reduced, not refused.** A screen
capture without Screen Recording succeeds and returns the desktop picture with
no window contents at all. An agent that does not check permissions will read
that as an empty screen. Call the permissions command first; a `permission`
failure means the user has to grant something, and no amount of retrying will
change it.

Grants follow the app's code signature, so a signed product keeps them across
updates. An unsigned development build is a different subject to the OS each
time it is rebuilt, and will be asked again.

## The viewer

On macOS, the first `computerUse` command that changes something opens a small
window in the corner mirroring what is being driven, with a ring on the point
just acted on. It follows the work, leaves after about twelve seconds of quiet,
and returns on the next such command. Commands that only look — screenshots,
window lists, accessibility queries — never open it.

There is no command for it, deliberately. It exists so a person can see their
own machine being driven, and an agent able to switch it off could work
unobserved. Closing it is the person's to do, and it stays closed for that run.

It ignores the mouse, so it can never swallow a click meant for what is
underneath it; when the work happens under it, it moves to another corner.
