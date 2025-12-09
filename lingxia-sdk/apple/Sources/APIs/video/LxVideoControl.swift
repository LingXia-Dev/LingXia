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
            ComponentRouter.shared.setCallback(componentId: id, callbackId: callback_id)
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
            ComponentRouter.shared.dispatchCommand(
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
#endif
