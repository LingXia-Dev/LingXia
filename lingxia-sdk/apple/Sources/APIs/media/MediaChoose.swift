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
                let _ = onCallback(callback_id, false, "Unable to find top view controller")
                return
            }

            // Parse source types
            guard let sourceTypesData = sourceTypesJson.data(using: .utf8),
                  let sourceTypes = try? JSONDecoder().decode([String].self, from: sourceTypesData) else {
                let _ = onCallback(callback_id, false, "Failed to parse source types")
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
                let _ = onCallback(callback_id, false, "No supported source types")
            }
        }
        return true
    }

    nonisolated private static func sendDone(_ callbackId: UInt64) {
        let _ = onCallback(callbackId, true, "{\"done\":true}")
    }

    private static func openCamera(
        presenter: UIViewController,
        mode: String,
        cameraFacing: String,
        maxDuration: String,
        callbackId: UInt64
    ) {

        guard UIImagePickerController.isSourceTypeAvailable(.camera) else {
            let _ = onCallback(callbackId, false, "Camera is not available on this device")
            return
        }

        checkCameraPermission { granted in
            guard granted else {
                let _ = onCallback(callbackId, false, "Camera access is required to capture media. Please enable access in Settings > Privacy & Security > Camera.")
                return
            }

            let modeLowercased = mode.lowercased()
            let desiredFacingFront = cameraFacing.lowercased() == "front"

            if modeLowercased == "video" {
                checkMicrophonePermission { micGranted in
                    guard micGranted else {
                        let _ = onCallback(callbackId, false, "Microphone access is required to record video. Please enable access in Settings > Privacy & Security > Microphone.")
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
                            let _ = onCallback(callbackId, true, "{\"cancel\":true}")
                            sendDone(callbackId)
                        case .failure(let message):
                            let _ = onCallback(callbackId, false, message)
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
                                let _ = onCallback(callbackId, false, "Failed to serialize camera capture result")
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
                    let _ = onCallback(callbackId, true, "{\"cancel\":true}")
                    sendDone(callbackId)
                case .failure(let message):
                    let _ = onCallback(callbackId, false, message)
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
                        let _ = onCallback(callbackId, false, "Failed to serialize camera capture result")
                        sendDone(callbackId)
                    }
                }
            }
            presenter.present(photoController, animated: true)
        }
    }

    private static func checkCameraPermission(completion: @escaping (Bool) -> Void) {
        let status = AVCaptureDevice.authorizationStatus(for: .video)
        switch status {
        case .authorized:
            completion(true)
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { granted in
                DispatchQueue.main.async {
                    completion(granted)
                }
            }
        case .denied, .restricted:
            completion(false)
        @unknown default:
            completion(false)
        }
    }

    private static func checkMicrophonePermission(completion: @escaping (Bool) -> Void) {
        let audioSession = AVAudioSession.sharedInstance()
        switch audioSession.recordPermission {
        case .granted:
            completion(true)
        case .denied:
            completion(false)
        case .undetermined:
            audioSession.requestRecordPermission { granted in
                DispatchQueue.main.async {
                    completion(granted)
                }
            }
        @unknown default:
            completion(false)
        }
    }

    private static func openAlbum(presenter: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {

        // Check if PHPickerViewController is available (iOS 14+)
        if #available(iOS 14.0, *) {
            handlePhotoLibraryAccess(
                presenter: presenter,
                mode: mode,
                maxCount: maxCount,
                callbackId: callbackId
            )
        } else {
            // For iOS 13 and below, we would need to use UIImagePickerController with permission checks
            let _ = onCallback(callbackId, false, "Photo picker requires iOS 14.0 or later")
        }
    }

    private static func handlePhotoLibraryAccess(
        presenter: UIViewController,
        mode: String,
        maxCount: UInt32,
        callbackId: UInt64
    ) {
        guard #available(iOS 14.0, *) else { return }

        let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)

        switch status {
        case .authorized, .limited, .notDetermined:
            // Always present custom picker; it will handle permission acquisition on '+' click if needed
            presentPhotoPicker(presenter: presenter, mode: mode, maxCount: maxCount, callbackId: callbackId)
        case .denied, .restricted:
            let _ = onCallback(callbackId, false, "Photo library access is required to select photos. Please enable access in Settings > Privacy & Security > Photos.")
        @unknown default:
            let _ = onCallback(callbackId, false, "Photo library access is required to select photos. Please enable access in Settings > Privacy & Security > Photos.")
        }
    }

    private static func presentPhotoPicker(presenter: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {
        // Use custom picker for consistent UX and Limited-mode support
        MediaPickerViewController.present(from: presenter, mode: mode, maxCount: maxCount, callbackId: callbackId)
    }
}
#endif
