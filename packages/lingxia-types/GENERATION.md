# Type generation

`@lingxia/types` generates its Logic runtime declarations from the Rust bindings
in `crates/lingxia-logic` with `rong-typegen` 0.6.0.

```sh
npm run gen:logic
npm run check:logic
npm run check:quality
```

`gen:logic` installs the pinned generator under the workspace `target/`
directory on first use, then writes `src/generated/logic.ts` and the DOM-free
`src/generated/logic-web.d.ts` runtime profile. Both outputs are committed so
package consumers never need Rust.

The generated module replaces all previous handwritten domain declaration
files. Those files used to have matching package subpaths (`./app`, `./surface`,
and so on); the subpaths were aliases of the same generated file and are gone.
Import reusable shapes from `@lingxia/types`, and automation types from
`@lingxia/types/automation`.

Runtime-backed structs/classes come directly from their Rust bindings;
semantic unions, callbacks, handles, and lifecycle contracts live as TS-only
`js_api!` metadata in `crates/lingxia-logic/src/public_types.rs`. Binding-layer
`JS*` class names stay unexported so they do not leak into the public contract.

Rong typegen cannot yet express generic TS-only declaration names or correlated
overloads. The minimal generation prelude therefore contains only nine generic
contracts plus the `downloadFile` overloads. It is generator input, not a second
public declaration tree.

`check:quality` verifies the complete legacy public-name manifest, critical
documentation, branded paths, overload resolution, and representative complex
return types. The old handwritten declarations and comparison fixture are not
kept in the repository.

The same check also ties the generated Logic Web declarations (`fetch`, URL,
encoding, abort, streams, timers, console, and related types) to the explicit
`rong_modules::init` array used by the LingXia Logic runtime.
