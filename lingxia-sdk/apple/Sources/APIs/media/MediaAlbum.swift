#if os(iOS)
import UIKit
import PhotosUI
import Photos
import UniformTypeIdentifiers
import AVFoundation
import CLingXiaRustAPI

// MARK: - Album Delegate
final class AlbumDelegate: NSObject, PHPickerViewControllerDelegate {
    private let callbackId: UInt64
    private let cleanup: () -> Void

    init(callbackId: UInt64, cleanup: @escaping () -> Void) {
        self.callbackId = callbackId
        self.cleanup = cleanup
        super.init()
    }

    func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {

        picker.dismiss(animated: true)

        guard !results.isEmpty else {
            sendCancel()
            cleanup()
            return
        }

        var jsonArray: [[String: Any]] = []
        let group = DispatchGroup()
        
        for result in results {
            group.enter()
            handleResult(result) { item in
                if let item {
                    jsonArray.append(item)
                }
                group.leave()
            }
        }
        
        group.notify(queue: .main) {
            self.sendCallback(jsonArray: jsonArray)
            self.cleanup()
        }
    }

    private func handleResult(_ result: PHPickerResult, completion: @escaping ([String: Any]?) -> Void) {
        guard #available(iOS 14.0, *) else {
            DispatchQueue.main.async {
                    completion(nil)
            }
                    return
                }
                
        let provider = result.itemProvider

        if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
            provider.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { url, error in
                if error != nil {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }

                guard let url else {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }

                let tempURL = copyMediaFileToTemp(
                    from: url,
                    prefix: "album_image",
                    fallbackExtension: "jpg",
                    requiresSecurityScope: true
                )
            DispatchQueue.main.async {
                    if let tempURL {
                        let jsonItem: [String: Any] = [
                            "uri": tempURL.absoluteString,
                            "fileType": "image",
                            "isOriginal": true
                        ]
                        completion(jsonItem)
                    } else {
                    completion(nil)
                    }
                }
            }
        } else if provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier) {
            provider.loadFileRepresentation(forTypeIdentifier: UTType.movie.identifier) { url, error in
                if error != nil {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }
                
                guard let url else {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }

                let tempURL = copyMediaFileToTemp(
                    from: url,
                    prefix: "album_video",
                    fallbackExtension: "mov",
                    requiresSecurityScope: true
                )
                DispatchQueue.main.async {
                    if let tempURL {
                        let jsonItem: [String: Any] = [
                            "uri": tempURL.absoluteString,
                            "fileType": "video",
                            "isOriginal": true
                        ]
                        completion(jsonItem)
                    } else {
                        completion(nil)
                    }
                }
            }
        } else {
            DispatchQueue.main.async {
                completion(nil)
            }
        }
    }

    private func sendCallback(jsonArray: [[String: Any]]) {
        do {
            let jsonData = try JSONSerialization.data(withJSONObject: jsonArray, options: [])
            let jsonString = String(data: jsonData, encoding: .utf8) ?? "[]"

            let _ = onCallback(callbackId, true, jsonString)
        } catch {
            let _ = onCallback(callbackId, false, "Failed to serialize album data")
        }
    }

    private func sendCancel() {
        let _ = onCallback(callbackId, true, "{\"cancel\":true}")
    }
}
#endif

#if os(iOS)
enum PhotoCaptureResult {
    case success(URL)
    case cancelled
    case failure(String)
}

enum PhotoCaptureHint {
    static let preparing = "准备相机..."
    static let ready = "点击拍照"
    static let switching = "切换摄像头..."
}
#endif
