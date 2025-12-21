#if os(iOS)
import UIKit
import AVFoundation
import Photos
import CLingXiaSwiftAPI
import CLingXiaRustAPI

extension LxAppMedia {
    nonisolated static func chooseMedia(
        max_count: UInt32,
        mode: RustStr,
        source_types_json: RustStr,
        camera_facing: RustStr,
        max_duration: RustStr,
        callback_id: UInt64
    ) -> Bool {
        let modeStr = mode.toString()
        let sourceTypesJson = source_types_json.toString()
        let cameraFacingStr = camera_facing.toString()
        let maxDurationStr = max_duration.toString()


        DispatchQueue.main.async {
            guard let presenter = topViewController() else {
                let _ = onCallback(callback_id, false, "1000")
                return
            }

            // Parse source types
            guard let sourceTypesData = sourceTypesJson.data(using: .utf8),
                  let sourceTypes = try? JSONDecoder().decode([String].self, from: sourceTypesData) else {
                let _ = onCallback(callback_id, false, "1002")
                return
            }

            let allowAlbum = sourceTypes.contains("album")
            let allowCamera = sourceTypes.contains("camera")

            if allowCamera {
                openCamera(
                    presenter: presenter,
                    mode: modeStr,
                    cameraFacing: cameraFacingStr,
                    maxDuration: maxDurationStr,
                    callbackId: callback_id
                )
            } else if allowAlbum {
                openAlbum(presenter: presenter, mode: modeStr, maxCount: max_count, callbackId: callback_id)
            } else {
                let _ = onCallback(callback_id, false, "1002")
            }
        }
        return true
    }

    nonisolated private static func sendDone(_ callbackId: UInt64) {
        let _ = onCallback(callbackId, true, "{\"done\":true}")
    }

    nonisolated private static func sendError(_ callbackId: UInt64, _ code: Int) {
        let _ = onCallback(callbackId, false, "\(code)")
    }

    private static func openCamera(
        presenter: UIViewController,
        mode: String,
        cameraFacing: String,
        maxDuration: String,
        callbackId: UInt64
    ) {

        guard UIImagePickerController.isSourceTypeAvailable(.camera) else {
            let _ = onCallback(callbackId, false, "6001")
            return
        }

        PermissionManager.ensureCameraAccess { granted in
            guard granted else {
                let _ = onCallback(callbackId, false, "3001")
                return
            }

            let modeLowercased = mode.lowercased()
            let desiredFacingFront = cameraFacing.lowercased() == "front"

            if modeLowercased == "video" {
                PermissionManager.ensureMicrophoneAccess { micGranted in
                    guard micGranted else {
                        let _ = onCallback(callbackId, false, "3003")
                        return
                    }

                    let maxDurationValue = Double(maxDuration).flatMap { $0 > 0 ? $0 : nil }
                    let initialPosition: AVCaptureDevice.Position = desiredFacingFront ? .front : .back

                    let videoController = VideoCaptureViewController(
                        initialCameraPosition: initialPosition,
                        maxDuration: maxDurationValue
                    ) { result in
                        switch result {
                        case .cancelled:
                            let _ = onCallback(callbackId, false, "2000")
                            sendDone(callbackId)
                        case .failure(_):
                            let _ = onCallback(callbackId, false, "1001")
                            sendDone(callbackId)
                        case .success(let fileURL):
                            let jsonItem: [String: Any] = [
                                "uri": fileURL.path,
                                "fileType": "video",
                                "isOriginal": true
                            ]

                            if let data = try? JSONSerialization.data(withJSONObject: [jsonItem], options: []),
                               let jsonString = String(data: data, encoding: .utf8) {
                                let _ = onCallback(callbackId, true, jsonString)
                                sendDone(callbackId)
                            } else {
                                let _ = onCallback(callbackId, false, "1000")
                                sendDone(callbackId)
                            }
                            return
                        }
                    }

                    presenter.present(videoController, animated: true)
                }
                return
            }

            let initialPosition: AVCaptureDevice.Position = desiredFacingFront ? .front : .back
            let photoController = PhotoCaptureViewController(initialCameraPosition: initialPosition) { result in
                switch result {
                case .cancelled:
                    let _ = onCallback(callbackId, false, "2000")
                    sendDone(callbackId)
                case .failure(_):
                    let _ = onCallback(callbackId, false, "1001")
                    sendDone(callbackId)
                case .success(let fileURL):
                    let jsonItem: [String: Any] = [
                        "uri": fileURL.path,
                        "fileType": "image",
                        "isOriginal": true
                    ]

                    if let data = try? JSONSerialization.data(withJSONObject: [jsonItem], options: []),
                       let jsonString = String(data: data, encoding: .utf8) {
                        let _ = onCallback(callbackId, true, jsonString)
                        sendDone(callbackId)
                    } else {
                        let _ = onCallback(callbackId, false, "1000")
                        sendDone(callbackId)
                    }
                }
            }
            presenter.present(photoController, animated: true)
        }
    }

    private static func openAlbum(presenter: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {

        let selectionLimit = mode.lowercased() == "video" ? 1 : maxCount

        // Check if PHPickerViewController is available (iOS 14+)
        if #available(iOS 14.0, *) {
            handlePhotoLibraryAccess(
                presenter: presenter,
                mode: mode,
                maxCount: selectionLimit,
                callbackId: callbackId
            )
        } else {
            // For iOS 13 and below, we would need to use UIImagePickerController with permission checks
            let _ = onCallback(callbackId, false, "6002")
        }
    }

    private static func handlePhotoLibraryAccess(
        presenter: UIViewController,
        mode: String,
        maxCount: UInt32,
        callbackId: UInt64
    ) {
        guard #available(iOS 14.0, *) else { return }

        PermissionManager.ensurePhotoLibraryAccess { granted in
            if granted {
                presentPhotoPicker(presenter: presenter, mode: mode, maxCount: maxCount, callbackId: callbackId)
            } else {
                let _ = onCallback(callbackId, false, "3004")
            }
        }
    }

    private static func presentPhotoPicker(presenter: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {
        // Use custom picker for consistent UX and Limited-mode support
        MediaPickerViewController.present(from: presenter, mode: mode, maxCount: maxCount, callbackId: callbackId)
    }
}
#endif
