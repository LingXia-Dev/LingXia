# Apple SDK Host Guide

> Scope: app-facing Apple SDK APIs for host apps on iOS and macOS.
> If a symbol is not documented here, it is not part of the supported host-app contract.

## Start Here

### Choose Your Integration Path

Use this rule of thumb:

| If you want... | Use... |
|---|---|
| the default LingXia app shell with minimal setup | `Lingxia.quickStart()` |
| the default shell, but with your own sidebar / toolbar / padding / chrome | `Lingxia.quickStart(configuration:)` |
| your own window, split view, panel, or layout, with LingXia mounted inside part of it | `Lingxia.initializeRuntime()` + `LxAppController` + `LxAppHostView` |
| runner / tooling integration | internal runner SPI only, not host-app API |

The main product-grade host APIs are:

- `Lingxia`
- `LxAppShell` and `LxAppShellConfiguration`
- `LxAppRuntime`
- `LxAppController`
- `LxAppHostView`

Tooling-only namespaces such as runner SPI are intentionally excluded from the supported host-app contract.

## Recommended Naming

Use these names in host app code. They match the SDK mental model and make code easier to read.

- `runtime`: `LxAppRuntime` singleton state
- `controller`: `LxAppController` session lifecycle owner
- `shell`: `LxAppShell` default LingXia chrome
- `hostView`: `LxAppHostView` embedded LingXia content region
- `session`: `LxAppSession` opened app instance
- `config`: `LxAppShellConfiguration` shell layout/style value

Avoid app-side wrapper names like `manager`, `engine`, or `bridge` unless you are wrapping the SDK on purpose.

## Quick Start

Use `Lingxia.quickStart()` when the host wants the default LingXia shell.

```swift
import lingxia

@MainActor
func startApp() {
    Lingxia.enableWebViewDebugging()
    _ = try? Lingxia.quickStart()
}
```

Use `Lingxia.quickStart(configuration:)` when you want the shell, but you also want to control sidebar, toolbar, chrome, and layout.

## Shell Customization

For host apps, the most important advanced API is `LxAppShellConfiguration`.

Use it when you want to control:

- whether the sidebar exists at all
- whether the toolbar exists at all
- sidebar structure
- custom Swift-native sidebar / toolbar providers
- content padding
- corner radius and shadow
- floating panel layout
- macOS traffic-light placement
- shell background colors

### Most Important Fields

```swift
public struct LxAppShellConfiguration: Codable, Sendable {
    public var sidebar: LxAppSidebarMode
    public var toolbar: LxAppToolbarMode
    public var chrome: LxAppChromeStyle
    public var trafficLightPlacement: LxAppTrafficLightPlacement
    public var panelLayout: LxAppFloatingPanelLayout
    public var sidebarBackground: LxAppColor
    public var toolbarBackground: LxAppColor
}
```

Field intent:

- `sidebar`: no sidebar, declarative sidebar, or Swift-native sidebar.
- `toolbar`: no toolbar, declarative toolbar, or Swift-native toolbar.
- `chrome`: corner radius, shadow, and content padding around the main content area.
- `trafficLightPlacement`: macOS traffic-light placement.
- `panelLayout`: floating-panel geometry behavior.
- `sidebarBackground` / `toolbarBackground`: shell chrome colors.

### Common Layout Recipes

Immersive content:

```swift
var config = LxAppShellConfiguration()
config.sidebar = .hidden
config.toolbar = .hidden
config.chrome = .flat
_ = try? Lingxia.quickStart(configuration: config)
```

Standard desktop app shell:

```swift
var config = LxAppShellConfiguration()
config.sidebar = .declarative(mySidebar)
config.toolbar = .declarative(.default)
config.chrome = .init(cornerRadius: 12, hasShadow: true, contentPadding: 8)
config.sidebarBackground = .sidebarBackground
config.toolbarBackground = .toolbarBackground
_ = try? Lingxia.quickStart(configuration: config)
```

Hybrid shell with host-owned sidebar:

```swift
var config = LxAppShellConfiguration()
config.sidebar = .swiftNative(mySidebarHandle)
config.toolbar = .declarative(.default)
config.chrome = .default
_ = try? Lingxia.quickStart(configuration: config)
```

## Local Embedding

Use `LxAppHostView` when the host app wants to build its own window or layout and embed LingXia content inside one region of it.

Examples:

- a macOS split view where the right side is LingXia content
- a custom AppKit / UIKit container window
- a floating panel owned by the host app
- an inspector or tool pane that embeds one lxapp session

```swift
import lingxia

@MainActor
func mountIntoMyOwnWindow(containerView: NSView) async throws {
    _ = try Lingxia.initializeRuntime()

    let controller = LxAppController()
    Lingxia.activate(controller: controller)

    let hostView = LxAppHostView(controller: controller)
    hostView.frame = containerView.bounds
    containerView.addSubview(hostView)

    let session = try await controller.openHomeApp()
    try await hostView.mount(session)
}
```

Recommended rule:

- use one `LxAppController` per host integration flow
- use one `LxAppHostView` per embedded visual region
- keep the host app responsible for window creation, container layout, and resize behavior
- let the SDK own lxapp session lifecycle and webview attachment

