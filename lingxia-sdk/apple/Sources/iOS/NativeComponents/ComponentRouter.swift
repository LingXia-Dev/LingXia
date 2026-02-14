import Foundation
import OSLog

#if os(iOS)
import UIKit

/// Global router for dispatching commands from Rust FFI to native components.
///
/// This is a lightweight registry that only maintains componentId -> manager mappings
/// for command routing. All component state (including callbacks) is managed by
/// NativeComponentManager.
@MainActor
final class ComponentRouter {
    static let shared = ComponentRouter()
    private let log = OSLog(subsystem: "LingXia", category: "NativeComponent")
    
    private struct WeakManager {
        weak var value: NativeComponentManager?
    }
    
    private var managers: [String: WeakManager] = [:]
    
    private init() {}
    
    func register(componentId: String, manager: NativeComponentManager) {
        managers[componentId] = WeakManager(value: manager)
    }
    
    func unregister(componentId: String) {
        StreamDecoderRegistry.shared.destroy(componentId: componentId)
        managers.removeValue(forKey: componentId)
    }
    
    /// Set callback for a component. Called from Rust FFI.
    /// Returns true if component exists and callback was set.
    func setCallback(componentId: String, callbackId: UInt64) -> Bool {
        guard let manager = managers[componentId]?.value else { return false }
        os_log(
            "ComponentRouter setCallback componentId=%{public}@ callbackId=%{public}@",
            log: log,
            type: .debug,
            componentId,
            String(callbackId)
        )
        return manager.setCallback(componentId: componentId, callbackId: callbackId)
    }
    
    /// Dispatch a command to a component. Called from Rust FFI.
    func dispatchCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        if name != "enterFullscreen" && name != "exitFullscreen" {
            if StreamDecoderRegistry.shared.shouldHandleCommand(componentId: componentId) {
                if StreamDecoderRegistry.shared.handleCommand(
                    componentId: componentId,
                    name: name,
                    params: params
                ) {
                    return true
                }
            }
        }

        guard let manager = managers[componentId]?.value else { return false }
        return manager.dispatchCommand(componentId: componentId, name: name, params: params)
    }

    func componentView(componentId: String) -> UIView? {
        return managers[componentId]?.value?.componentView(componentId: componentId)
    }

    func emitComponentEvent(componentId: String, event: String, detail: [String: Any]) {
        managers[componentId]?.value?.emitComponentEvent(componentId: componentId, event: event, detail: detail)
    }

    func setStreamDecoderActive(componentId: String, active: Bool) {
        managers[componentId]?.value?.setStreamDecoderActive(componentId: componentId, active: active)
    }
}

#endif
