#if os(iOS)
import UIKit
import PhotosUI
import Photos
import UniformTypeIdentifiers
import AVFoundation
import CryptoKit
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

    private func loadImageObjectFallback(provider: NSItemProvider, completion: @escaping ([String: Any]?) -> Void) {
        if provider.canLoadObject(ofClass: UIImage.self) {
            provider.loadObject(ofClass: UIImage.self) { object, _ in
                guard let image = object as? UIImage else {
                    DispatchQueue.main.async { completion(nil) }
                    return
                }
                DispatchQueue.main.async {
                    do {
                        guard let d = image.jpegData(compressionQuality: 0.95) else { completion(nil); return }
                        // Deterministic temp name from data hash
                        let hex = d.withUnsafeBytes { ptr -> String in
                            let digest = SHA256.hash(data: Data(ptr))
                            return digest.map { String(format: "%02x", $0) }.joined()
                        }
                        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("album_image_\(hex).jpg")
                        if !FileManager.default.fileExists(atPath: tmp.path) {
                            try d.write(to: tmp, options: .atomic)
                        }
                            let jsonItem: [String: Any] = [
                                "uri": "tempfile://\(tmp.path)",
                                "fileType": "image",
                                "isOriginal": true
                            ]
                            completion(jsonItem)
                    } catch {
                        completion(nil)
                    }
                }
            }
        } else {
            DispatchQueue.main.async { completion(nil) }
        }
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
                    if let fileURL = url, error == nil {
                        DispatchQueue.main.async {
                            do {
                                let tempURL = try LxAppMediaStorage.copyToTemporary(
                                    from: fileURL,
                                    prefix: "album_image",
                                    fallbackExtension: "jpg",
                                    requiresSecurityScope: true
                                )
                            let jsonItem: [String: Any] = [
                                "uri": "tempfile://\(tempURL.path)",
                                "fileType": "image",
                                "isOriginal": true
                            ]
                            completion(jsonItem)
                            } catch {
                                // fallback to object-based load
                                self.loadImageObjectFallback(provider: provider, completion: completion)
                            }
                        }
                    } else {
                        // fallback to object-based load
                        self.loadImageObjectFallback(provider: provider, completion: completion)
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
                    if let fileURL = url, error == nil {
                        DispatchQueue.main.async {
                            do {
                                let tempURL = try LxAppMediaStorage.copyToTemporary(
                                    from: fileURL,
                                    prefix: "album_video",
                                    fallbackExtension: "mov",
                                    requiresSecurityScope: true
                                )
                            let jsonItem: [String: Any] = [
                                "uri": "tempfile://\(tempURL.path)",
                                "fileType": "video",
                                "isOriginal": true
                            ]
                            completion(jsonItem)
                            } catch {
                                completion(nil)
                            }
                        }
                    } else {
                        DispatchQueue.main.async { completion(nil) }
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

#if os(iOS)
import UIKit
import AVFoundation

extension LxAppMedia {
    // Transcode a temporary image file into JPEG at the destination path
    nonisolated static func transcodeTempImageToJpeg(
        src_path: RustStr,
        dest_path: RustStr
    ) -> Bool {
        let src = src_path.toString()
        let dst = dest_path.toString()
        guard !src.isEmpty, !dst.isEmpty else { return false }
        let destURL = URL(fileURLWithPath: dst)
        do {
            try FileManager.default.createDirectory(at: destURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        } catch {
            return false
        }
        guard let image = UIImage(contentsOfFile: src) else { return false }
        guard let data = image.jpegData(compressionQuality: 0.95) else { return false }
        do {
            if FileManager.default.fileExists(atPath: destURL.path) {
                try? FileManager.default.removeItem(at: destURL)
            }
            try data.write(to: destURL, options: .atomic)
            return true
        } catch {
            return false
        }
    }

    // Transcode a temporary video file into MP4 at the destination path
    nonisolated static func transcodeTempVideoToMp4(
        src_path: RustStr,
        dest_path: RustStr
    ) -> Bool {
        let src = src_path.toString()
        let dst = dest_path.toString()
        guard !src.isEmpty, !dst.isEmpty else { return false }
        let sourceURL = URL(fileURLWithPath: src)
        let destURL = URL(fileURLWithPath: dst)
        do {
            try FileManager.default.createDirectory(at: destURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        } catch {
            return false
        }
        let asset = AVAsset(url: sourceURL)
        guard let exporter = AVAssetExportSession(asset: asset, presetName: AVAssetExportPresetHighestQuality) ?? AVAssetExportSession(asset: asset, presetName: AVAssetExportPresetPassthrough) else {
            return false
        }
        guard exporter.supportedFileTypes.contains(.mp4) else { return false }
        if FileManager.default.fileExists(atPath: destURL.path) {
            try? FileManager.default.removeItem(at: destURL)
        }
        exporter.outputURL = destURL
        exporter.outputFileType = .mp4
        exporter.shouldOptimizeForNetworkUse = true
        let semaphore = DispatchSemaphore(value: 0)
        exporter.exportAsynchronously {
            semaphore.signal()
        }
        _ = semaphore.wait(timeout: .now() + 120)
        switch exporter.status {
        case .completed:
            return true
        default:
            return false
        }
    }
}
#endif
