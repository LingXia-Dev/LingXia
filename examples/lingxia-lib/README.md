# lingxia-lib

Native library for your LingXia app. Re-exports LingXia SDK symbols and registers custom JS extensions.

## JS Extensions

Register native extensions via `LxLogicExtension`, accessible as `lx.*` from page code.
See [`src/extension.rs`](./src/extension.rs) for an example.

```ts
const greeting = lx.hello.sayHello("LingXia"); // "Hello, LingXia!"
```

## Optional Cloud Runtime

Enable the `cloud` feature to opt into `lingxia-device-cloud`.

```bash
LXAPP_FEATURES=cloud ./examples/macos/dev.sh --framework react
```
