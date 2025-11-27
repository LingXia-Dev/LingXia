import Foundation
import UIKit
import WebKit
import OSLog
@preconcurrency import ObjectiveC

#if os(iOS)

private let sameLevelLog = OSLog(subsystem: "LingXia", category: "SameLevel")

private struct SameLevelAssociatedKeys {
    nonisolated(unsafe) static var configured: UInt8 = 0
    nonisolated(unsafe) static var manager: UInt8 = 0
}

extension WKWebView {
    fileprivate var lxSameLevelConfigured: Bool {
        get {
            (objc_getAssociatedObject(self, &SameLevelAssociatedKeys.configured) as? Bool) ?? false
        }
        set {
            objc_setAssociatedObject(
                self,
                &SameLevelAssociatedKeys.configured,
                newValue,
                .OBJC_ASSOCIATION_RETAIN_NONATOMIC
            )
        }
    }

    fileprivate var lxSameLevelManager: SameLevelBridge? {
        get {
            objc_getAssociatedObject(self, &SameLevelAssociatedKeys.manager) as? SameLevelBridge
        }
        set {
            objc_setAssociatedObject(
                self,
                &SameLevelAssociatedKeys.manager,
                newValue,
                .OBJC_ASSOCIATION_RETAIN_NONATOMIC
            )
        }
    }
}

/// Bridge between JS component.* messages and native SameLevel components
@MainActor
final class SameLevelBridge: NSObject, WKScriptMessageHandler {
    // Global registry for component factories (built-ins + user-registered)
    private static var registeredFactories: [String: LxNativeComponentFactory] = [:]
    private static var defaultsRegistered = false

    private weak var webView: WKWebView?
    private weak var overlayHost: UIView?
    private var componentManager: SameLevelComponentManager?
    private var pageKey: String
    private var pendingPageKeyUpdate: Bool = false

    static func attachIfNeeded(to webView: WKWebView) {
        if webView.lxSameLevelConfigured {
            os_log("SameLevelBridge already configured for WebView", log: sameLevelLog, type: .info)
            return
        }
        webView.lxSameLevelConfigured = true

        os_log("SameLevelBridge attaching to WebView", log: sameLevelLog, type: .info)

        // Ensure built-in components are registered before installing
        registerDefaultComponents()

        let bridge = SameLevelBridge(webView: webView)
        bridge.install()
        webView.lxSameLevelManager = bridge
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

    private func install() {
        guard let webView = webView else { return }

        // Create overlay host in scrollView content space if not present
        let host = makeOrFindOverlayHost(for: webView)
        overlayHost = host

        let manager = SameLevelComponentManager(
            scrollView: webView.scrollView,
            hostView: host,
            defaultPageId: pageKey,
            eventSink: { [weak self] payload in
                self?.sendEventToJavaScript(payload)
            }
        )

        // Register all known factories (built-ins + user-registered)
        Self.registeredFactories.forEach { type, factory in
            manager.register(type: type, factory: factory)
        }

        componentManager = manager

        // Register script message handler for "SameLevel"
        let controller = webView.configuration.userContentController
        controller.add(self, name: "SameLevel")

        os_log("SameLevelBridge installed for WebView (handler added)", log: sameLevelLog, type: .info)
    }

    private func makeOrFindOverlayHost(for webView: WKWebView) -> UIView {
        // Reuse existing host if any (identified by tag)
        let existing = webView.scrollView.subviews.first(where: { $0.tag == 0x1EAF }) // arbitrary tag
        if let host = existing {
            return host
        }

        let host = SameLevelOverlayHost()
        host.backgroundColor = .clear
        host.isUserInteractionEnabled = true
        host.clipsToBounds = false
        host.layer.zPosition = 1000
        host.tag = 0x1EAF

        let scrollView = webView.scrollView
        scrollView.canCancelContentTouches = false
        scrollView.delaysContentTouches = false
        scrollView.addSubview(host)
        scrollView.bringSubviewToFront(host)

        host.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            host.topAnchor.constraint(equalTo: scrollView.contentLayoutGuide.topAnchor),
            host.leadingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.leadingAnchor),
            host.trailingAnchor.constraint(equalTo: scrollView.contentLayoutGuide.trailingAnchor),
            host.bottomAnchor.constraint(equalTo: scrollView.contentLayoutGuide.bottomAnchor)
        ])

        return host
    }
}

