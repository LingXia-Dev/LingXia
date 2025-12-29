import Foundation
import UIKit
import WebKit
import OSLog

private let sameLevelComponentLog = OSLog(subsystem: "LingXia", category: "SameLevel")

#if os(iOS)

// MARK: - Protocols

@MainActor
protocol LxNativeComponent: AnyObject {
    var id: String { get }
    var view: UIView { get }

    func mount(in host: UIView)
    func update(props: [String: Any])
    func setFrame(_ frame: CGRect)
    func focus()
    func blur()
    func handleCommand(name: String, params: [String: Any]?)
    func unmount()
}

@MainActor
protocol LxNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping (_ event: [String: Any]) -> Void
    ) -> LxNativeComponent
}

// MARK: - Component Manager

@MainActor
final class SameLevelComponentManager {
    private weak var scrollView: UIScrollView?
    private weak var hostView: UIView?
    private weak var webView: WKWebView?

    private var components: [String: LxNativeComponent] = [:]
    private var componentPage: [String: String] = [:]
    private var pageComponents: [String: Set<String>] = [:]
    private var componentWKChildScrollView: [String: UIScrollView] = [:]
    // Rust callback IDs for VideoContext event forwarding
    private var componentCallbacks: [String: UInt64] = [:]
    private let defaultPageId: String
    private var factories: [String: LxNativeComponentFactory] = [:]
    private let eventSink: (_ payload: [String: Any]) -> Void

    private var webOverlayCoverageRestore: [String: Bool] = [:]
    private var scrollBounceRestore: (bounces: Bool, alwaysBounceVertical: Bool)? = nil

    init(
        scrollView: UIScrollView,
        hostView: UIView,
        webView: WKWebView,
        defaultPageId: String,
        eventSink: @escaping (_ payload: [String: Any]) -> Void
    ) {
        self.scrollView = scrollView
        self.hostView = hostView
        self.webView = webView
        self.defaultPageId = defaultPageId
        self.eventSink = eventSink
    }

    func register(type: String, factory: LxNativeComponentFactory) {
        factories[type] = factory
    }

    func handle(message: [String: Any]) {
        guard let action = message["action"] as? String else { return }

        let logType: OSLogType = action == "component.update" ? .debug : .info
        if let id = message["id"] as? String {
            os_log("SameLevelComponentManager handle action=%{public}@ id=%{public}@", log: sameLevelComponentLog, type: logType, action, id)
        } else {
            os_log("SameLevelComponentManager handle action=%{public}@", log: sameLevelComponentLog, type: logType, action)
        }

        switch action {
        case "component.mount":
            handleMount(message)
        case "component.update":
            handleUpdate(message)
        case "component.unmount":
            handleUnmount(message)
        case "component.focus":
            handleFocus(message)
        case "component.blur":
            handleBlur(message)
        case "component.command":
            handleCommand(message)
        case "component.coverage":
            handleCoverage(message)
        case "page.lifecycle":
            handlePageLifecycle(message)
        default:
            break
        }
    }

    private func handleMount(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let type = parameters["type"] as? String,
              let rectDict = parameters["rect"] as? [String: Any] else {
            return
        }

        let pageId = resolvePageId(parameters)
        let props = (parameters["props"] as? [String: Any]) ?? [:]
        let zIndex = CGFloat(parameters["zIndex"] as? Double ?? 0)
        let cornerRadius = CGFloat(parameters["cornerRadius"] as? Double ?? 0)

        let rect = rectFrom(dict: rectDict)

        guard components[id] == nil, let factory = factories[type] else {
            os_log("SameLevelComponentManager mount skipped for id=%{public}@ type=%{public}@", log: sameLevelComponentLog, type: .error, id, type)
            return
        }

        let component = factory.make(id: id, initialProps: props) { [weak self] event in
            self?.sendEventToWeb(componentId: id, event: event)
        }
        components[id] = component
        componentPage[id] = pageId
        pageComponents[pageId, default: []].insert(id)
        ComponentRouter.shared.register(componentId: id, manager: self)

        // True same-level rendering: mount in WKChildScrollView when available
        var targetRect = rect
        if let scrollContainerRectDict = parameters["scrollContainerRect"] as? [String: Any],
           let wkChildScrollView = resolveWKChildScrollView(rectFrom(dict: scrollContainerRectDict)) {
            componentWKChildScrollView[id] = wkChildScrollView
            targetRect = wkChildScrollView.bounds

            // Disable scrolling on WKChildScrollView
            wkChildScrollView.isScrollEnabled = false
            wkChildScrollView.delaysContentTouches = false
            wkChildScrollView.canCancelContentTouches = false

            component.mount(in: wkChildScrollView)

            // Register for hit-test passthrough
            WKContentViewHitTestSwizzler.shared.registerNativeView(component.view, in: wkChildScrollView)
        } else if let host = hostView {
            component.mount(in: host)
        } else {
            os_log("Failed to mount component %{public}@ (no container)", log: sameLevelComponentLog, type: .error, id)
            return
        }
        component.setFrame(pixelAligned(targetRect))
        component.update(props: props)
        component.view.layer.cornerRadius = cornerRadius
        if #available(iOS 13.0, *) {
            component.view.layer.cornerCurve = .continuous
        }
        component.view.layer.masksToBounds = true
        component.view.layer.zPosition = zIndex

