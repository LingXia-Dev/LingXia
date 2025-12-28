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

    nonisolated static func createStreamDecoder(component_id: RustStr) -> Bool {
        let componentId = component_id.toString()
        return runOnMainActor {
            StreamDecoderRegistry.shared.create(componentId: componentId)
        }
    }

    nonisolated static func configureStreamVideo(
        component_id: RustStr,
        config_json: RustStr
    ) -> Bool {
        let componentId = component_id.toString()
        let configJson = config_json.toString()
        return runOnMainActor {
            StreamDecoderRegistry.shared.configureVideo(componentId: componentId, configJson: configJson)
        }
    }

    nonisolated static func configureStreamAudio(
        component_id: RustStr,
        config_json: RustStr
    ) -> Bool {
        let componentId = component_id.toString()
        let configJson = config_json.toString()
        return runOnMainActor {
            StreamDecoderRegistry.shared.configureAudio(componentId: componentId, configJson: configJson)
        }
    }

    nonisolated static func pushStreamVideo(
        component_id: RustStr,
        data: RustVec<UInt8>,
        dts_ms: UInt32,
        pts_ms: UInt32,
        keyframe: Bool
    ) -> Bool {
        let componentId = component_id.toString()
        let length = data.len()
        let buffer = UnsafeBufferPointer(start: data.as_ptr(), count: length)
        let payload = Data(buffer: buffer)
        return runOnMainActor {
            StreamDecoderRegistry.shared.pushVideo(
                componentId: componentId,
                data: payload,
                dtsMs: dts_ms,
                ptsMs: pts_ms,
                keyframe: keyframe
            )
        }
    }

    nonisolated static func pushStreamAudio(
        component_id: RustStr,
        data: RustVec<UInt8>,
        dts_ms: UInt32,
        pts_ms: UInt32
    ) -> Bool {
        let componentId = component_id.toString()
        let length = data.len()
        let buffer = UnsafeBufferPointer(start: data.as_ptr(), count: length)
        let payload = Data(buffer: buffer)
        return runOnMainActor {
            StreamDecoderRegistry.shared.pushAudio(
                componentId: componentId,
                data: payload,
                dtsMs: dts_ms,
                ptsMs: pts_ms
            )
        }
    }

    nonisolated static func stopStreamDecoder(component_id: RustStr) -> Bool {
        let componentId = component_id.toString()
        return runOnMainActor {
            StreamDecoderRegistry.shared.stop(componentId: componentId)
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
