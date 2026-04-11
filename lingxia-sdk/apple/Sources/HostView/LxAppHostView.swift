import Foundation
import WebKit
import OSLog

#if os(iOS)
import UIKit
public typealias LxAppPlatformView = UIView
#elseif os(macOS)
import AppKit
public typealias LxAppPlatformView = NSView
#endif

/// A view that hosts a single LxApp page via WKWebView.
///
/// `LxAppHostView` owns a WKWebView reference and exposes an
/// `AsyncStream` of events plus a `dispatch(command)` API.
/// It has no chrome (no toolbar, no sidebar) — use `LxAppShell`
/// for the full windowed experience.
///
/// ```swift
/// let controller = LxAppController()
/// let hostView = LxAppHostView(controller: controller)
/// container.addSubview(hostView)
/// let session = try await controller.openHomeApp()
/// try await hostView.mount(session)
///
/// for await event in hostView.events {
///     // handle title changes, navigation requests, etc.
/// }
/// ```
@MainActor
public final class LxAppHostView: LxAppPlatformView {
    private final class WeakHostViewBox {
        weak var value: LxAppHostView?

        init(_ value: LxAppHostView) {
            self.value = value
        }
    }

    private final class NavigationDelegateProxy: NSObject, WKNavigationDelegate {
        nonisolated(unsafe) weak var hostView: LxAppHostView?
        nonisolated(unsafe) weak var forwardedDelegate: (NSObjectProtocol & WKNavigationDelegate)?

        override func responds(to aSelector: Selector!) -> Bool {
            super.responds(to: aSelector) || forwardedDelegate?.responds(to: aSelector) == true
        }

        override func forwardingTarget(for aSelector: Selector!) -> Any? {
            if forwardedDelegate?.responds(to: aSelector) == true {
                return forwardedDelegate
            }
            return super.forwardingTarget(for: aSelector)
        }

