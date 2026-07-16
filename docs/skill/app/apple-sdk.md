# Apple SDK Host Guide

> Scope: app-facing Apple SDK APIs for host apps on iOS and macOS.
> If a symbol is not documented here, it is not part of the supported host-app contract.

## Integration Paths

Use one of two paths:

| If you are... | Use |
|---|---|
| Building a LingXia host app | `Lingxia.quickStart()` + `lingxia.yaml` |
| Embedding LingXia into an existing native app UI | `Lingxia.initializeRuntime()` + `LxAppController` + `LxAppHostView` |

Most apps should use `Lingxia.quickStart()`. Window shape, menu bar entries,
sidebar items, toolbar items, titlebar items, startup behavior, and the home
surface are configured in `lingxia.yaml`.

For `lingxia.yaml` configuration, see [App Project Configuration](../app/project.md).

## Quick Start

Use `Lingxia.quickStart()` for product apps.

```swift
import AppKit
import lingxia

class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        Lingxia.enableWebViewDebugging()
        do {
            try Lingxia.quickStart()
        } catch {
            fatalError("Lingxia startup failed: \(error)")
        }
    }

    func applicationShouldHandleReopen(
        _ sender: NSApplication,
        hasVisibleWindows flag: Bool
    ) -> Bool {
        return !Lingxia.handleAppActivation()
    }

    func applicationShouldTerminateAfterLastWindowClosed(
        _ sender: NSApplication
    ) -> Bool {
        return false
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
```

`quickStart()` loads the bundled `app.json` and generated `ui.json`, initializes
the runtime, creates the host shell, and opens the launch `main` surface (the
one with `launch: true`).

## UI Configuration

Host UI belongs in `lingxia.yaml`, not in Swift code — declare it with the
adaptive `surfaces:` list.

Examples:

- A normal window: a `role: main` surface (with `launch: true`).
- A docked companion (sidebar/panel): a `role: aside` surface with an `edge` and a `sidebar:` entry.
- A menu-bar app: a `role: main` surface with a `tray:` entry and no `launch: true` (starts hidden, opened from the tray).

See [Surfaces (adaptive UI)](./project.md#surfaces-adaptive-ui) for the full configuration model.

## Advanced Embedding

Use this path only when an existing native app already owns its windows, scenes,
navigation, split views, panels, or layout, and LingXia should be mounted into
one native region.

```swift
import AppKit
import lingxia

@MainActor
func mountLingXia(in containerView: NSView) async throws {
    try Lingxia.initializeRuntime()

    let controller = LxAppController()
    Lingxia.activate(controller: controller)

    let hostView = LxAppHostView(controller: controller)
    hostView.translatesAutoresizingMaskIntoConstraints = false
    containerView.addSubview(hostView)

    NSLayoutConstraint.activate([
        hostView.topAnchor.constraint(equalTo: containerView.topAnchor),
        hostView.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
        hostView.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
        hostView.bottomAnchor.constraint(equalTo: containerView.bottomAnchor),
    ])

    let session = try await controller.openHomeApp()
    try await hostView.mount(session)
}
```

Rules:

- Use one `LxAppController` per native integration flow.
- Use one `LxAppHostView` per embedded visual region.
- The host app owns native window/layout behavior.
- LingXia owns lxapp session lifecycle and webview attachment.

## API Reference — where it lives

Swift signatures are **not** mirrored here — a hand-copied listing drifts. The
authoritative surface is the `lingxia` SwiftPM package itself: use Xcode
jump-to-definition / autocomplete, or read the package sources.

The supported host-app contract is exactly these symbols (plus their
request/event/id types):

| Symbol | Role |
|---|---|
| `Lingxia` | Entry points and host state: `quickStart()`, `handleAppActivation()`, `initializeRuntime()`, `activate(controller:)`, `enableWebViewDebugging()`, `handleAppLink(url:)`, `displayLanguage` |
| `LxAppController` | Session lifecycle for advanced embedding: `open` / `openHomeApp` / `navigate` / `close`, `events` stream, interceptors |
| `LxAppHostView` | The embeddable native view: `mount` / `unmount` / `dispatch`, `events` stream (`LxAppHostViewRepresentable` wraps it for SwiftUI) |
| `L10n` | SDK localization lookup for host-owned native chrome: `string(_:)` and formatted `string(_:_:)` |

`Lingxia.initialize()` has been removed. Use `quickStart()` for product apps or
`initializeRuntime()` for advanced embedding. Most apps should also never touch
`LxAppRuntime.shared` directly — both entry points wrap it.

Semantics the signatures can't convey:

- Controller events: `didOpen` / `didClose` carry the affected `LxAppSession`;
  `.mountInHost(id:)` mounts the opened session into the registered
  `LxAppHostView`.
- Host-view events `didChangeTitle`, `didUpdateCanGoBack`, `didStartLoading`,
  `didFinishLoading`, and `didFail` come from the mounted webview;
  `dispatch(.triggerCapsuleAction(...))` forwards that action into the runtime
  for the mounted app session.
- `Lingxia.displayLanguage` is the effective display language after
  initialization. A language saved in LingXia settings wins; otherwise it is
  the locale supplied to the Rust runtime by `initializeRuntime()`. `L10n.string`
  resolves the SDK's `en` or `zh-Hans` resource bundle from this value instead
  of independently following the process locale.

## Legacy Shell Override

`LxAppShellConfiguration`, `LxAppShell`, `LxAppSidebarMode`, and
`LxAppToolbarMode` remain available for migration and internal shell work.

New host apps should not use them to configure product UI. Put product UI in
`lingxia.yaml` instead.

Legacy examples:

```swift
var config = LxAppShellConfiguration()
config.sidebar = .hidden
config.toolbar = .hidden
_ = try Lingxia.quickStart(configuration: config)
```

Equivalent product configuration:

```yaml
surfaces:
  - id: myapp
    render: lxapp
    role: main
    launch: true
```

Prefer the YAML form for new apps. See [App Project Configuration → Surfaces](./project.md#surfaces-adaptive-ui) for the full schema.
