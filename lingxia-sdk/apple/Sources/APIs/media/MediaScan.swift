#if os(iOS)
import UIKit
import AVFoundation
import Vision
import CLingXiaSwiftAPI
import CLingXiaRustAPI

extension LxAppMedia {
    nonisolated static func scanCode(
        scan_types_json: RustStr,
        only_from_camera: Bool,
        callback_id: UInt64
    ) -> Bool {
#if os(iOS)
        let typesJson = scan_types_json.toString()
        DispatchQueue.main.async {
            guard #available(iOS 13.0, *) else {
                let _ = onCallback(callback_id, false, "scanCode requires iOS 13.0 or later")
                return
            }
            guard let presenter = topViewController() else {
                let _ = onCallback(callback_id, false, "Unable to find top view controller")
                return
            }

            let codes: [Int]
            if let data = typesJson.data(using: .utf8),
               let parsed = try? JSONSerialization.jsonObject(with: data, options: []) as? [Int] {
                codes = parsed
            } else {
                codes = []
            }

            let controller = ScanCodeViewController(
                scanTypes: codes,
                onlyFromCamera: only_from_camera,
                callbackId: callback_id
            )
            controller.modalPresentationStyle = .fullScreen
            presenter.present(controller, animated: true)
        }
        return true
#else
        return false
#endif
    }
}
#endif
