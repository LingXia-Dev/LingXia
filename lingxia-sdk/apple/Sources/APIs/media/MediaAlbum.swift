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
            if let assetIdentifier = result.assetIdentifier {
                DispatchQueue.main.async {
                    let jsonItem: [String: Any] = [
                        "uri": "phasset:\(assetIdentifier)",
                        "fileType": "image",
                        "isOriginal": true
                    ]
                    completion(jsonItem)
                }
            } else {
                provider.loadFileRepresentation(forTypeIdentifier: UTType.image.identifier) { url, error in
                    if error != nil {
                        DispatchQueue.main.async { completion(nil) }
                        return
                    }

                    guard let url else {
                        DispatchQueue.main.async { completion(nil) }
                        return
                    }

                    DispatchQueue.main.async {
                        do {
                            let cachedURL = try LxAppMediaStorage.copy(
                                from: url,
                                prefix: "album_image",
                                fallbackExtension: "jpg",
                                requiresSecurityScope: true
                            )
                            let jsonItem: [String: Any] = [
                                "uri": cachedURL.path,
                                "fileType": "image",
                                "isOriginal": true
                            ]
                            completion(jsonItem)
                        } catch {
                            completion(nil)
                        }
                    }
                }
            }
        } else if provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier) {
            if let assetIdentifier = result.assetIdentifier {
                DispatchQueue.main.async {
                    let jsonItem: [String: Any] = [
                        "uri": "phasset:\(assetIdentifier)",
                        "fileType": "video",
                        "isOriginal": true
                    ]
                    completion(jsonItem)
                }
            } else {
                provider.loadFileRepresentation(forTypeIdentifier: UTType.movie.identifier) { url, error in
                    if error != nil {
                        DispatchQueue.main.async { completion(nil) }
                        return
                    }
                    
                    guard let url else {
                        DispatchQueue.main.async { completion(nil) }
                        return
                    }
                    DispatchQueue.main.async {
                        do {
                            let cachedURL = try LxAppMediaStorage.copy(
                                from: url,
                                prefix: "album_video",
                                fallbackExtension: "mov",
                                requiresSecurityScope: true
                            )
                            let jsonItem: [String: Any] = [
                                "uri": cachedURL.path,
                                "fileType": "video",
                                "isOriginal": true
                            ]
                            completion(jsonItem)
                        } catch {
                            completion(nil)
                        }
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
extension LxAppMedia {
    nonisolated static func copyAssetResource(
        local_identifier: RustStr,
        destination_path: RustStr,
        media_type: Int32
    ) -> Bool {
        let assetId = local_identifier.toString()
        let path = destination_path.toString()

        guard !assetId.isEmpty, !path.isEmpty else {
            return false
        }

        let fetchResult = PHAsset.fetchAssets(withLocalIdentifiers: [assetId], options: nil)
        guard let asset = fetchResult.firstObject else {
            return false
        }

        let isVideo = media_type == 1
        let resources = PHAssetResource.assetResources(for: asset)

        let preferredTypes: [PHAssetResourceType]
        if isVideo {
            preferredTypes = [.video, .fullSizeVideo, .pairedVideo, .adjustmentBaseVideo]
        } else {
            preferredTypes = [.photo, .fullSizePhoto, .adjustmentBasePhoto]
        }

        let resource = resources.first { preferredTypes.contains($0.type) } ?? resources.first
        guard let assetResource = resource else {
            return false
        }

        let destinationURL = URL(fileURLWithPath: path)
        let fileManager = FileManager.default
        let parentURL = destinationURL.deletingLastPathComponent()
        try? fileManager.createDirectory(at: parentURL, withIntermediateDirectories: true)
        if fileManager.fileExists(atPath: destinationURL.path) {
            try? fileManager.removeItem(at: destinationURL)
        }

        let options = PHAssetResourceRequestOptions()
        options.isNetworkAccessAllowed = true

        let semaphore = DispatchSemaphore(value: 0)
        var success = true

        PHAssetResourceManager.default().writeData(
            for: assetResource,
            toFile: destinationURL,
            options: options
        ) { error in
            if error != nil {
                success = false
            }
            semaphore.signal()
        }

        semaphore.wait()
        return success
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
