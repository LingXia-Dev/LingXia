#if os(macOS)
import Foundation
import AppKit
import WebKit
import CLingXiaRustAPI

@MainActor
protocol MacNativeComponent: AnyObject {
    var id: String { get }
    var view: NSView { get }

    func mount(in host: NSView)
    func update(props: [String: Any])
    func setFrame(_ frame: CGRect)
    func focus()
    func blur()
    func handleCommand(name: String, params: [String: Any]?)
    func unmount()
}

@MainActor
protocol MacNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping (_ event: [String: Any]) -> Void
    ) -> MacNativeComponent
}

@MainActor
final class MacNativeComponentManager {
    private weak var hostView: NSView?
    private weak var webView: WKWebView?

    private var components: [String: MacNativeComponent] = [:]
    private var componentPage: [String: String] = [:]
    private var componentPageFuncBindings: [String: [String: String]] = [:]
    private var componentDataset: [String: [String: Any]] = [:]
    private var readyComponentIds: Set<String> = []
    private var pendingEventsByComponent: [String: [[String: Any]]] = [:]
    private var pageComponents: [String: Set<String>] = [:]
    private var componentCallbacks: [String: UInt64] = [:]
    private let defaultPageId: String
    private var factories: [String: MacNativeComponentFactory] = [:]
    private let eventSink: (_ payload: [String: Any]) -> Void

    private var scrollOffsetX: CGFloat = 0
    private var scrollOffsetY: CGFloat = 0
    private var componentDocumentRects: [String: CGRect] = [:]
    private let inactivePageStopDelayNs: UInt64 = 60_000_000_000
    private let maxPendingNativeEventsPerComponent: Int = 8
    private var pageInactiveStopTasks: [String: Task<Void, Never>] = [:]
    private var pageInactiveStopGeneration: [String: UInt64] = [:]
    private var componentPlaybackIntent: [String: Bool] = [:]
    private var componentsPendingAutoResume: Set<String> = []
    private var inactivePages: Set<String> = []

    init(
        hostView: NSView,
        webView: WKWebView,
        defaultPageId: String,
        eventSink: @escaping (_ payload: [String: Any]) -> Void
    ) {
        self.hostView = hostView
        self.webView = webView
        self.defaultPageId = defaultPageId
        self.eventSink = eventSink
    }

    func register(type: String, factory: MacNativeComponentFactory) {
        factories[type] = factory
    }