## Advanced Runtime Integration

Use `Lingxia.initializeRuntime()` when the host wants to build its own integration without the default shell.

```swift
import lingxia

@MainActor
func startCustomHost() async throws {
    let runtime = try Lingxia.initializeRuntime()
    let controller = LxAppController()
    Lingxia.activate(controller: controller)
    _ = runtime
}
```

This path is appropriate when:

- the host app already has its own navigation model
- the host app already owns windows or scenes
- LingXia should appear only inside one area of the UI
- the host app wants to intercept open / close decisions itself

## API Reference

### `Lingxia`

```swift
@MainActor
public enum Lingxia {
    @discardableResult
    public static func initializeRuntime() throws -> LxAppRuntimeInfo

    public static func activate(controller: LxAppController)

    public static func enableWebViewDebugging()

    public static func handleAppLink(url: URL)

    @discardableResult
    public static func quickStart(
        configuration: LxAppShellConfiguration = .init()
    ) throws -> LxAppShell
}
```

`Lingxia.initialize()` is removed. Hosts should use `quickStart()` or `initializeRuntime()`.

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
    @available(*, deprecated, renamed: "id")
    public var viewId: LxAppHostViewID { get }

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

### `LxAppShell`

```swift
@MainActor
public final class LxAppShell {
    public let controller: LxAppController
    public private(set) var configuration: LxAppShellConfiguration
    public let hostView: LxAppHostView

    public init(
        controller: LxAppController = .init(),
        configuration: LxAppShellConfiguration = .init()
    )

    public func updateConfiguration(_ newConfig: LxAppShellConfiguration)
    public func show()
    public func hide()
}
```

Configuration types:

```swift
public struct LxAppShellConfiguration: Codable, Sendable {
    public var sidebar: LxAppSidebarMode
    public var toolbar: LxAppToolbarMode
    public var chrome: LxAppChromeStyle
    public var trafficLightPlacement: LxAppTrafficLightPlacement
    public var panelLayout: LxAppFloatingPanelLayout
    public var sidebarBackground: LxAppColor
    public var toolbarBackground: LxAppColor
}

public enum LxAppSidebarMode: Sendable, Codable
public enum LxAppToolbarMode: Sendable, Codable
public struct LxAppSidebarTree: Codable, Sendable, Hashable
public struct LxAppSidebarSection: Codable, Sendable, Hashable, Identifiable
public struct LxAppSidebarTab: Codable, Sendable, Hashable, Identifiable
public struct LxAppSidebarHandle: Sendable, Hashable
public protocol LxAppSidebarProviding: AnyObject
public struct LxAppToolbarSpec: Codable, Sendable, Hashable
public struct LxAppToolbarHandle: Sendable, Hashable
public protocol LxAppToolbarProviding: AnyObject
public struct LxAppChromeStyle: Codable, Sendable, Hashable
public struct LxAppColor: Codable, Sendable, Hashable
public struct LxAppFloatingPanelLayout: Codable, Sendable, Hashable
public enum LxAppTrafficLightPlacement: String, Codable, Sendable, CaseIterable
```

### `LxAppRuntime`

```swift
@MainActor
public final class LxAppRuntime {
    public static let shared: LxAppRuntime
    public private(set) var info: LxAppRuntimeInfo? { get }
    public var isInitialized: Bool { get }

    @discardableResult
    public func initialize() throws -> LxAppRuntimeInfo
}
```

Supporting types:

```swift
public struct LxAppRuntimeInfo: Codable, Sendable, Hashable
public struct LxAppCapabilities: OptionSet, Sendable, Codable, Hashable
public enum LxAppRuntimeError: Error, Codable, Sendable
```

## Internal Rust Bridge Contract

The Rust bridge is an internal contract.

- Rust-facing callback names and generated bridge symbols are not host-app API.
- Directory changes on the Swift side do not matter to Rust by themselves.
- Rust only needs updates when bridge function names or bridge signatures change.

In this repository, host-app API and Rust bridge API are intentionally treated as separate contracts.

## Not Shipped

These host-app namespaces are intentionally absent from the current supported contract:

- toast namespace
- dialogs
- popups
- documents
- URL routing facade
- push integration helpers

They may return in a future revision once they have product-level behavior and tests.

## Source Layout

For the Apple SDK source tree:

- `Sources/Runtime`, `Controller`, `HostView`, `Shell`, `Sidebar`, and `Toolbar`
  are the formal host-app API layers.
- `Sources/Capabilities` contains internal implementation for host-facing capabilities.
- `Sources/ShellUI` contains internal shared shell UI implementation that is not host-app API.
- `Sources/Browser` contains internal browser-only helpers.
- `Sources/Runner/SPI` contains runner-only SPI bridge code.
- `Sources/Support` contains internal support helpers.
- `Sources/macOS/Sidebar` and `Sources/macOS/Toolbar` contain macOS-only shell view implementations.
- `Sources/FFI` contains Rust-calls-Swift bridge entry points only.

This split is intentional. Host-app APIs and Rust bridge APIs should not live in the same folder or share the same namespace.