        if cornerRadius > 0 {
            component.update(props: ["cornerRadius": cornerRadius])
        }
    }

    private func handleUpdate(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let component = components[id] else { return }

        if let rectDict = parameters["rect"] as? [String: Any] {
            if let superview = component.view.superview,
               NSStringFromClass(type(of: superview)).contains("WKChildScrollView") {
                // True same-level: WebKit auto-manages position
                component.setFrame(superview.bounds)
            } else {
                // Fallback overlay mode: use rect from JS
                component.setFrame(pixelAligned(rectFrom(dict: rectDict)))
            }
        }
        if let props = parameters["props"] as? [String: Any] {
            component.update(props: props)
        }
        if let zIndex = parameters["zIndex"] as? Double {
            component.view.layer.zPosition = CGFloat(zIndex)
        }
        if let radius = parameters["cornerRadius"] as? Double {
            component.view.layer.cornerRadius = CGFloat(radius)
            component.view.layer.masksToBounds = true
            component.update(props: ["cornerRadius": radius])
        }
    }

    private func handleUnmount(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              !id.isEmpty else { return }
        let pageId = resolvePageId(parameters)
        unmountComponent(id: id, pageId: pageId)
    }

    private func handleFocus(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let component = components[id] else { return }
        component.focus()
        ensureVisible(component.view.frame)
    }

    private func handleBlur(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let component = components[id] else { return }
        component.blur()
    }

    private func handleCommand(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let name = parameters["name"] as? String,
              let component = components[id] else { return }
        let params = parameters["params"] as? [String: Any]
        component.handleCommand(name: name, params: params)
    }

    // MARK: - Public API for Rust FFI

    /// Set Rust callback ID for a component (used by VideoContext).
    /// Returns true if component exists, false otherwise.
    func setCallback(componentId: String, callbackId: UInt64) -> Bool {
        guard components[componentId] != nil else { return false }
        componentCallbacks[componentId] = callbackId
        return true
    }

    func componentView(componentId: String) -> UIView? {
        return components[componentId]?.view
    }

    func emitComponentEvent(componentId: String, event: String, detail: [String: Any] = [:]) {
        sendEventToWeb(componentId: componentId, event: ["event": event, "detail": detail])
    }

    func setStreamDecoderActive(componentId: String, active: Bool) {
        (components[componentId] as? VideoComponent)?.setStreamDecoderActive(active)
    }

    private func handlePageLifecycle(_ parameters: [String: Any]) {
        let pageId = resolvePageId(parameters)
        guard let state = parameters["state"] as? String else { return }
        switch state {
        case "inactive":
            pausePage(pageId)
        case "active":
            resumePage(pageId)
        case "destroyed":
            unmountPage(pageId)
        default:
            break
        }
    }

    func teardownAll() {
        let allIds = Array(components.keys)
        allIds.forEach { id in
            unmountComponent(id: id, pageId: componentPage[id])
        }
        pageComponents.removeAll()
    }

    // MARK: - Helpers

    private func sendEventToWeb(componentId: String, event: [String: Any]) {
        var payload = event
        payload["action"] = "component.event"
        payload["id"] = componentId
        if let pageId = componentPage[componentId] {
            payload["pageId"] = pageId
        }
        eventSink(payload)

        // Also forward to Rust callback if registered (for VideoContext)
        if let callbackId = componentCallbacks[componentId] {
            var enriched = payload
            enriched["componentId"] = componentId
            if let data = try? JSONSerialization.data(withJSONObject: enriched, options: []),
               let enrichedJson = String(data: data, encoding: .utf8) {
                _ = onCallback(callbackId, true, enrichedJson)
            }
        }
    }

    private func resolvePageId(_ dict: [String: Any]) -> String {
        if let pageId = dict["pageId"] as? String, !pageId.isEmpty {
            return pageId
        }
        return defaultPageId
    }

    private func rectFrom(dict: [String: Any]) -> CGRect {
        let x = CGFloat((dict["x"] as? Double) ?? 0)
        let y = CGFloat((dict["y"] as? Double) ?? 0)
        let w = CGFloat((dict["width"] as? Double) ?? 0)
        let h = CGFloat((dict["height"] as? Double) ?? 0)
        return CGRect(x: x, y: y, width: w, height: h)
    }

    private func pixelAligned(_ rect: CGRect) -> CGRect {
        let scale = UIScreen.main.scale
        let midYpx = (rect.midY * scale).rounded()
        let xpx = (rect.origin.x * scale).rounded()
        let wpx = max(1, (rect.size.width * scale).rounded())
        let hpxBase = max(1, (rect.size.height * scale).rounded())
        let fudgePx: CGFloat = 2
        let hpx = hpxBase + fudgePx
        let ypx = midYpx - (hpx / 2.0)
        return CGRect(x: xpx / scale, y: ypx / scale, width: wpx / scale, height: hpx / scale)
    }

    private func ensureVisible(_ rect: CGRect) {
        guard let scrollView = scrollView else { return }
        scrollView.scrollRectToVisible(rect.insetBy(dx: 0, dy: -20), animated: true)
    }

    private func unmountPage(_ pageId: String) {
        guard let ids = pageComponents.removeValue(forKey: pageId) else { return }
        for id in ids {
            unmountComponent(id: id, pageId: pageId)
        }
    }

    private func pausePage(_ pageId: String) {
        guard let ids = pageComponents[pageId] else { return }
        for id in ids {
            guard let component = components[id] else { continue }
            component.blur()
            component.view.isHidden = true
            component.handleCommand(name: "pause", params: nil)
        }
    }

    private func resumePage(_ pageId: String) {
        guard let ids = pageComponents[pageId] else { return }
        for id in ids {
            guard let component = components[id] else { continue }
            component.view.isHidden = false
            component.focus()
        }
    }

    func dispatchCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        guard let component = components[componentId] else { return false }
        component.handleCommand(name: name, params: params)
        return true
    }

    private func unmountComponent(id: String, pageId: String?) {
        guard let component = components.removeValue(forKey: id) else { return }
        if component is VideoComponent {
            webOverlayCoverageRestore.removeValue(forKey: id)
            updateScrollBounceSuppression()
        }
        // Unregister from hit-test swizzler before unmount
        WKContentViewHitTestSwizzler.shared.unregisterNativeView(component.view)
        component.unmount()
        componentWKChildScrollView.removeValue(forKey: id)
        if let pageId {
            var set = pageComponents[pageId] ?? []
            set.remove(id)
            if set.isEmpty {
                pageComponents.removeValue(forKey: pageId)
            } else {
                pageComponents[pageId] = set
            }
        }
        componentPage.removeValue(forKey: id)
        componentCallbacks.removeValue(forKey: id)
        ComponentRouter.shared.unregister(componentId: id)
    }

    private func handleCoverage(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let covered = parameters["covered"] as? Bool else { return }
        setWebOverlayCoverage(componentId: id, covered: covered)
    }

    func setWebOverlayCoverage(componentId: String, covered: Bool) {
        guard let component = components[componentId] as? VideoComponent else { return }
        let view = component.view

        if covered {
            if webOverlayCoverageRestore[componentId] == nil {
                webOverlayCoverageRestore[componentId] = view.isUserInteractionEnabled
            }
            view.isUserInteractionEnabled = false
            updateScrollBounceSuppression()
            return
        }

        let restore = webOverlayCoverageRestore.removeValue(forKey: componentId)
        if let restore {
            view.isUserInteractionEnabled = restore
        }
        updateScrollBounceSuppression()
    }

    private func updateScrollBounceSuppression() {
        guard let scrollView else { return }
        let suppress = !webOverlayCoverageRestore.isEmpty

        if suppress {
            if scrollBounceRestore == nil {
                scrollBounceRestore = (
                    bounces: scrollView.bounces,
                    alwaysBounceVertical: scrollView.alwaysBounceVertical
                )
            }
            scrollView.bounces = false
            scrollView.alwaysBounceVertical = false
            return
        }

        guard let restore = scrollBounceRestore else { return }
        scrollView.bounces = restore.bounces
        scrollView.alwaysBounceVertical = restore.alwaysBounceVertical
        scrollBounceRestore = nil
    }

    /// Convert window-space rect to WKScrollView coords and locate matching WKChildScrollView.
    private func resolveWKChildScrollView(_ rectInWindow: CGRect) -> UIScrollView? {
        guard let webView = webView else { return nil }
        // Ensure subviews are laid out before matching
        webView.scrollView.layoutIfNeeded()

        let rectInWebView = webView.convert(rectInWindow, from: nil)
        let rectInScroll = webView.scrollView.convert(rectInWebView, from: webView)
        return findChildScrollView(in: webView.scrollView, matching: rectInScroll)
    }

    private func findChildScrollView(in view: UIView,
                                     matching rect: CGRect,
                                     originTolerance: CGFloat = 64.0,
                                     sizeTolerance: CGFloat = 24.0) -> UIScrollView? {
        let className = NSStringFromClass(type(of: view))
        if className.contains("WKChildScrollView"),
           let scrollView = view as? UIScrollView {
            let frame = scrollView.frame
            let originMatch = abs(frame.origin.x - rect.origin.x) < originTolerance &&
                              abs(frame.origin.y - rect.origin.y) < originTolerance
            let sizeMatch = abs(frame.size.width - rect.size.width) < sizeTolerance &&
                            abs(frame.size.height - rect.size.height) < sizeTolerance
            if originMatch && sizeMatch {
                return scrollView
            }
        }
        for subview in view.subviews {
            if let found = findChildScrollView(in: subview,
                                               matching: rect,
                                               originTolerance: originTolerance,
                                               sizeTolerance: sizeTolerance) {
                return found
            }
        }
        return nil
    }
}

