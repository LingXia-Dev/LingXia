#if os(iOS)
import Foundation
import WebKit
import CLingXiaRustAPI

enum LxAppVideo {
    nonisolated static func setVideoPlayerCallback(
        component_id: RustStr,
        callback_id: UInt64
    ) -> Bool {
        let id = component_id.toString()
        return runOnMainActor {
            VideoPlayerRegistry.shared.setCallbackIfExists(componentId: id, callbackId: callback_id)
        }
    }

    nonisolated static func dispatchVideoCommand(
        component_id: RustStr,
        name: RustStr,
        params_json: RustStr
    ) -> Bool {
        let componentId = component_id.toString()
        let commandName = name.toString()
        let paramsString = params_json.toString()

        var params: [String: Any]?
        if !paramsString.isEmpty,
           let data = paramsString.data(using: .utf8),
           let parsed = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            params = parsed
        }

        return runOnMainActor {
            VideoPlayerRegistry.shared.dispatchCommand(
                componentId: componentId,
                name: commandName,
                params: params
            )
        }
    }
    
    private nonisolated static func runOnMainActor<T: Sendable>(_ block: @MainActor () -> T) -> T {
        if Thread.isMainThread {
            return MainActor.assumeIsolated(block)
        } else {
            return DispatchQueue.main.sync {
                MainActor.assumeIsolated(block)
            }
        }
    }
}

@MainActor
final class VideoPlayerRegistry {
    static let shared = VideoPlayerRegistry()

    private struct WeakManager {
        weak var value: SameLevelComponentManager?
    }

    private var callbacks: [String: UInt64] = [:]
    private var managers: [String: WeakManager] = [:]

    func registerComponent(componentId: String, manager: SameLevelComponentManager) {
        managers[componentId] = WeakManager(value: manager)
    }

    func unregisterComponent(componentId: String) {
        managers.removeValue(forKey: componentId)
        callbacks.removeValue(forKey: componentId)
    }

    /// Register callback only if the component (manager) already exists.
    /// Returns true if successful, false if component not found.
    func setCallbackIfExists(componentId: String, callbackId: UInt64) -> Bool {
        guard managers[componentId]?.value != nil else {
            return false
        }
        callbacks[componentId] = callbackId
        return true
    }

    func dispatchCommand(componentId: String, name: String, params: [String: Any]?) -> Bool {
        guard let manager = managers[componentId]?.value else { return false }
        return manager.dispatchCommand(componentId: componentId, name: name, params: params)
    }

    func emitEventIfNeeded(componentId: String, payload: [String: Any]) {
        guard let callbackId = callbacks[componentId] else { return }
        // Ensure the payload carries componentId so Rust can demux with a single callbackId.
        var enriched = payload
        enriched["componentId"] = componentId
        if let data = try? JSONSerialization.data(withJSONObject: enriched, options: []),
           let enrichedJson = String(data: data, encoding: .utf8) {
            _ = onCallback(callbackId, true, enrichedJson)
        }
    }
}
#endif
