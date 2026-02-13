#if os(macOS)
import Foundation
import WebKit
import AppKit
@preconcurrency import ObjectiveC

private struct MacNativeComponentAssociatedKeys {
    nonisolated(unsafe) static var configured: UInt8 = 0
    nonisolated(unsafe) static var bridge: UInt8 = 0
}

extension WKWebView {
    fileprivate var lxMacNativeComponentConfigured: Bool {
        get {
            (objc_getAssociatedObject(self, &MacNativeComponentAssociatedKeys.configured) as? Bool) ?? false
        }
        set {
            objc_setAssociatedObject(
                self,
                &MacNativeComponentAssociatedKeys.configured,
                newValue,
                .OBJC_ASSOCIATION_RETAIN_NONATOMIC
            )
        }
    }

    fileprivate var lxMacNativeComponentBridge: MacNativeBridge? {
        get {
            objc_getAssociatedObject(self, &MacNativeComponentAssociatedKeys.bridge) as? MacNativeBridge
        }
        set {
            objc_setAssociatedObject(
                self,
                &MacNativeComponentAssociatedKeys.bridge,
                newValue,
                .OBJC_ASSOCIATION_RETAIN_NONATOMIC
            )
        }
    }
}

@MainActor
final class MacNativeBridge: NSObject, WKScriptMessageHandler {
    private static var registeredFactories: [String: MacNativeComponentFactory] = [:]
    private static var defaultsRegistered = false

    private weak var webView: WKWebView?
    private weak var overlayHost: NSView?
    private var componentManager: MacNativeComponentManager?
    private var pageKey: String

    static func attachIfNeeded(to webView: WKWebView, in container: NSView) {
        if webView.lxMacNativeComponentConfigured {
            if let bridge = webView.lxMacNativeComponentBridge {
                bridge.rebindIfNeeded(in: container)
            } else {
                registerDefaultComponents()
                let bridge = MacNativeBridge(webView: webView)
                bridge.install(in: container)
                webView.lxMacNativeComponentBridge = bridge
            }
            return
        }
        webView.lxMacNativeComponentConfigured = true

        registerDefaultComponents()

        let bridge = MacNativeBridge(webView: webView)
        bridge.install(in: container)
        webView.lxMacNativeComponentBridge = bridge
    }

    private static func ensureOverlayHostOnTop(in container: NSView) {
        guard container.subviews.contains(where: { $0 is MacNativeComponentOverlayHost }) else { return }
        container.sortSubviews({ (a, b, _) -> ComparisonResult in
            let aIsHost = a is MacNativeComponentOverlayHost
            let bIsHost = b is MacNativeComponentOverlayHost
            if aIsHost && !bIsHost { return .orderedDescending }
            if !aIsHost && bIsHost { return .orderedAscending }
            return .orderedSame
        }, context: nil)
    }

    private init(webView: WKWebView) {
        self.webView = webView
        self.pageKey = Self.makePageKey(for: webView)
        super.init()
    }

    deinit {
        if let manager = componentManager {
            Task { @MainActor in
                manager.teardownAll()
            }
        }
    }

    private func install(in container: NSView) {
        guard let webView = webView else { return }

        let host = makeOrFindOverlayHost(in: container)
        Self.ensureOverlayHostOnTop(in: container)
        overlayHost = host

        let manager = MacNativeComponentManager(
            hostView: host,
            webView: webView,
            defaultPageId: pageKey,
            eventSink: { [weak self] payload in
                self?.sendEventToJavaScript(payload)
            }
        )

        Self.registeredFactories.forEach { type, factory in
            manager.register(type: type, factory: factory)
        }

        componentManager = manager

        let controller = webView.configuration.userContentController
        controller.add(self, name: "NativeComponent")

        injectScrollTracker(into: webView)
    }

    private func rebindIfNeeded(in container: NSView) {
        if componentManager == nil {
            install(in: container)
            return
        }

        let host = makeOrFindOverlayHost(in: container)
        Self.ensureOverlayHostOnTop(in: container)

        if overlayHost !== host {
            overlayHost = host
            componentManager?.rebindHostView(host)
        }
    }

    private func injectScrollTracker(into webView: WKWebView) {
        let script = WKUserScript(source: """
        (function(){
          if (window.__lxScrollTrackerInstalled) return;
          window.__lxScrollTrackerInstalled = true;
          var lastX = -1, lastY = -1;
          function send() {
            var x = window.scrollX, y = window.scrollY;
            if (x !== lastX || y !== lastY) {
              lastX = x; lastY = y;
              window.webkit.messageHandlers.NativeComponent.postMessage({
                action: 'scroll.update', scrollX: x, scrollY: y
              });
            }
          }
          window.addEventListener('scroll', send, { passive: true, capture: true });
          send();
        })();
        """, injectionTime: .atDocumentEnd, forMainFrameOnly: true)
        webView.configuration.userContentController.addUserScript(script)
    }

