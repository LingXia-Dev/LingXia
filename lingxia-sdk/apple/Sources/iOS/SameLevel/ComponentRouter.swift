import Foundation

#if os(iOS)

/// Global router for dispatching commands from Rust FFI to native components.
///
/// This is a lightweight registry that only maintains componentId -> manager mappings
/// for command routing. All component state (including callbacks) is managed by
/// SameLevelComponentManager.
@MainActor
final class ComponentRouter {
    static let shared = ComponentRouter()
    
    private struct WeakManager {
        weak var value: SameLevelComponentManager?
    }
    
    private var managers: [String: WeakManager] = [:]
    
    private init() {}
    
    func register(componentId: String, manager: SameLevelComponentManager) {
        managers[componentId] = WeakManager(value: manager)
    }
    
    func unregister(componentId: String) {
        managers.removeValue(forKey: componentId)
    }
    
    /// Set callback for a component. Called from Rust FFI.
    /// Returns true if component exists and callback was set.
    func setCallback(componentId: String, callbackId: UInt64) -> Bool {
        guard let manager = managers[componentId]?.value else { return false }
        return manager.setCallback(componentId: componentId, callbackId: callbackId)
    }
    
    /// Dispatch a command to a component. Called from Rust FFI.
    func dispatchCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        guard let manager = managers[componentId]?.value else { return false }
        return manager.dispatchCommand(componentId: componentId, name: name, params: params)
    }
}

#endif