// Custom overlay host that passes through touches to its subviews
private final class SameLevelOverlayHost: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        // Check all subviews first (native components)
        for subview in subviews.reversed() {
            if subview.isHidden || !subview.isUserInteractionEnabled { continue }
            let convertedPoint = convert(point, to: subview)
            if let hit = subview.hitTest(convertedPoint, with: event) {
                return hit
            }
        }
        // Don't consume touches - let them pass through to the web content
        return nil
    }
}

extension SameLevelBridge {
    // MARK: - WKScriptMessageHandler

    func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        guard message.name == "SameLevel" else { return }

        var dict: [String: Any]?

        if let body = message.body as? [String: Any] {
            dict = body
        } else if let json = message.body as? String,
                  let data = json.data(using: .utf8),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            dict = obj
        }

        guard let payload = dict else {
            os_log("SameLevelBridge: unsupported message body %@", log: sameLevelLog, type: .error, String(describing: message.body))
            return
        }

        if let action = payload["action"] as? String,
           let id = payload["id"] as? String {
            os_log("SameLevelBridge handling action=%{public}@ id=%{public}@", log: sameLevelLog, type: .debug, action, id)
        }

        var payloadWithPage = payload
        if payloadWithPage["pageId"] == nil {
            payloadWithPage["pageId"] = pageKey
        }

        componentManager?.handle(message: payloadWithPage)
    }

    // MARK: - JS event delivery

    private func sendEventToJavaScript(_ payload: [String: Any]) {
        guard let webView = webView else { return }

        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
              let eventPayloadJson = String(data: data, encoding: .utf8) else {
            os_log("SameLevelBridge: failed to encode event payload", log: sameLevelLog, type: .error)
            return
        }

        // Construct the full message object expected by __LingXiaRecvMessage
        let fullMessage: [String: Any] = [
            "type": "event",
            "name": "samelevel",
            "payload": payload // Pass the original payload dictionary directly
        ]

        //  Convert the full message object to a JSON string
        guard let fullMessageData = try? JSONSerialization.data(withJSONObject: fullMessage, options: []),
              let fullMessageJsonString = String(data: fullMessageData, encoding: .utf8) else {
            os_log("SameLevelBridge: failed to encode full message", log: sameLevelLog, type: .error)
            return
        }

        // Serialize this STRING as a JSON string literal (to safely embed in JS)
        // Wrapping in an array [str] and serializing gives ["escaped_str"]
        // We then strip the brackets to get "escaped_str"
        guard let safeJsStringData = try? JSONSerialization.data(withJSONObject: [fullMessageJsonString], options: []),
              let safeJsStringWithBrackets = String(data: safeJsStringData, encoding: .utf8) else {
             os_log("SameLevelBridge: failed to escape message string", log: sameLevelLog, type: .error)
             return
        }

        // Remove leading '[' and trailing ']'
        let safeJsLiteral = String(safeJsStringWithBrackets.dropFirst().dropLast())

        let script = """
        (function(){
          if (typeof window.__LingXiaRecvMessage === 'function') {
            try { window.__LingXiaRecvMessage(\(safeJsLiteral)); } catch (e) {}
          } else {
            console.warn('[LingXia SameLevel] __LingXiaRecvMessage not available for SameLevel events');
          }
        })();
        """

        webView.evaluateJavaScript(script, completionHandler: nil)
    }

    /// Register a SameLevel native component factory. Call early (e.g. app launch) before pages load.
    @MainActor
    static func register(type: String, factory: LxNativeComponentFactory) {
        registeredFactories[type] = factory
        os_log("SameLevelBridge registered component type=%{public}@", log: sameLevelLog, type: .info, type)
    }

    @MainActor
    private static func registerDefaultComponents() {
        guard !defaultsRegistered else { return }
        defaultsRegistered = true

        if registeredFactories["video.native"] == nil {
            registeredFactories["video.native"] = VideoComponentFactory()
        }
    }

    private static func makePageKey(for webView: WKWebView) -> String {
        let app = webView.appId ?? "app"
        let path = webView.currentPath ?? "page"
        return "\(app):\(path)"
    }

    // Lifecycle hooks exposed to host
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
        guard let bridge = webView?.lxSameLevelManager else { return }
        bridge.markPageInactive()
    }

    @MainActor
    static func notifyPageActive(for webView: WKWebView?) {
        guard let bridge = webView?.lxSameLevelManager else { return }
        bridge.markPageActive()
    }

    @MainActor
    static func notifyPageDestroyed(for webView: WKWebView?) {
        guard let bridge = webView?.lxSameLevelManager else { return }
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

#endif