    private func makeOrFindOverlayHost(in container: NSView) -> NSView {
        if let existing = container.subviews.first(where: { $0 is MacNativeComponentOverlayHost }) {
            existing.wantsLayer = true
            existing.layer?.masksToBounds = true
            Self.ensureOverlayHostOnTop(in: container)
            return existing
        }

        let host = MacNativeComponentOverlayHost()
        host.wantsLayer = true
        host.layer?.backgroundColor = NSColor.clear.cgColor
        host.layer?.masksToBounds = true
        host.layer?.zPosition = 1000
        host.translatesAutoresizingMaskIntoConstraints = false

        container.addSubview(host, positioned: .above, relativeTo: container.subviews.last)

        NSLayoutConstraint.activate([
            host.topAnchor.constraint(equalTo: container.topAnchor),
            host.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            host.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            host.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])

        return host
    }

    func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        guard message.name == "NativeComponent" else { return }

        var dict: [String: Any]?
        if let body = message.body as? [String: Any] {
            dict = body
        } else if let json = message.body as? String,
                  let data = json.data(using: .utf8),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            dict = obj
        }

        guard let payload = dict else { return }

        if let action = payload["action"] as? String, action == "scroll.update" {
            let scrollX = CGFloat((payload["scrollX"] as? Double) ?? 0)
            let scrollY = CGFloat((payload["scrollY"] as? Double) ?? 0)
            componentManager?.updateScrollOffset(x: scrollX, y: scrollY)
            return
        }

        var payloadWithPage = payload
        if payloadWithPage["pageId"] == nil {
            payloadWithPage["pageId"] = pageKey
        }

        componentManager?.handle(message: payloadWithPage)
    }

    private func sendEventToJavaScript(_ payload: [String: Any]) {
        guard let webView = webView else { return }

        let fullMessage: [String: Any] = [
            "type": "event",
            "name": "nativecomponent",
            "payload": payload
        ]

        guard let fullMessageData = try? JSONSerialization.data(withJSONObject: fullMessage, options: []),
              let fullMessageJsonString = String(data: fullMessageData, encoding: .utf8) else { return }

        guard let safeJsStringData = try? JSONSerialization.data(withJSONObject: [fullMessageJsonString], options: []),
              let safeJsStringWithBrackets = String(data: safeJsStringData, encoding: .utf8) else { return }

        let safeJsLiteral = String(safeJsStringWithBrackets.dropFirst().dropLast())

        let script = """
        (function(){
          if (typeof window.__LingXiaRecvMessage === 'function') {
            try { window.__LingXiaRecvMessage(\(safeJsLiteral)); } catch (e) {}
          } else {
            console.warn('[LingXia MacNativeComponent] __LingXiaRecvMessage not available');
          }
        })();
        """

        webView.evaluateJavaScript(script, completionHandler: nil)
    }

    @MainActor
    static func register(type: String, factory: MacNativeComponentFactory) {
        registeredFactories[type] = factory
    }

    @MainActor
    private static func registerDefaultComponents() {
        guard !defaultsRegistered else { return }
        defaultsRegistered = true

        if registeredFactories["video.native"] == nil {
            registeredFactories["video.native"] = MacVideoComponentFactory()
        }
    }

    private static func makePageKey(for webView: WKWebView) -> String {
        let app = webView.appId ?? "app"
        let path = webView.currentPath ?? "page"
        return "\(app):\(path)"
    }

    @MainActor
    func markPageInactive() {
        componentManager?.handle(message: [
            "action": "page.lifecycle",
            "state": "inactive",
            "pageId": pageKey
        ])
    }

    @MainActor
    func markPageActive() {
        refreshPageKeyIfNeeded()
        componentManager?.handle(message: [
            "action": "page.lifecycle",
            "state": "active",
            "pageId": pageKey
        ])
    }

    @MainActor
    func markPageDestroyed() {
        refreshPageKeyIfNeeded()
        componentManager?.handle(message: [
            "action": "page.lifecycle",
            "state": "destroyed",
            "pageId": pageKey
        ])
        componentManager?.teardownAll()
    }

    @MainActor
    static func notifyPageInactive(for webView: WKWebView?) {
        guard let bridge = webView?.lxMacNativeComponentBridge else { return }
        bridge.markPageInactive()
    }

    @MainActor
    static func notifyPageActive(for webView: WKWebView?) {
        guard let bridge = webView?.lxMacNativeComponentBridge else { return }
        bridge.markPageActive()
    }

    @MainActor
    static func notifyPageDestroyed(for webView: WKWebView?) {
        guard let bridge = webView?.lxMacNativeComponentBridge else { return }
        bridge.markPageDestroyed()
    }

    private func refreshPageKeyIfNeeded() {
        guard let webView = webView else { return }
        let newKey = Self.makePageKey(for: webView)
        if newKey != pageKey {
            pageKey = newKey
        }
    }
}

private final class MacNativeComponentOverlayHost: NSView {
    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}

#endif
