#if os(iOS)
import UIKit
import AVFoundation
import Photos
import PhotosUI
import UniformTypeIdentifiers
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

    private static func sendDone(_ callbackId: UInt64) {
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
                            exportVideoToCache(from: fileURL) { exportResult in
                                switch exportResult {
                                case .success(let cacheURL):
                                    let jsonItem: [String: Any] = [
                                        "uri": cacheURL.path,
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
                                case .failure:
                                    let _ = onCallback(callbackId, false, "Failed to process captured video")
                                    sendDone(callbackId)
                                }
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
            // PHPickerViewController doesn't require explicit permission, but we should check photo library access
            checkPhotoLibraryPermission { hasPermission in
                if hasPermission {
                    presentPhotoPicker(presenter: presenter, mode: mode, maxCount: maxCount, callbackId: callbackId)
                } else {
                    // Send error callback for permission denied
                    let _ = onCallback(callbackId, false, "Photo library access is required to select photos. Please enable access in Settings > Privacy & Security > Photos.")
                }
            }
        } else {
            // For iOS 13 and below, we would need to use UIImagePickerController with permission checks
            let _ = onCallback(callbackId, false, "Photo picker requires iOS 14.0 or later")
        }
    }

    private static func checkPhotoLibraryPermission(completion: @escaping (Bool) -> Void) {
        let deliver: (Bool) -> Void = { granted in
            if Thread.isMainThread {
                completion(granted)
            } else {
                DispatchQueue.main.async {
                    completion(granted)
                }
            }
        }

        if #available(iOS 14.0, *) {
            let status = PHPhotoLibrary.authorizationStatus(for: .readWrite)

            switch status {
            case .authorized, .limited:
                deliver(true)
            case .notDetermined:
                PHPhotoLibrary.requestAuthorization(for: .readWrite) { newStatus in
                    let granted = newStatus == .authorized || newStatus == .limited
                    deliver(granted)
                }
            case .denied:
                deliver(false)
            case .restricted:
                deliver(false)
            @unknown default:
                deliver(false)
            }
        } else {
            deliver(false)
        }
    }

    private static func presentPhotoPicker(presenter: UIViewController, mode: String, maxCount: UInt32, callbackId: UInt64) {

        let configuration: PHPickerConfiguration
        if #available(iOS 15.0, *) {
            configuration = PHPickerConfiguration(photoLibrary: PHPhotoLibrary.shared())
        } else {
            configuration = PHPickerConfiguration()
        }
        var mutableConfiguration = configuration
        mutableConfiguration.selectionLimit = Int(maxCount)

        // Set media type based on mode
        switch mode.lowercased() {
        case "video":
            mutableConfiguration.filter = .videos
        case "image":
            mutableConfiguration.filter = .images
        default: // mix
            mutableConfiguration.filter = .any(of: [.images, .videos])
        }

        let picker = PHPickerViewController(configuration: mutableConfiguration)
        let delegate = AlbumDelegate(callbackId: callbackId) {
            LxAppMedia.albumPickerDelegate = nil
        }
        albumPickerDelegate = delegate
        picker.delegate = delegate
        presenter.present(picker, animated: true)
    }
}
#endif
