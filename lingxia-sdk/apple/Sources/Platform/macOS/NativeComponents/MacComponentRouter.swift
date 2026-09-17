#if os(macOS)
import Foundation

@MainActor
final class MacComponentRouter {
    static let shared = MacComponentRouter()

    private struct WeakManager {
        weak var value: MacNativeComponentManager?
    }

    private var managers: [String: WeakManager] = [:]

    private init() {}

    func register(componentId: String, manager: MacNativeComponentManager) {
        managers[componentId] = WeakManager(value: manager)
    }

    /// Drop `manager`'s registration. A reloaded page registers the same ids
    /// before the previous page's manager finishes its deferred teardown, so
    /// an id now owned by another live manager is left alone.
    func unregister(componentId: String, manager: MacNativeComponentManager) {
        if let owner = managers[componentId]?.value, owner !== manager { return }
        managers.removeValue(forKey: componentId)
    }

    func setCallback(componentId: String, callbackId: UInt64) -> Bool {
        guard let manager = managers[componentId]?.value else { return false }
        return manager.setCallback(componentId: componentId, callbackId: callbackId)
    }

    func dispatchCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        guard let manager = managers[componentId]?.value else { return false }
        return manager.dispatchCommand(componentId: componentId, name: name, params: params)
    }

    func emitComponentEvent(componentId: String, event: String, detail: [String: Any]) {
        managers[componentId]?.value?.emitComponentEvent(componentId: componentId, event: event, detail: detail)
    }
}

#endif