    func rebindHostView(_ host: NSView) {
        guard hostView !== host else { return }
        hostView = host

        for (id, component) in components {
            component.view.removeFromSuperview()
            component.mount(in: host)
            if let docRect = componentDocumentRects[id] {
                component.setFrame(documentToViewport(docRect))
            }
        }
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
        guard let host = hostView else { return }

        let pageId = resolvePageId(parameters)
        let props = (parameters["props"] as? [String: Any]) ?? [:]
        let cornerRadius = CGFloat(parameters["cornerRadius"] as? Double ?? 0)

        let rect = rectFrom(dict: rectDict)

        guard components[id] == nil, let factory = factories[type] else { return }

        let component = factory.make(id: id, initialProps: props) { [weak self] event in
            self?.dispatchComponentEvent(componentId: id, event: event)
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
        MacComponentRouter.shared.register(componentId: id, manager: self)
        component.mount(in: host)

        componentDocumentRects[id] = rect
        let viewportRect = documentToViewport(rect)
        component.setFrame(viewportRect)
        component.update(props: props)

        if cornerRadius > 0 {
            component.view.wantsLayer = true
            component.view.layer?.cornerRadius = cornerRadius
            component.view.layer?.masksToBounds = true
            component.update(props: ["cornerRadius": cornerRadius])
        }
    }

    private func handleUpdate(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let component = components[id] else { return }

        if let rectDict = parameters["rect"] as? [String: Any] {
            let docRect = rectFrom(dict: rectDict)
            componentDocumentRects[id] = docRect
            component.setFrame(documentToViewport(docRect))
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
        if let radius = parameters["cornerRadius"] as? Double {
            component.view.wantsLayer = true
            component.view.layer?.cornerRadius = CGFloat(radius)
            component.view.layer?.masksToBounds = true
            component.update(props: ["cornerRadius": radius])
        }
    }

    private func handleUnmount(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String, !id.isEmpty else { return }
        let pageId = resolvePageId(parameters)
        unmountComponent(id: id, pageId: pageId)
    }

    private func handleFocus(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let component = components[id] else { return }
        component.focus()
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

    func setCallback(componentId: String, callbackId: UInt64) -> Bool {
        componentCallbacks[componentId] = callbackId
        return true
    }

    func dispatchCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        guard let component = components[componentId] else { return false }
        component.handleCommand(name: name, params: params)
        return true
    }

    func emitComponentEvent(componentId: String, event: String, detail: [String: Any] = [:]) {
        dispatchComponentEvent(componentId: componentId, event: ["event": event, "detail": detail])
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

    private func dispatchComponentEvent(componentId: String, event: [String: Any]) {
        var payload = event
        updatePlaybackIntent(componentId: componentId, event: payload["event"] as? String)
        payload["action"] = "component.event"
        payload["id"] = componentId
        payload["componentId"] = componentId
        if let pageId = componentPage[componentId] {
            payload["pageId"] = pageId
        }
        emitEventToView(componentId: componentId, payload: payload)
        dispatchPageFunc(componentId: componentId, payload: payload)

        let eventName = payload["event"] as? String
        let shouldForward: Bool = {
            switch eventName {
            case "waiting", "playrequest", "playing", "pause", "stop", "ended", "error", "seeked", "seeking":
                return true
            default:
                return false
            }
        }()

        if shouldForward, let callbackId = componentCallbacks[componentId] {
            if let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
               let enrichedJson = String(data: data, encoding: .utf8) {
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

    private func documentToViewport(_ rect: CGRect) -> CGRect {
        return CGRect(
            x: rect.origin.x - scrollOffsetX,
            y: rect.origin.y - scrollOffsetY,
            width: rect.size.width,
            height: rect.size.height
        )
    }

    func updateScrollOffset(x: CGFloat, y: CGFloat) {
        scrollOffsetX = x
        scrollOffsetY = y
        for (id, docRect) in componentDocumentRects {
            guard let component = components[id] else { continue }
            component.setFrame(documentToViewport(docRect))
        }
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
            return
        }
        let pageEvent = buildPageEvent(componentId: componentId, eventName: eventName, payload: payload)
        guard let data = try? JSONSerialization.data(withJSONObject: pageEvent, options: []),
              let payloadJson = String(data: data, encoding: .utf8) else {
            return
        }
        guard let bindingsData = try? JSONSerialization.data(withJSONObject: bindings, options: []),
              let bindingsJson = String(data: bindingsData, encoding: .utf8) else {
            return
        }
        _ = onNativeComponentEvent(route.appid, route.path, componentId, eventName, payloadJson, bindingsJson)
    }

    private func rectFrom(dict: [String: Any]) -> CGRect {
        let x = Self.cgFloat(from: dict["x"])
        let y = Self.cgFloat(from: dict["y"])
        let w = Self.cgFloat(from: dict["width"])
        let h = Self.cgFloat(from: dict["height"])
        return CGRect(x: x, y: y, width: w, height: h)
    }

    private static func cgFloat(from value: Any?) -> CGFloat {
        if let cg = value as? CGFloat { return cg }
        if let number = value as? NSNumber { return CGFloat(number.doubleValue) }
        if let doubleValue = value as? Double { return CGFloat(doubleValue) }
        if let floatValue = value as? Float { return CGFloat(floatValue) }
        if let intValue = value as? Int { return CGFloat(intValue) }
        if let string = value as? String, let parsed = Double(string) {
            return CGFloat(parsed)
        }
        return 0
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

    private func unmountPage(_ pageId: String) {
        cancelInactivePageStop(pageId)
        guard let ids = pageComponents.removeValue(forKey: pageId) else { return }
        for id in ids {
            unmountComponent(id: id, pageId: pageId)
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

    private func unmountComponent(id: String, pageId: String?) {
        componentsPendingAutoResume.remove(id)
        componentPlaybackIntent.removeValue(forKey: id)
        readyComponentIds.remove(id)
        pendingEventsByComponent.removeValue(forKey: id)
        guard let component = components.removeValue(forKey: id) else { return }
        if let callbackId = componentCallbacks[id] {
            let payload: [String: Any] = [
                "action": "component.event",
                "id": id,
                "componentId": id,
                "event": "unmount",
                "detail": [String: Any]()
            ]
            if let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
               let json = String(data: data, encoding: .utf8) {
                _ = onCallback(callbackId, true, json)
            }
        }
        component.unmount()
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
        componentPageFuncBindings.removeValue(forKey: id)
        componentDataset.removeValue(forKey: id)
        componentCallbacks.removeValue(forKey: id)
        componentDocumentRects.removeValue(forKey: id)
        MacComponentRouter.shared.unregister(componentId: id)
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
}

#endif
