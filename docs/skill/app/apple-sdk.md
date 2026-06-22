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

## API Reference

### `Lingxia`

```swift
@MainActor
public enum Lingxia {
    @discardableResult
    public static func quickStart() throws -> LxAppShell

    @available(*, deprecated, message: "Configure product UI in lingxia.yaml and use Lingxia.quickStart(). Use initializeRuntime() + LxAppController + LxAppHostView for advanced embedding.")
    @discardableResult
    public static func quickStart(
        configuration: LxAppShellConfiguration
    ) throws -> LxAppShell

    public static func handleAppActivation() -> Bool

    @discardableResult
    public static func initializeRuntime() throws -> LxAppRuntimeInfo

    public static func activate(controller: LxAppController)

    public static func enableWebViewDebugging()

    public static func handleAppLink(url: URL)
}
```

`Lingxia.initialize()` has been removed. Use `quickStart()` for product apps or
`initializeRuntime()` for advanced embedding.

### `LxAppController`

```swift
@MainActor
public final class LxAppController {
    public let id: LxAppControllerID
    public private(set) var sessions: [LxAppSessionID: LxAppSession] { get }
    public var events: AsyncStream<LxAppControllerEvent> { get }

    public init()

    public func setInterceptor(
        _ kind: LxAppControllerInterceptor,
        handler: ((LxAppInterceptContext) async -> LxAppInterceptDecision?)?
    )

    public func session(forAppId appId: String) -> LxAppSession?

    @discardableResult
    public func open(_ request: LxAppOpenRequest) async throws -> LxAppSession

    @discardableResult
    public func openHomeApp(path: String = "") async throws -> LxAppSession

    public func navigate(_ request: LxAppNavigateRequest)

    @discardableResult
    public func close(_ sessionId: LxAppSessionID) async -> Bool
}
```

Supporting types:

```swift
public struct LxAppControllerID: Hashable, Codable, Sendable
public struct LxAppSessionID: Hashable, Codable, Sendable
public struct LxAppSession: Hashable, Codable, Sendable, Identifiable
public struct LxAppOpenRequest: Codable, Sendable
public struct LxAppNavigateRequest: Codable, Sendable
public enum LxAppOpenPresentation: String, Codable, Sendable, CaseIterable
public enum LxAppAnimation: String, Codable, Sendable, CaseIterable
public enum LxAppControllerEvent: Codable, Sendable
public enum LxAppControllerInterceptor: String, Codable, Sendable
public struct LxAppInterceptContext: Codable, Sendable
public enum LxAppInterceptDecision: Codable, Sendable
public struct LxAppErrorPayload: Codable, Sendable, Error, Hashable
public enum LxAppJSONValue: Codable, Sendable, Hashable
```

Controller event semantics:

- `didOpen` carries the opened `LxAppSession`.
- `didClose` carries the closed `LxAppSession`.
- `.mountInHost(id:)` mounts the opened session into the registered `LxAppHostView`.

### `LxAppHostView`

```swift
#if os(iOS)
public typealias LxAppPlatformView = UIView
#else
public typealias LxAppPlatformView = NSView
#endif

@MainActor
public final class LxAppHostView: LxAppPlatformView {
    public let id: LxAppHostViewID
    public let controller: LxAppController
    public private(set) var webView: WKWebView? { get }
    public private(set) var mountedSession: LxAppSession? { get }
    public private(set) var appId: String? { get }
    public private(set) var currentPath: String? { get }
    public private(set) var canGoBack: Bool { get }

    public var events: AsyncStream<LxAppHostViewEvent> { get }

    public init(controller: LxAppController, frame: CGRect = .zero)
    public func attach(_ webView: WKWebView, appId: String?, path: String?)
    public func mount(_ session: LxAppSession) async throws
    public func mount(sessionId: LxAppSessionID) async throws
    public func unmount()
    public func dispatch(_ command: LxAppHostViewCommand)
}
```

SwiftUI wrapper:

```swift
public struct LxAppHostViewRepresentable {
    public init(hostView: LxAppHostView, onEvent: ((LxAppHostViewEvent) -> Void)? = nil)
    public init(controller: LxAppController, onEvent: ((LxAppHostViewEvent) -> Void)? = nil)
}
```

Supporting types:

```swift
public struct LxAppHostViewID: Hashable, Codable, Sendable
public enum LxAppHostViewEvent: Codable, Sendable
public enum LxAppHostViewCommand: Codable, Sendable
```

Host-view event semantics:

- `didChangeTitle`, `didUpdateCanGoBack`, `didStartLoading`, `didFinishLoading`, and `didFail` come from the mounted webview.
- `dispatch(.triggerCapsuleAction(...))` forwards that action into the runtime for the mounted app session.

### `LxAppRuntime`

```swift
@MainActor
public final class LxAppRuntime {
    public static let shared: LxAppRuntime
    public private(set) var info: LxAppRuntimeInfo? { get }
    public func initialize() throws -> LxAppRuntimeInfo
}

public struct LxAppRuntimeInfo: Codable, Sendable, Hashable {
    public let homeAppId: String
    public let capabilities: LxAppCapabilities
    public let dataPath: String
    public let cachesPath: String
}
```

Most apps should not call `LxAppRuntime.shared.initialize()` directly. Use
`Lingxia.quickStart()` or `Lingxia.initializeRuntime()`.

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
