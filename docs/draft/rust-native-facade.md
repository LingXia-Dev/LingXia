# LingXia Rust Native Facade Status

The Rust native facade cleanup is complete. This file records the guardrails
that should stay true.

## Public Entry Points

Native app developers should import from `lingxia` and its explicit facade
modules:

- `lingxia::native`
- `lingxia::HostAddon`
- `lingxia::register_host_addon`
- `lingxia::LxApp`
- `lingxia::Result`
- `lingxia::Error`
- `lingxia::app`
- `lingxia::task`
- `lingxia::downloads`
- `lingxia::settings`
- `lingxia::file`
- `lingxia::media`
- `lingxia::provider`
- `lingxia::push`
- `lingxia::log`
- `lingxia::update`
- `lingxia::host`
- `lingxia::js` when `js-lxapp` is enabled

Lower-level workspace crate names should not be part of normal native app
authoring.

## Service Boundary

`lingxia-service` is the shared behavior layer for APIs reused by `lingxia`,
`lingxia-logic`, and `lingxia-shell`.

Current service modules:

- `downloads`
- `settings`
- `file`
- `media`

Do not move whole domain/runtime crates into `lingxia-service`. Keep
implementation crates such as `lingxia-media` and `lingxia-transfer` as lower
layers, and put only reusable service behavior in `lingxia-service`.

## JS AppService Boundary

JS extension authoring stays under `lingxia::js` and is only available with the
`js-lxapp` Cargo feature.

Root-level `register_logic_extension` and `LxLogicExtension` must not return.

`features.appService` in `lingxia.yaml` maps to the Cargo `js-lxapp` feature.
Lxapp manifests must use `logic`, not `appService`.

## Bridge Boundary

Bridge protocol primitives are exposed as `LingXiaBridge.raw.*`.

High-level host convenience helpers are:

- `LingXiaBridge.invoke`
- `LingXiaBridge.stream`
- `LingXiaBridge.notify`
- `LingXiaBridge.channel`

Generated browser-global native clients use `LingXiaBridge.raw.*` because they
already generate full `host.*` routes and wrap handles themselves. Generated TS
module clients use the high-level bridge helpers.

## Tests That Guard This

- `crates/lingxia/tests/facade_boundary.rs` compiles native handlers returning
  `lingxia::Result`.
- The same test prevents root-level JS extension re-exports from returning.
- The same test prevents `lingxia-logic` from depending on `lingxia`.

Keep these tests when changing facade boundaries.
