# native

Native library for your LingXia app. Re-exports LingXia SDK symbols and installs a host addon.

## Host Addon

Implement `lingxia::HostAddon` to contribute bootstrap behavior and JS APIs.
Logic extensions registered from that addon are accessible as `lx.*` from page code.
See [`src/extension.rs`](./src/extension.rs) for an example.

```ts
const greeting = lx.hello.sayHello("LingXia"); // "Hello, LingXia!"
```

## Optional Native Features

This example keeps external providers out of its default dependency graph. Apps
that own a cloud or update provider should declare that provider as an optional
dependency in their host crate, then enable the corresponding Cargo feature with
`lingxia build --native-feature <feature>` or `LINGXIA_NATIVE_FEATURES`.
