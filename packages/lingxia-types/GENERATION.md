# Type generation

`@lingxia/types` generates its Logic runtime declarations from the Rust bindings
in `crates/lingxia-logic` with `rong-typegen` 0.5.0.

```sh
npm run gen:logic
npm run check:logic
npm run check:quality
```

`gen:logic` installs the pinned generator under the workspace `target/`
directory on first use, then writes `src/generated/logic.ts` and the DOM-free
`src/generated/logic-web.d.ts` runtime profile. Both outputs are committed so
package consumers never need Rust.

The generated `Lx` interface is merged with `HandwrittenLx`, the previous public
contract retained as a curated compatibility baseline. `check:quality` requires
the merged generated interface to remain assignable to every handwritten API;
generator inference may become more precise, but it cannot weaken or remove the
published surface. Signatures that need correlated overloads stay in the
curated augmentation and use a non-callable generated registration signature.
The same check also ties the generated Logic Web declarations (`fetch`, URL,
encoding, abort, streams, timers, console, and related types) to the explicit
`rong_modules::init` array used by the LingXia Logic runtime.