/// Swizzles WKContentView's hitTest to allow touch events to pass through to native views in WKChildScrollView.
/// This enables true same-level rendering with working touch interactions.
@MainActor
final class WKContentViewHitTestSwizzler {
    static let shared = WKContentViewHitTestSwizzler()

    private var registeredViews: [ObjectIdentifier: (view: UIView, container: UIScrollView)] = [:]
    private static var swizzled = false

    private init() {
        Self.performSwizzle()
    }

    func registerNativeView(_ view: UIView, in container: UIScrollView) {
        registeredViews[ObjectIdentifier(view)] = (view, container)
        os_log("WKContentViewHitTestSwizzler: registered view %{public}@", log: sameLevelComponentLog, type: .debug, String(describing: view))
    }

    func unregisterNativeView(_ view: UIView) {
        registeredViews.removeValue(forKey: ObjectIdentifier(view))
        os_log("WKContentViewHitTestSwizzler: unregistered view", log: sameLevelComponentLog, type: .debug)
    }

    /// Find registered native view at the given point in WKContentView's coordinate space
    func nativeView(at point: CGPoint, in contentView: UIView) -> UIView? {
        for (_, entry) in registeredViews {
            let view = entry.view
            guard let superview = view.superview else { continue }
            // Convert point to the native view's coordinate space
            let pointInView = contentView.convert(point, to: superview)
            if view.frame.contains(pointInView) && !view.isHidden && view.alpha > 0.01 && view.isUserInteractionEnabled {
                // Return the view that should receive touches
                return view.hitTest(superview.convert(pointInView, to: view), with: nil) ?? view
            }
        }
        return nil
    }

    private static func performSwizzle() {
        guard !swizzled else { return }
        swizzled = true

        guard let wkContentViewClass = NSClassFromString("WKContentView") else {
            os_log("WKContentViewHitTestSwizzler: WKContentView class not found", log: sameLevelComponentLog, type: .error)
            return
        }

        let originalSelector = #selector(UIView.hitTest(_:with:))
        let swizzledSelector = #selector(UIView.lx_swizzled_hitTest(_:with:))

        guard let originalMethod = class_getInstanceMethod(wkContentViewClass, originalSelector),
              let swizzledMethod = class_getInstanceMethod(UIView.self, swizzledSelector) else {
            os_log("WKContentViewHitTestSwizzler: failed to get methods", log: sameLevelComponentLog, type: .error)
            return
        }

        method_exchangeImplementations(originalMethod, swizzledMethod)
        os_log("WKContentViewHitTestSwizzler: swizzle complete", log: sameLevelComponentLog, type: .info)
    }
}

extension UIView {
    @objc func lx_swizzled_hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        if let nativeView = WKContentViewHitTestSwizzler.shared.nativeView(at: point, in: self) {
            return nativeView
        }
        return lx_swizzled_hitTest(point, with: event)
    }
}

#endif
