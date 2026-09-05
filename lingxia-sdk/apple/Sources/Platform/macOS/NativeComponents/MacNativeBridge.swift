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
    private let surfaceBinding: NativeComponentSurfaceBinding
    private var active = true

    static func attachIfNeeded(to webView: WKWebView, in container: NSView) {
        guard case .lxAppPage(let requestedBinding) = webView.nativeComponentSurfaceBinding else {
            webView.lxMacNativeComponentBridge?.invalidate()
            webView.lxMacNativeComponentBridge = nil
            webView.lxMacNativeComponentConfigured = false
            return
        }
        if webView.lxMacNativeComponentConfigured {
            if let bridge = webView.lxMacNativeComponentBridge {
                if bridge.surfaceBinding == .lxAppPage(requestedBinding) {
                    bridge.rebindIfNeeded(in: container)
                    return
                }
                bridge.invalidate()
            } else {
                registerDefaultComponents()
                let bridge = MacNativeBridge(webView: webView, surfaceBinding: .lxAppPage(requestedBinding))
                bridge.install(in: container)
                webView.lxMacNativeComponentBridge = bridge
                return
            }
        }
        webView.lxMacNativeComponentConfigured = true

        registerDefaultComponents()

        let bridge = MacNativeBridge(webView: webView, surfaceBinding: .lxAppPage(requestedBinding))
        bridge.install(in: container)
        webView.lxMacNativeComponentBridge = bridge
    }

    /// Offset the overlay the same way the page is offset, so native
    /// components stay registered with the content they sit on while a pull is
    /// open. Nothing to do for a container that has no components.
    static func setOverlayOffset(_ offset: CGFloat, in container: NSView) {
        guard let host = container.subviews
            .compactMap({ $0 as? MacNativeComponentOverlayHost }).first
        else { return }
        host.verticalOffset = offset
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

    private init(webView: WKWebView, surfaceBinding: NativeComponentSurfaceBinding) {
        self.webView = webView
        self.pageKey = Self.makePageKey(for: webView)
        self.surfaceBinding = surfaceBinding
        super.init()
    }

    deinit {
        if let manager = componentManager {
            Task { @MainActor in
                manager.teardownAll()
            }
        }
    }

    private func invalidate() {
        active = false
        webView?.configuration.userContentController.removeScriptMessageHandler(forName: "NativeComponent")
        componentManager?.teardownAll()
        componentManager = nil
    }

    private func bindingIsCurrent(isMainFrame: Bool) -> Bool {
        guard active, let webView else { return false }
        let currentIdentity: UInt?
        if case .lxAppPage(let binding) = surfaceBinding {
            let pointer = lingxia.findWebViewByPageInstanceId(binding.pageInstanceID)
            currentIdentity = pointer == 0 ? nil : pointer
        } else {
            currentIdentity = nil
        }
        let currentBinding = webView.nativeComponentSurfaceBinding
        let currentPageInstanceID: String?
        let currentGeneration: UInt64?
        if case .lxAppPage(let binding) = currentBinding {
            currentPageInstanceID = binding.pageInstanceID
            currentGeneration = binding.attachmentGeneration
        } else {
            currentPageInstanceID = nil
            currentGeneration = nil
        }
        return surfaceBinding.admits(
            isMainFrame: isMainFrame,
            currentPageInstanceID: currentPageInstanceID,
            currentWebViewIdentity: currentIdentity,
            currentAttachmentGeneration: currentGeneration
        )
    }

    private func admits(_ message: WKScriptMessage) -> Bool {
        bindingIsCurrent(isMainFrame: message.frameInfo.isMainFrame)
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
        // Two consumers, one answer: the component layer's scroll offset, and
        // whether a pull-to-refresh gesture may start. The gesture itself is
        // read natively; only "is anything scrolled" has to come from the page,
        // because WebKit owns scrolling and never reports its position.
        //
        // `atTop` walks the scrollable ancestors of the element under the
        // pointer rather than trusting the document: a page whose content lives
        // in its own scroller leaves `window.scrollY` at 0 forever, and a pull
        // must not arm while the thing being scrolled is part-way down.
        let script = WKUserScript(source: """
        (function(){
          if (window.__lxScrollTrackerInstalled) return;
          window.__lxScrollTrackerInstalled = true;
          var lastX = -1, lastY = -1, lastTop = null, scroller = null;
          var pointerX = -1, pointerY = -1;
          function chainAtTop(node) {
            while (node && node !== document) {
              if (node.scrollTop > 0) return false;
              node = node.parentElement;
            }
            return true;
          }
          function atTop() {
            if (window.scrollY > 0) return false;
            // Resolve the element under the pointer now rather than caching it:
            // a trackpad scrolls without moving the pointer, so a cached node
            // outlives the list that recycled or re-rendered it and would
            // answer for a chain that is no longer in the document.
            var under = pointerX >= 0 ? document.elementFromPoint(pointerX, pointerY) : null;
            // The element under the pointer decides, not whichever element
            // scrolled last: a page with a scrolled list beside a horizontal
            // strip would otherwise report the strip's position for the list.
            return chainAtTop(under || scroller);
          }
          function send() {
            var x = window.scrollX, y = window.scrollY, top = atTop();
            if (x !== lastX || y !== lastY || top !== lastTop) {
              lastX = x; lastY = y; lastTop = top;
              window.webkit.messageHandlers.NativeComponent.postMessage({
                action: 'scroll.update', scrollX: x, scrollY: y, atTop: top
              });
            }
          }
          window.addEventListener('mousemove', function(event) {
            if (event.clientX === pointerX && event.clientY === pointerY) return;
            pointerX = event.clientX;
            pointerY = event.clientY;
            send();
          }, { passive: true, capture: true });
          window.addEventListener('scroll', function(event) {
            scroller = event.target;
            send();
          }, { passive: true, capture: true });
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

        let top = host.topAnchor.constraint(equalTo: container.topAnchor)
        let bottom = host.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        host.top = top
        host.bottom = bottom
        NSLayoutConstraint.activate([
            top,
            host.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            host.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            bottom
        ])

        return host
    }

    func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        guard message.name == "NativeComponent" else { return }
        guard admits(message) else { return }

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
            if let atTop = payload["atTop"] as? Bool {
                webView?.lxPullToRefreshController?.setAtTop(atTop)
            }
            return
        }

        var payloadWithPage = payload
        payloadWithPage["pageId"] = pageKey

        componentManager?.handle(message: payloadWithPage)
    }

    private func sendEventToJavaScript(_ payload: [String: Any]) {
        guard let webView = webView else { return }
        guard bindingIsCurrent(isMainFrame: true) else { return }
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
          try { window.__LingXiaRecvMessage(\(safeJsLiteral)); } catch (e) {}
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
        if registeredFactories["media-swiper.native"] == nil {
            registeredFactories["media-swiper.native"] = MacMediaSwiperComponentFactory()
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
        // WebView is being torn down; release everything once to avoid duplicate destroy paths.
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
    nonisolated override var isFlipped: Bool { true }

    /// Kept with the page while pull-to-refresh holds it open. Constraints
    /// rather than a layer transform, so hit-testing follows the pixels.
    var top: NSLayoutConstraint?
    var bottom: NSLayoutConstraint?
    var verticalOffset: CGFloat = 0 {
        didSet {
            guard verticalOffset != oldValue else { return }
            top?.constant = verticalOffset
            bottom?.constant = verticalOffset
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }
}

#endif
