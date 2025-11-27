import Foundation
import UIKit
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

    private var components: [String: LxNativeComponent] = [:]
    private var componentPage: [String: String] = [:]
    private var pageComponents: [String: Set<String>] = [:]
    private let defaultPageId: String
    private var factories: [String: LxNativeComponentFactory] = [:]
    private let eventSink: (_ payload: [String: Any]) -> Void

    init(
        scrollView: UIScrollView,
        hostView: UIView,
        defaultPageId: String,
        eventSink: @escaping (_ payload: [String: Any]) -> Void
    ) {
        self.scrollView = scrollView
        self.hostView = hostView
        self.defaultPageId = defaultPageId
        self.eventSink = eventSink
    }

    func register(type: String, factory: LxNativeComponentFactory) {
        factories[type] = factory
    }

    // MARK: - Message handlers

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

        guard let host = hostView else { return }

        component.mount(in: host)
        component.setFrame(pixelAligned(rect))
        component.update(props: props)
        component.view.layer.cornerRadius = cornerRadius
        if #available(iOS 13.0, *) {
            component.view.layer.cornerCurve = .continuous
        }
        component.view.layer.masksToBounds = true
        component.view.layer.zPosition = zIndex
        host.bringSubviewToFront(component.view)

        if cornerRadius > 0 {
            component.update(props: ["cornerRadius": cornerRadius])
        }
    }

    private func handleUpdate(_ parameters: [String: Any]) {
        guard let id = parameters["id"] as? String,
              let component = components[id] else { return }

        if let rectDict = parameters["rect"] as? [String: Any] {
            let rect = rectFrom(dict: rectDict)
            component.setFrame(pixelAligned(rect))
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
        os_log("SameLevelComponentManager handleCommand name=%{public}@ id=%{public}@", log: sameLevelComponentLog, type: .info, name, id)
        let params = parameters["params"] as? [String: Any]
        component.handleCommand(name: name, params: params)
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

    private func unmountComponent(id: String, pageId: String?) {
        guard let component = components.removeValue(forKey: id) else { return }
        component.unmount()
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
    }
}

#endif
