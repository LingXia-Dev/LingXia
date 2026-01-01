import Foundation
import UIKit
import OSLog

#if os(iOS)

@MainActor
final class PickerComponentFactory: LxNativeComponentFactory {
    func make(
        id: String,
        initialProps: [String: Any],
        eventSink: @escaping ([String: Any]) -> Void
    ) -> LxNativeComponent {
        PickerComponent(id: id, initialProps: initialProps, eventSink: eventSink)
    }
}

@MainActor
final class PickerComponent: NSObject, LxNativeComponent {
    let id: String
    let view: UIView

    private let eventSink: ([String: Any]) -> Void
    private var currentProps: [String: Any]
    private var currentCallbackId: UInt64 = 0
    private static let log = OSLog(subsystem: "LingXia", category: "PickerComponent")
    private static var nextCallbackId: UInt64 = 1

    init(id: String, initialProps: [String: Any], eventSink: @escaping ([String: Any]) -> Void) {
        self.id = id
        self.eventSink = eventSink
        self.currentProps = initialProps
        // Picker doesn't have a persistent view, create placeholder
        self.view = UIView()
        self.view.backgroundColor = .clear
        self.view.isUserInteractionEnabled = false
        super.init()
    }

    func mount(in host: UIView) {
        // Add placeholder view to host
        if !host.subviews.contains(view) {
            host.addSubview(view)
        }
        // Auto-show picker on mount with current props
        showPickerWithProps(self.currentProps)
    }

    func update(props: [String: Any]) {
        self.currentProps = props
    }

    func setFrame(_ frame: CGRect) {
        view.frame = frame
    }

    func focus() { }
    func blur() { }

    func handleCommand(name: String, params: [String: Any]?) {
        // Commands not needed for picker
    }

    func unmount() {
        // Dismiss picker and cleanup
        LxAppPicker.dismissPicker()
        if currentCallbackId != 0 {
            LxAppPicker.localCallbacks.removeValue(forKey: currentCallbackId)
        }
        view.removeFromSuperview()
    }

    // MARK: - Private Helpers

    private func showPickerWithProps(_ props: [String: Any]) {
        let columnsJSON: String = {
            if let str = props["columns"] as? String { return str }
            if let obj = props["columns"], let data = try? JSONSerialization.data(withJSONObject: obj),
               let str = String(data: data, encoding: .utf8) { return str }
            return "[]"
        }()

        Self.nextCallbackId &+= 1
        currentCallbackId = Self.nextCallbackId

        LxAppPicker.localCallbacks[currentCallbackId] = { [weak self] success, data in
            guard let self else { return }
            // Only remove callback on confirm/cancel (terminal events), not scroll
            let isTerminal = !success || data.contains("\"confirm\"") || data.contains("\"cancel\"")
            if isTerminal {
                LxAppPicker.localCallbacks.removeValue(forKey: self.currentCallbackId)
            }
            self.handlePickerCallback(success: success, data: data)
        }

        LxAppPicker.showPicker(
            columns: columnsJSON,
            cancelText: props["cancelText"] as? String ?? "Cancel",
            cancelButtonColor: props["cancelButtonColor"] as? String ?? "",
            cancelTextColor: props["cancelTextColor"] as? String ?? "",
            confirmText: props["confirmText"] as? String ?? "OK",
            confirmButtonColor: props["confirmButtonColor"] as? String ?? "",
            confirmTextColor: props["confirmTextColor"] as? String ?? "",
            callbackID: currentCallbackId
        )
    }

    private func sendEvent(_ event: String, detail: [String: Any]) {
        eventSink(["event": event, "detail": detail])
    }

    private func handlePickerCallback(success: Bool, data: String) {
        if !success {
            sendEvent("change", detail: ["cancelled": true])
            return
        }

        guard let jsonData = data.data(using: .utf8),
              let result = try? JSONSerialization.jsonObject(with: jsonData) as? [String: Any] else { return }

        var detail: [String: Any] = [:]
        if let index = result["index"] { detail["index"] = index }
        if result["confirm"] as? Bool == true {
            detail["confirmed"] = true
        } else if result["cancel"] as? Bool == true {
            detail["cancelled"] = true
        }

        sendEvent("change", detail: detail)
    }
}

#endif
