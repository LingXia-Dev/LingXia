# Apple SDK

Public Swift entry points for iOS and macOS host apps. A symbol not listed here
is not part of the host-app contract.

## Paths

| If you are... | Use |
|---|---|
| Building a LingXia host app | `Lingxia.quickStart()` + `lingxia.yaml` |
| Embedding LingXia into an existing native app UI | `Lingxia.initializeRuntime()` + `LxAppController` + `LxAppHostView` |

Host UI (windows, asides, tray) is declared in `lingxia.yaml` →
[Surfaces](./project.md#surfaces), never in Swift. Packet Tunnel packaging is a
CLI convention: [iOS Packet Tunnel extensions](../cli/lingxia.md#ios-packet-tunnel-extensions).

## SDK dependency

Host `ios/Package.swift` and `macos/Package.swift` declare
`.package(name: "lingxia", path: "../.lingxia/sdk/apple")` and the `lingxia`
product. `lingxia build`, `dev`, and `upgrade` prepare that ignored symlink to
the selected SDK cache; SDK upgrades do not rewrite the manifests. Keep
`.lingxia/sdk/` out of Git. Old placeholder/CLI-managed absolute-path manifests
must be updated to this declaration before using these commands. Explicitly
vendored SDK dependencies are left alone.

## Quick start

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

`quickStart()` loads the bundled `app.json` and `ui.json`, starts the runtime,
creates the shell, and opens the launch `main` surface.
`quickStart(configuration:)` is not a way to configure product UI.

## Advanced embedding

Only when an existing native app owns its windows and layout, and LingXia is
mounted into one region:

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

- One `LxAppController` per native integration flow; one `LxAppHostView` per
  embedded region.
- The host owns window and layout; LingXia owns lxapp sessions and WebView
  attachment.

## Symbols

Signatures live in the `lingxia` SwiftPM package (use Xcode jump-to-definition).
The contract is these symbols plus their request/event/id types:

| Symbol | Role |
|---|---|
| `Lingxia` | `runProductCommandIfInvoked()`, `quickStart()`, `handleAppActivation()`, `initializeRuntime()`, `activate(controller:)`, `enableWebViewDebugging()`, `handleAppLink(url:)`, `displayLanguage` |
| `LxAppController` | Sessions for advanced embedding: `open` / `openHomeApp` / `navigate` / `close`, `events` stream, interceptors |
| `LxAppHostView` | Embeddable view: `mount` / `unmount` / `dispatch`, `events` stream (`LxAppHostViewRepresentable` for SwiftUI) |
| `L10n` | SDK strings for host-owned native chrome: `string(_:)`, `string(_:_:)` |

Do not touch `LxAppRuntime.shared`; both entry points wrap it.

## Semantics

- `runProductCommandIfInvoked()` must be the first call in a macOS product
  entrypoint: it runs `HostAddon::install_product_cli` before CLI parsing and
  returns without starting the runtime on a GUI launch.
- Controller events `didOpen` / `didClose` carry the `LxAppSession`;
  `.mountInHost(id:)` mounts it into the registered `LxAppHostView`.
- Host-view events (`didChangeTitle`, `didUpdateCanGoBack`, `didStartLoading`,
  `didFinishLoading`, `didFail`) come from the mounted WebView;
  `dispatch(.triggerCapsuleAction(...))` forwards a capsule action to the
  mounted session.
- `Lingxia.displayLanguage` is the effective display language (saved setting,
  else the locale passed to `initializeRuntime()`). `L10n.string` resolves
  `en` / `zh-Hans` from it.
