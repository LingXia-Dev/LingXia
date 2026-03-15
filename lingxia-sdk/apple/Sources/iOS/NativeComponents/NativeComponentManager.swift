import Foundation
import WebKit
import OSLog
import CLingXiaRustAPI

private let nativeComponentLog = OSLog(subsystem: "LingXia", category: "NativeComponent")

#if os(iOS)
import UIKit

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
final class NativeComponentManager {
    private weak var scrollView: UIScrollView?
    private weak var hostView: UIView?
    private weak var webView: WKWebView?

    private var components: [String: LxNativeComponent] = [:]
    private var componentTypes: [String: String] = [:]
    private var componentPage: [String: String] = [:]
    private var componentPageFuncBindings: [String: [String: String]] = [:]
    private var componentDataset: [String: [String: Any]] = [:]
    private var readyComponentIds: Set<String> = []
    private var pendingEventsByComponent: [String: [[String: Any]]] = [:]
    private var pageComponents: [String: Set<String>] = [:]
    // Monotonic generation per component id. Used to drop stale async events from old instances.
    private var componentEpochs: [String: UInt64] = [:]
    private var lastAppliedViewportRect: [String: CGRect] = [:]
    private let frameEpsilon: CGFloat = 0.5
    // Rust callback IDs for VideoContext event forwarding
    private var componentCallbacks: [String: UInt64] = [:]
    private let defaultPageId: String
    private var factories: [String: LxNativeComponentFactory] = [:]
    private let eventSink: (_ payload: [String: Any]) -> Void

    private var webOverlayCoverageRestore: [String: Bool] = [:]
    private var scrollBounceRestore: (bounces: Bool, alwaysBounceVertical: Bool)? = nil
    private let inactivePageStopDelayNs: UInt64 = 60_000_000_000
    private let maxPendingNativeEventsPerComponent: Int = 8
    private var pageInactiveStopTasks: [String: Task<Void, Never>] = [:]
    private var pageInactiveStopGeneration: [String: UInt64] = [:]
    private var componentPlaybackIntent: [String: Bool] = [:]
    private var componentsPendingAutoResume: Set<String> = []
    private var inactivePages: Set<String> = []

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