        func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
            hostView?.emit(.didStartLoading)
            forwardedDelegate?.webView?(webView, didStartProvisionalNavigation: navigation)
        }

        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            hostView?.emit(.didFinishLoading)
            forwardedDelegate?.webView?(webView, didFinish: navigation)
        }

        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
            hostView?.emit(.didFail(LxAppErrorPayload.from(error)))
            forwardedDelegate?.webView?(webView, didFail: navigation, withError: error)
        }

        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
            hostView?.emit(.didFail(LxAppErrorPayload.from(error)))
            forwardedDelegate?.webView?(webView, didFailProvisionalNavigation: navigation, withError: error)
        }
    }

    private nonisolated(unsafe) static var registry: [LxAppHostViewID: WeakHostViewBox] = [:]

    // MARK: - Properties

    /// Stable identifier for this host view instance.
    public let id = LxAppHostViewID()
    @available(*, deprecated, renamed: "id")
    public var viewId: LxAppHostViewID { id }

    /// Controller driving the sessions rendered by this host view.
    public let controller: LxAppController

    /// The underlying WKWebView. Exposed read-only for advanced use cases.
    public private(set) var webView: WKWebView?

    /// The session currently mounted in this host view.
    public private(set) var mountedSession: LxAppSession?

    /// The app ID currently loaded in this host view.
    public private(set) var appId: String?

    /// The current page path.
    public private(set) var currentPath: String?

    /// Whether the webview can navigate back.
    public private(set) var canGoBack: Bool = false

    private static let log = OSLog(subsystem: "LingXia", category: "LxAppHostView")
    private static let mountRetryDelayNs: UInt64 = 50_000_000
    private static let mountRetryCount = 40
    private var titleObservation: NSKeyValueObservation?
    private var canGoBackObservation: NSKeyValueObservation?
    private var loadingObservation: NSKeyValueObservation?
    private let navigationDelegateProxy = NavigationDelegateProxy()
    private var controllerEventsTask: Task<Void, Never>?

    // MARK: - Events

    /// Continuous stream of host view events. Each call returns an
    /// independent stream (multiple consumers supported).
    public var events: AsyncStream<LxAppHostViewEvent> {
        let id = nextContinuationId
        nextContinuationId += 1
        let (stream, continuation) = AsyncStream.makeStream(of: LxAppHostViewEvent.self)
        continuations[id] = continuation
        continuation.onTermination = { [weak self] _ in
            Task { @MainActor in
                self?.continuations.removeValue(forKey: id)
            }
        }
        return stream
    }

    private var continuations: [Int: AsyncStream<LxAppHostViewEvent>.Continuation] = [:]
    private var nextContinuationId = 0

    // MARK: - Init

    public init(controller: LxAppController, frame: CGRect = .zero) {
        self.controller = controller
        super.init(frame: frame)
        Self.registry[id] = WeakHostViewBox(self)
        navigationDelegateProxy.hostView = self
        observeControllerEvents()
    }

    @available(*, unavailable, message: "Use init(controller:frame:) instead.")
    public override init(frame: CGRect) {
        self.controller = LxAppController()
        super.init(frame: frame)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("LxAppHostView does not support Interface Builder")
    }

    deinit {
        Self.registry.removeValue(forKey: id)
        MainActor.assumeIsolated {
            clearEventObservers()
            controllerEventsTask?.cancel()
        }
    }

    // MARK: - Attach

    /// Attach an existing WKWebView to this host view.
    ///
    /// The host view takes ownership of layout. The WKWebView is added
    /// as a subview and resized to fill bounds.
    public func attach(_ wv: WKWebView, appId: String? = nil, path: String? = nil) {
        clearEventObservers()
        self.webView?.removeFromSuperview()
        self.webView = wv
        self.appId = appId ?? wv.appId
        self.currentPath = path ?? wv.currentPath
        self.canGoBack = wv.canGoBack
        if let appId = self.appId,
           let path = self.currentPath,
           let session = controller.session(forAppId: appId),
           session.path == path {
            self.mountedSession = session
        }

        WebViewManager.attachWebViewToContainer(wv, container: self)
        setupEventObservers(for: wv)
        emit(.didChangeTitle(wv.title))
        emit(.didUpdateCanGoBack(canGoBack))
        if wv.isLoading {
            emit(.didStartLoading)
        }
    }

    /// Mount an existing controller session into this host view.
    public func mount(sessionId: LxAppSessionID) async throws {
        guard let session = controller.sessions[sessionId] else {
            throw LxAppErrorPayload(
                code: "session_not_found",
                message: "No controller session exists for \(sessionId.rawValue)"
            )
        }

        for _ in 0..<Self.mountRetryCount {
            if let webView = WebViewManager.findWebView(
                appId: session.appId,
                path: session.path,
                sessionId: session.id.rawValue
            ) {
                mountedSession = session
                attach(webView, appId: session.appId, path: session.path)
                return
            }
            try await Task.sleep(nanoseconds: Self.mountRetryDelayNs)
        }

        throw LxAppErrorPayload(
            code: "webview_not_ready",
            message: "Timed out waiting for the session webview to be ready",
            details: .object([
                "appId": .string(session.appId),
                "path": .string(session.path),
                "sessionId": .number(Double(session.id.rawValue)),
            ])
        )
    }

    /// Mount an already opened session into this host view.
    public func mount(_ session: LxAppSession) async throws {
        try await mount(sessionId: session.id)
    }

    /// Unmount the current session from this host view.
    public func unmount() {
        clearEventObservers()
        webView?.removeFromSuperview()
        webView = nil
        mountedSession = nil
        appId = nil
        currentPath = nil
        canGoBack = false
    }

    // MARK: - Commands

    /// Dispatch a command to this host view.
    public func dispatch(_ command: LxAppHostViewCommand) {
        switch command {
        case .reload:
            webView?.reload()
        case .goBack:
            webView?.goBack()
        case .scrollToTop:
            #if os(iOS)
            webView?.scrollView.setContentOffset(.zero, animated: true)
            #elseif os(macOS)
            webView?.evaluateJavaScript("window.scrollTo(0,0)", completionHandler: nil)
            #endif
        case .triggerCapsuleAction(let action):
            guard let appId else { return }
            let _ = onLxappEvent(appId, LxAppEvent.capsuleClick, action.rawValue)
        }
    }

    // MARK: - Internal event emission

    /// Emit an event to all active consumers.
    internal func emit(_ event: LxAppHostViewEvent) {
        for (_, c) in continuations {
            c.yield(event)
        }
    }

    internal static func resolve(id: LxAppHostViewID) -> LxAppHostView? {
        if let hostView = registry[id]?.value {
            return hostView
        }
        registry.removeValue(forKey: id)
        return nil
    }

    private func observeControllerEvents() {
        controllerEventsTask = Task { [weak self, controller] in
            for await event in controller.events {
                guard let self else { return }
                switch event {
                case .didNavigate(let sessionId, _):
                    guard mountedSession?.id == sessionId else { continue }
                    try? await mount(sessionId: sessionId)
                case .didClose(let session):
                    guard mountedSession?.id == session.id else { continue }
                    unmount()
                default:
                    continue
                }
            }
        }
    }

    private func setupEventObservers(for webView: WKWebView) {
        titleObservation = webView.observe(\.title, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor [weak self] in
                self?.emit(.didChangeTitle(webView.title))
            }
        }

        canGoBackObservation = webView.observe(\.canGoBack, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                self.canGoBack = webView.canGoBack
                self.emit(.didUpdateCanGoBack(webView.canGoBack))
            }
        }

        loadingObservation = webView.observe(\.isLoading, options: [.new]) { [weak self] webView, _ in
            Task { @MainActor [weak self] in
                guard let self else { return }
                if webView.isLoading {
                    self.emit(.didStartLoading)
                } else {
                    self.emit(.didFinishLoading)
                }
            }
        }

        if let existingDelegate = webView.navigationDelegate,
           (existingDelegate as AnyObject) !== navigationDelegateProxy {
            navigationDelegateProxy.forwardedDelegate = existingDelegate
        } else {
            navigationDelegateProxy.forwardedDelegate = nil
        }
        webView.navigationDelegate = navigationDelegateProxy
    }

    private func clearEventObservers() {
        titleObservation?.invalidate()
        canGoBackObservation?.invalidate()
        loadingObservation?.invalidate()
        titleObservation = nil
        canGoBackObservation = nil
        loadingObservation = nil

        if let webView,
           webView.navigationDelegate === navigationDelegateProxy {
            webView.navigationDelegate = navigationDelegateProxy.forwardedDelegate
        }
        navigationDelegateProxy.forwardedDelegate = nil
    }

    // MARK: - Layout

    #if os(macOS)
    public override func layout() {
        super.layout()
        webView?.frame = bounds
    }
    #else
    public override func layoutSubviews() {
        super.layoutSubviews()
        webView?.frame = bounds
    }
    #endif
}

private extension LxAppErrorPayload {
    static func from(_ error: Error) -> LxAppErrorPayload {
        if let payload = error as? LxAppErrorPayload {
            return payload
        }

        let nsError = error as NSError
        return LxAppErrorPayload(
            code: nsError.domain,
            message: nsError.localizedDescription
        )
    }
}
