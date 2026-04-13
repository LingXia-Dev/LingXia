# lingxia-lib

Native library for your LingXia app. Re-exports LingXia SDK symbols and installs a host addon.

## Host Addon

Implement `lingxia::HostAddon` to contribute bootstrap behavior and JS APIs.
Logic extensions registered from that addon are accessible as `lx.*` from page code.
See [`src/extension.rs`](./src/extension.rs) for an example.

```ts
const greeting = lx.hello.sayHello("LingXia"); // "Hello, LingXia!"
```

## Optional Cloud Runtime

Enable the `cloud` feature to opt into `lingxia-device-cloud`.

```bash
LXAPP_FEATURES=cloud lingxia build --platform macos --framework react
```