        switch action {
        case "component.mount":
            handleMount(message)
        case "component.update":
            handleUpdate(message)
        case "component.unmount":
            handleUnmount(message)
        case "component.ready":
            handleReady(message)
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

    private func handleReady(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String, !id.isEmpty else { return }
        readyComponentIds.insert(id)
        guard let pending = pendingEventsByComponent.removeValue(forKey: id) else { return }
        for payload in pending {
            eventSink(payload)
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
            os_log("NativeComponentManager mount skipped for id=%{public}@ type=%{public}@", log: nativeComponentLog, type: .error, id, type)
            return
        }

        let nextEpoch = (componentEpochs[id] ?? 0) + 1
        componentEpochs[id] = nextEpoch
        componentTypes[id] = type
        let component = factory.make(id: id, initialProps: props) { [weak self] event in
            guard let self else { return }
            // Guard against stale events from a previously unmounted instance that shared the same id.
            guard self.componentEpochs[id] == nextEpoch else { return }
            guard self.components[id] != nil else { return }
            self.dispatchComponentEvent(componentId: id, event: event)
        }
        components[id] = component
        componentPage[id] = pageId
        if let bindings = parsePageFuncBindings(props), !bindings.isEmpty {
            componentPageFuncBindings[id] = bindings
        }
        if let dataset = parseDataset(props), !dataset.isEmpty {
            componentDataset[id] = dataset
        }
        pageComponents[pageId, default: []].insert(id)
        ComponentRouter.shared.register(componentId: id, manager: self)

        // Preferred path on iOS: mount into WKChildScrollView for same-level behavior.
        // Only media components (video) use this path; input/textarea use overlay so
        // that WKChildScrollView mis-matching never puts them at the wrong position.
        //
        // JS measureElement sends document coordinates (viewport + window.scrollY), so use
        // the rect directly as content coordinates — do NOT add contentOffset again.
        var targetRect = rect
        var mountedInChildScrollView = false
        if prefersSameLevelMounting(type: type),
           let scrollContainerRectDict = parameters["scrollContainerRect"] as? [String: Any] {
            let scrollContainerRect = rectFrom(dict: scrollContainerRectDict)
            if let wkChildScrollView = resolveWKChildScrollView(scrollContainerRect) {
                targetRect = sameLevelContainerFrame(in: wkChildScrollView)
                mountedInChildScrollView = true

                prepareSameLevelScrollView(wkChildScrollView)

                component.mount(in: wkChildScrollView)
                WKContentViewHitTestSwizzler.shared.registerNativeView(component.view, in: wkChildScrollView)
                os_log("NativeComponent %{public}@ mounted in WKChildScrollView", log: nativeComponentLog, type: .info, id)
            } else {
                os_log("NativeComponent %{public}@ fallback to overlay (WKChildScrollView not found)", log: nativeComponentLog, type: .info, id)
            }
        }

        if !mountedInChildScrollView {
            guard let host = hostView else {
                os_log("Failed to mount component %{public}@ (no container)", log: nativeComponentLog, type: .error, id)
                return
            }
            component.mount(in: host)
            // Register for touch routing: WebKit may add WKChildScrollViews on top of our overlay
            // host at any time, intercepting touches. The swizzler on WKContentView.hitTest lets
            // us reclaim taps at the correct content-space position.
            if let sv = scrollView {
                WKContentViewHitTestSwizzler.shared.registerNativeView(component.view, in: sv)
            }
            os_log("NativeComponent %{public}@ mounted in overlay", log: nativeComponentLog, type: .info, id)
        }

        let alignedTargetRect = stablePixelAligned(targetRect)
        component.setFrame(alignedTargetRect)
        lastAppliedViewportRect[id] = alignedTargetRect
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

        if prefersSameLevelMounting(type: componentTypes[id] ?? ""),
           let scrollContainerRectDict = parameters["scrollContainerRect"] as? [String: Any] {
            let scrollContainerRect = rectFrom(dict: scrollContainerRectDict)
            promoteToWKChildScrollViewIfAvailable(componentId: id, component: component, scrollContainerRect: scrollContainerRect)
        }

        if let rectDict = parameters["rect"] as? [String: Any] {
            let viewportRect = rectFrom(dict: rectDict)
            if let superview = component.view.superview,
               NSStringFromClass(type(of: superview)).contains("WKChildScrollView") {
                // Same-level: container tracks scrolling; only keep bounds-sized frame.
                let childBounds = stablePixelAligned(sameLevelContainerFrame(in: superview))
                if let last = lastAppliedViewportRect[id], rectDistance(last, childBounds) <= frameEpsilon {
                    // no-op
                } else {
                    component.setFrame(childBounds)
                    lastAppliedViewportRect[id] = childBounds
                }
            } else {
                applyViewportFrame(componentId: id, component: component, rect: viewportRect)
            }
        }
        if let props = parameters["props"] as? [String: Any] {
            if props["pageFuncBindings"] != nil {
                let parsed = parsePageFuncBindings(props) ?? [:]
                if parsed.isEmpty {
                    componentPageFuncBindings.removeValue(forKey: id)
                } else {
                    componentPageFuncBindings[id] = parsed
                }
            }
            if props["dataset"] != nil {
                let parsed = parseDataset(props) ?? [:]
                if parsed.isEmpty {
                    componentDataset.removeValue(forKey: id)
                } else {
                    componentDataset[id] = parsed
                }
            }
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

    private func promoteToWKChildScrollViewIfAvailable(
        componentId: String,
        component: LxNativeComponent,
        scrollContainerRect: CGRect
    ) {
        guard let wkChildScrollView = resolveWKChildScrollView(scrollContainerRect) else {
            return
        }

        if component.view.superview === wkChildScrollView {
            return
        }

        if let currentSuperview = component.view.superview,
           NSStringFromClass(type(of: currentSuperview)).contains("WKChildScrollView") {
            // Once mounted in any WKChildScrollView, avoid reparent thrash caused by
            // fluctuating candidate matches during scroll/layout.
            return
        }

        if component.view.superview != nil {
            WKContentViewHitTestSwizzler.shared.unregisterNativeView(component.view)
            component.view.removeFromSuperview()
        }

        prepareSameLevelScrollView(wkChildScrollView)

        component.mount(in: wkChildScrollView)
        WKContentViewHitTestSwizzler.shared.registerNativeView(component.view, in: wkChildScrollView)

        let childBounds = stablePixelAligned(sameLevelContainerFrame(in: wkChildScrollView))
        component.setFrame(childBounds)
        lastAppliedViewportRect[componentId] = childBounds
        os_log("NativeComponent %{public}@ promoted to WKChildScrollView", log: nativeComponentLog, type: .info, componentId)
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
        if shouldEnsureVisibleOnFocus(componentId: id) {
            ensureVisible(component.view.frame, in: component.view.superview)
        }
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
        switch name {
        case "play":
            componentPlaybackIntent[id] = true
        case "pause", "stop":
            componentPlaybackIntent[id] = false
            componentsPendingAutoResume.remove(id)
        default:
            break
        }
        let params = parameters["params"] as? [String: Any]
        component.handleCommand(name: name, params: params)
    }

    // MARK: - Public API for Rust FFI

    /// Set Rust callback ID for a component (used by VideoContext).
    /// Returns true once stored (component may not exist yet).
    func setCallback(componentId: String, callbackId: UInt64) -> Bool {
        componentCallbacks[componentId] = callbackId
        return true
    }

    func componentView(componentId: String) -> UIView? {
        return components[componentId]?.view
    }

    func emitComponentEvent(componentId: String, event: String, detail: [String: Any] = [:]) {
        if event == "waiting" || event == "playrequest" || event == "playing" || event == "pause" || event == "stop" || event == "ended" {
            (components[componentId] as? VideoComponent)?.handleStreamDecoderEvent(event)
        }
        dispatchComponentEvent(componentId: componentId, event: ["event": event, "detail": detail])
    }

    func setStreamDecoderActive(componentId: String, active: Bool) {
        (components[componentId] as? VideoComponent)?.setStreamDecoderActive(active)
    }

    private func handlePageLifecycle(_ parameters: [String: Any]) {
        let pageId = resolvePageId(parameters)
        guard let state = parameters["state"] as? String else { return }
        switch state {
        case "inactive":
            if inactivePages.insert(pageId).inserted {
                pausePage(pageId)
            }
        case "active":
            inactivePages.remove(pageId)
            resumePage(pageId)
        case "destroyed":
            inactivePages.remove(pageId)
            unmountPage(pageId)
        default:
            break
        }
    }

    func teardownAll() {
        cancelAllInactivePageStops()
        inactivePages.removeAll()
        componentsPendingAutoResume.removeAll()
        componentPlaybackIntent.removeAll()
        let allIds = Array(components.keys)
        allIds.forEach { id in
            unmountComponent(id: id, pageId: componentPage[id])
        }
        pageComponents.removeAll()
    }

    // MARK: - Helpers

    private func dispatchComponentEvent(componentId: String, event: [String: Any]) {
        guard let component = components[componentId] else { return }
        var payload = event
        if let eventName = payload["event"] as? String,
           eventName == "focus",
           shouldEnsureVisibleOnFocus(componentId: componentId) {
            ensureVisible(component.view.frame, in: component.view.superview)
        }
        updatePlaybackIntent(componentId: componentId, event: payload["event"] as? String)
        payload["action"] = "component.event"
        payload["id"] = componentId
        payload["componentId"] = componentId
        if let pageId = componentPage[componentId] {
            payload["pageId"] = pageId
        }
        emitEventToView(componentId: componentId, payload: payload)
        dispatchPageFunc(componentId: componentId, payload: payload)

        // Also forward to Rust callback if registered (for VideoContext)
        let eventName = payload["event"] as? String
        let shouldForwardToCallback: Bool = {
            switch eventName {
            case "waiting", "playrequest", "playing", "pause", "stop", "ended", "error", "seeked", "seeking":
                return true
            default:
                return false
            }
        }()

        if shouldForwardToCallback, let callbackId = componentCallbacks[componentId] {
            if let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
               let enrichedJson = String(data: data, encoding: .utf8) {
                os_log(
                    "NativeComponent callback event componentId=%{public}@ event=%{public}@ callbackId=%{public}@",
                    log: nativeComponentLog,
                    type: .debug,
                    componentId,
                    String(payload["event"] as? String ?? ""),
                    String(callbackId)
                )
                _ = onCallback(callbackId, true, enrichedJson)
            }
        }
    }

    private func emitEventToView(componentId: String, payload: [String: Any]) {
        if readyComponentIds.contains(componentId) {
            eventSink(payload)
            return
        }
        var queue = pendingEventsByComponent[componentId] ?? []
        queue.append(payload)
        if queue.count > maxPendingNativeEventsPerComponent {
            queue.removeFirst(queue.count - maxPendingNativeEventsPerComponent)
        }
        pendingEventsByComponent[componentId] = queue
    }

    private func resolvePageId(_ dict: [String: Any]) -> String {
        if let pageId = dict["pageId"] as? String, !pageId.isEmpty {
            return pageId
        }
        return defaultPageId
    }

    private func parsePageFuncBindings(_ props: [String: Any]) -> [String: String]? {
        var bindings: [String: String] = [:]
        if let raw = props["pageFuncBindings"] as? [String: Any] {
            for (event, value) in raw {
                let eventKey = event.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
                guard !eventKey.isEmpty else { continue }
                guard let fn = value as? String else { continue }
                let fnName = fn.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !fnName.isEmpty else { continue }
                bindings[eventKey] = fnName
            }
        }
        if let rawJson = props["pageFuncBindingsJson"] as? String,
           let data = rawJson.data(using: .utf8),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            for (event, value) in json {
                let eventKey = event.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
                guard !eventKey.isEmpty else { continue }
                guard let fn = value as? String else { continue }
                let fnName = fn.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !fnName.isEmpty else { continue }
                bindings[eventKey] = fnName
            }
        }
        return bindings.isEmpty ? nil : bindings
    }

    private func parseDataset(_ props: [String: Any]) -> [String: Any]? {
        var dataset: [String: Any] = [:]
        if let raw = props["dataset"] as? [String: Any] {
            for (key, value) in raw {
                let normalized = key.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !normalized.isEmpty else { continue }
                dataset[normalized] = value
            }
        }
        if let rawJson = props["datasetJson"] as? String,
           let data = rawJson.data(using: .utf8),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            for (key, value) in json {
                let normalized = key.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !normalized.isEmpty else { continue }
                dataset[normalized] = value
            }
        }
        return dataset.isEmpty ? nil : dataset
    }

    private func parsePageId(_ pageId: String) -> (appid: String, path: String)? {
        guard let separator = pageId.firstIndex(of: ":") else { return nil }
        let appId = String(pageId[..<separator])
        let path = String(pageId[pageId.index(after: separator)...])
        guard !appId.isEmpty, !path.isEmpty else { return nil }
        return (appid: appId, path: path)
    }

    private func buildPageEvent(componentId: String, eventName: String, payload: [String: Any]) -> [String: Any] {
        let detail = payload["detail"] ?? [String: Any]()
        let dataset = componentDataset[componentId] ?? [:]
        let target: [String: Any] = [
            "id": componentId,
            "dataset": dataset
        ]
        return [
            "type": eventName,
            "detail": detail,
            "target": target,
            "currentTarget": target,
            "timeStamp": Int(Date().timeIntervalSince1970 * 1000)
        ]
    }

    private func dispatchPageFunc(componentId: String, payload: [String: Any]) {
        guard let eventName = (payload["event"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(),
              !eventName.isEmpty else { return }
        guard let bindings = componentPageFuncBindings[componentId],
              !bindings.isEmpty else {
            return
        }
        guard let pageId = componentPage[componentId],
              let route = parsePageId(pageId) else {
            os_log(
                "NativeComponent drop event: invalid pageId componentId=%{public}@",
                log: nativeComponentLog,
                type: .error,
                componentId
            )
            return
        }
        let pageEvent = buildPageEvent(componentId: componentId, eventName: eventName, payload: payload)
        guard let data = try? JSONSerialization.data(withJSONObject: pageEvent, options: []),
              let payloadJson = String(data: data, encoding: .utf8) else {
            os_log(
                "NativeComponent drop event: payload encode failed componentId=%{public}@ event=%{public}@",
                log: nativeComponentLog,
                type: .error,
                componentId,
                eventName
            )
            return
        }
        guard let bindingsData = try? JSONSerialization.data(withJSONObject: bindings, options: []),
              let bindingsJson = String(data: bindingsData, encoding: .utf8) else {
            os_log(
                "NativeComponent drop event: bindings encode failed componentId=%{public}@ event=%{public}@",
                log: nativeComponentLog,
                type: .error,
                componentId,
                eventName
            )
            return
        }
        _ = dispatchPageFuncToRust(
            appid: route.appid,
            path: route.path,
            componentId: componentId,
            eventName: eventName,
            payloadJson: payloadJson,
            bindingsJson: bindingsJson
        )
    }

    private func dispatchPageFuncToRust(
        appid: String,
        path: String,
        componentId: String,
        eventName: String,
        payloadJson: String,
        bindingsJson: String
    ) -> Bool {
        payloadJson.toRustStr { payloadAsRustStr in
            bindingsJson.toRustStr { bindingsAsRustStr in
                eventName.toRustStr { eventNameAsRustStr in
                    componentId.toRustStr { componentIdAsRustStr in
                        path.toRustStr { pathAsRustStr in
                            appid.toRustStr { appidAsRustStr in
                                __swift_bridge__$dispatch_native_component_event(
                                    appidAsRustStr,
                                    pathAsRustStr,
                                    componentIdAsRustStr,
                                    eventNameAsRustStr,
                                    payloadAsRustStr,
                                    bindingsAsRustStr
                                )
                            }
                        }
                    }
                }
            }
        }
    }

    private func rectFrom(dict: [String: Any]) -> CGRect {
        let x = CGFloat((dict["x"] as? Double) ?? 0)
        let y = CGFloat((dict["y"] as? Double) ?? 0)
        let w = CGFloat((dict["width"] as? Double) ?? 0)
        let h = CGFloat((dict["height"] as? Double) ?? 0)
        return CGRect(x: x, y: y, width: w, height: h)
    }

    private func ensureVisible(_ rect: CGRect, in container: UIView?) {
        guard let scrollView = scrollView, let container = container else { return }
        let rectInScrollView = scrollView.convert(rect, from: container)
        scrollView.scrollRectToVisible(rectInScrollView.insetBy(dx: 0, dy: -20), animated: true)
    }

    private func shouldEnsureVisibleOnFocus(componentId: String) -> Bool {
        let type = componentTypes[componentId]
        return type != "input.native" && type != "textarea.native"
    }

    private func unmountPage(_ pageId: String) {
        cancelInactivePageStop(pageId)
        guard let ids = pageComponents.removeValue(forKey: pageId) else { return }
        for id in ids {
            unmountComponent(id: id, pageId: pageId)
        }
    }

    private func pausePage(_ pageId: String) {
        guard let ids = pageComponents[pageId] else { return }
        for id in ids {
            guard let component = components[id] else { continue }
            let shouldAutoResume = componentsPendingAutoResume.contains(id) || componentPlaybackIntent[id] == true
            if shouldAutoResume {
                componentsPendingAutoResume.insert(id)
            } else {
                componentsPendingAutoResume.remove(id)
            }
            component.blur()
            component.view.isHidden = true
            component.handleCommand(name: "pause", params: nil)
        }
        scheduleInactivePageStop(pageId)
    }

    private func resumePage(_ pageId: String) {
        cancelInactivePageStop(pageId)
        guard let ids = pageComponents[pageId] else { return }
        for id in ids {
            guard let component = components[id] else { continue }
            component.view.isHidden = false
            if componentsPendingAutoResume.remove(id) != nil {
                component.handleCommand(name: "play", params: nil)
            }
        }
    }

    private func scheduleInactivePageStop(_ pageId: String) {
        cancelInactivePageStop(pageId)
        let delayNs = inactivePageStopDelayNs
        let generation = (pageInactiveStopGeneration[pageId] ?? 0) + 1
        pageInactiveStopGeneration[pageId] = generation
        pageInactiveStopTasks[pageId] = Task { [weak self] in
            do {
                try await Task.sleep(nanoseconds: delayNs)
            } catch {
                return
            }
            guard !Task.isCancelled else { return }
            await self?.applyInactivePageStop(pageId, generation: generation)
        }
    }

    private func cancelInactivePageStop(_ pageId: String) {
        pageInactiveStopGeneration[pageId] = (pageInactiveStopGeneration[pageId] ?? 0) + 1
        pageInactiveStopTasks.removeValue(forKey: pageId)?.cancel()
    }

    private func cancelAllInactivePageStops() {
        pageInactiveStopTasks.values.forEach { $0.cancel() }
        pageInactiveStopTasks.removeAll()
    }

    private func applyInactivePageStop(_ pageId: String, generation: UInt64) {
        guard pageInactiveStopGeneration[pageId] == generation else { return }
        pageInactiveStopTasks.removeValue(forKey: pageId)
        guard let ids = pageComponents[pageId] else { return }
        for id in ids {
            components[id]?.handleCommand(name: "stop", params: nil)
        }
    }

    func dispatchCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        guard let component = components[componentId] else { return false }
        component.handleCommand(name: name, params: params)
        return true
    }

    private func unmountComponent(id: String, pageId: String?) {
        componentsPendingAutoResume.remove(id)
        componentPlaybackIntent.removeValue(forKey: id)
        readyComponentIds.remove(id)
        pendingEventsByComponent.removeValue(forKey: id)
        // Unregister first to block any queued command from being routed back to a component
        // that is in the middle of teardown.
        ComponentRouter.shared.unregister(componentId: id)
        let callbackId = componentCallbacks.removeValue(forKey: id)
        guard let component = components.removeValue(forKey: id) else { return }
        if component is VideoComponent {
            webOverlayCoverageRestore.removeValue(forKey: id)
            updateScrollBounceSuppression()
        }
        // Unregister from hit-test swizzler before unmount
        WKContentViewHitTestSwizzler.shared.unregisterNativeView(component.view)
        component.unmount()
        lastAppliedViewportRect.removeValue(forKey: id)
        if let pageId {
            var set = pageComponents[pageId] ?? []
            set.remove(id)
            if set.isEmpty {
                pageComponents.removeValue(forKey: pageId)
                inactivePages.remove(pageId)
                cancelInactivePageStop(pageId)
            } else {
                pageComponents[pageId] = set
            }
        }
        componentPage.removeValue(forKey: id)
        componentTypes.removeValue(forKey: id)
        componentPageFuncBindings.removeValue(forKey: id)
        componentDataset.removeValue(forKey: id)
        if let callbackId {
            let payload: [String: Any] = [
                "action": "component.event",
                "id": id,
                "componentId": id,
                "event": "unmount",
                "detail": [:]
            ]
            if let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
               let json = String(data: data, encoding: .utf8) {
                _ = onCallback(callbackId, true, json)
            }
        }
    }

    private func updatePlaybackIntent(componentId: String, event: String?) {
        switch event {
        case "play", "playrequest", "playing":
            componentPlaybackIntent[componentId] = true
        case "pause", "stop", "ended", "error":
            componentPlaybackIntent[componentId] = false
        default:
            break
        }
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

    // Use edge rounding without extra height fudge to avoid visual jitter during continuous tracking.
    private func stablePixelAligned(_ rect: CGRect) -> CGRect {
        let scale = UIScreen.main.scale
        let xpx = (rect.origin.x * scale).rounded()
        let ypx = (rect.origin.y * scale).rounded()
        let wpx = max(1, (rect.size.width * scale).rounded())
        let hpx = max(1, (rect.size.height * scale).rounded())
        return CGRect(x: xpx / scale, y: ypx / scale, width: wpx / scale, height: hpx / scale)
    }

    // Same-level container frame must stay in local coordinates.
    // Using `bounds` origin directly is incorrect because UIScrollView bounds origin follows contentOffset.
    private func sameLevelContainerFrame(in container: UIView) -> CGRect {
        CGRect(origin: .zero, size: container.bounds.size)
    }

    private func prepareSameLevelScrollView(_ scrollView: UIScrollView) {
        scrollView.isScrollEnabled = false
        scrollView.delaysContentTouches = false
        scrollView.canCancelContentTouches = false
    }

    private func applyViewportFrame(componentId: String, component: LxNativeComponent, rect: CGRect) {
        // JS measureElement sends document coordinates (viewport + window.scrollY).
        // The overlay host is constrained to contentLayoutGuide, so document coordinates
        // map directly to content coordinates — do NOT add contentOffset again.
        let aligned = stablePixelAligned(rect)
        if let last = lastAppliedViewportRect[componentId],
           rectDistance(last, aligned) <= frameEpsilon {
            return
        }
        component.setFrame(aligned)
        lastAppliedViewportRect[componentId] = aligned
    }

    /// Only video-type components use WKChildScrollView same-level mounting.
    /// Input/textarea use the overlay host which is already in content space,
    /// avoiding WKChildScrollView mis-matching that causes position drift.
    private func prefersSameLevelMounting(type: String) -> Bool {
        return type == "video.native"
    }

    private func rectDistance(_ lhs: CGRect, _ rhs: CGRect) -> CGFloat {
        let dx = abs(lhs.origin.x - rhs.origin.x)
        let dy = abs(lhs.origin.y - rhs.origin.y)
        let dw = abs(lhs.size.width - rhs.size.width)
        let dh = abs(lhs.size.height - rhs.size.height)
        return max(max(dx, dy), max(dw, dh))
    }

    private func viewportToContentRect(_ rect: CGRect) -> CGRect {
        guard let scrollView else { return rect }
        let offset = scrollView.contentOffset
        return CGRect(
            x: rect.origin.x + offset.x,
            y: rect.origin.y + offset.y,
            width: rect.size.width,
            height: rect.size.height
        )
    }

    /// Convert viewport-space rect to WKScrollView coords and locate matching WKChildScrollView.
    private func resolveWKChildScrollView(_ rectInViewport: CGRect) -> UIScrollView? {
        guard let webView = webView else { return nil }
        // Ensure subviews are laid out before matching
        webView.scrollView.layoutIfNeeded()

        // JS getBoundingClientRect() returns viewport-relative coordinates.
        // Convert to scroll view content coordinates by adding scroll offset.
        let scrollOffset = webView.scrollView.contentOffset
        let rectInScroll = CGRect(
            x: rectInViewport.origin.x + scrollOffset.x,
            y: rectInViewport.origin.y + scrollOffset.y,
            width: rectInViewport.size.width,
            height: rectInViewport.size.height
        )
        var candidates: [(scrollView: UIScrollView, frame: CGRect)] = []
        collectWKChildScrollViewsForMatch(in: webView.scrollView, root: webView.scrollView, result: &candidates)
        guard !candidates.isEmpty else { return nil }

        var best: (scrollView: UIScrollView, score: CGFloat)?
        for candidate in candidates {
            let frame = candidate.frame
            let dx = abs(frame.origin.x - rectInScroll.origin.x)
            let dy = abs(frame.origin.y - rectInScroll.origin.y)
            let dw = abs(frame.size.width - rectInScroll.size.width)
            let dh = abs(frame.size.height - rectInScroll.size.height)
            let score = dx + dy + (dw * 0.5) + (dh * 0.5)
            if let current = best {
                if score < current.score {
                    best = (candidate.scrollView, score)
                }
            } else {
                best = (candidate.scrollView, score)
            }
        }

        guard let best else { return nil }
        // Guardrail: reject clearly unrelated candidates.
        if best.score > 240 {
            return nil
        }
        return best.scrollView
    }

    private func collectWKChildScrollViewsForMatch(
        in view: UIView,
        root: UIScrollView,
        result: inout [(scrollView: UIScrollView, frame: CGRect)]
    ) {
        let className = NSStringFromClass(type(of: view))
        if className.contains("WKChildScrollView"), let scrollView = view as? UIScrollView {
            let frameInRoot = root.convert(scrollView.bounds, from: scrollView)
            result.append((scrollView: scrollView, frame: frameInRoot))
        }
        for subview in view.subviews {
            collectWKChildScrollViewsForMatch(in: subview, root: root, result: &result)
        }
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
        os_log("WKContentViewHitTestSwizzler: registered view %{public}@", log: nativeComponentLog, type: .debug, String(describing: view))
    }

    func unregisterNativeView(_ view: UIView) {
        registeredViews.removeValue(forKey: ObjectIdentifier(view))
        os_log("WKContentViewHitTestSwizzler: unregistered view", log: nativeComponentLog, type: .debug)
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
            os_log("WKContentViewHitTestSwizzler: WKContentView class not found", log: nativeComponentLog, type: .error)
            return
        }

        let originalSelector = #selector(UIView.hitTest(_:with:))
        let swizzledSelector = #selector(UIView.lx_swizzled_hitTest(_:with:))

        guard let originalMethod = class_getInstanceMethod(wkContentViewClass, originalSelector),
              let swizzledMethod = class_getInstanceMethod(UIView.self, swizzledSelector) else {
            os_log("WKContentViewHitTestSwizzler: failed to get methods", log: nativeComponentLog, type: .error)
            return
        }

        method_exchangeImplementations(originalMethod, swizzledMethod)
        os_log("WKContentViewHitTestSwizzler: swizzle complete", log: nativeComponentLog, type: .info)
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
