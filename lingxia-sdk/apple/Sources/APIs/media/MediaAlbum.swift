#if os(iOS)
import UIKit
import Photos
import AVFoundation
import CLingXiaRustAPI

extension LxAppMedia {
    // Copy album media to destination with normalized output format.
    // Image → JPEG (.jpg/.jpeg), Video → MP4 (.mp4)
    // Supported URIs: phasset:<localIdentifier> only (album assets)
    // Images are compressed to 80% quality by default
    nonisolated static func copyAlbumMediaToFile(
        uri: RustStr,
        destination_path: RustStr,
        media_type: Int32
    ) -> Bool {
        let rawUri = uri.toString()
        let dest = destination_path.toString()
        guard !rawUri.isEmpty, !dest.isEmpty else { return false }

        let isVideo = (media_type == 1)
        let destURL = URL(fileURLWithPath: dest)
        do {
            try FileManager.default.createDirectory(at: destURL.deletingLastPathComponent(), withIntermediateDirectories: true)
        } catch { return false }

        // Enforce destination extension contract
        let ext = destURL.pathExtension.lowercased()
        if isVideo {
            if ext != "mp4" { return false }
        } else {
            if ext != "jpg" && ext != "jpeg" { return false }
        }

        func clearDest() {
            if FileManager.default.fileExists(atPath: destURL.path) {
                try? FileManager.default.removeItem(at: destURL)
            }
        }

        func writeJpeg(from srcPath: String) -> Bool {
            guard let image = UIImage(contentsOfFile: srcPath) else { return false }
            // Default 80% JPEG compression
            guard let data = image.jpegData(compressionQuality: 0.8) else { return false }
            clearDest()
            do { try data.write(to: destURL, options: .atomic); return true } catch { return false }
        }

        // phasset only
        let assetId: String?
        if rawUri.hasPrefix("phasset:") {
            assetId = String(rawUri.dropFirst("phasset:".count))
        } else {
            assetId = nil
        }

        guard let localId = assetId, !localId.isEmpty else { return false }
        let fetch = PHAsset.fetchAssets(withLocalIdentifiers: [localId], options: nil)
        guard let asset = fetch.firstObject else { return false }

        if isVideo {
            // Export AVAsset directly to MP4 at destination
            let opt = PHVideoRequestOptions()
            opt.isNetworkAccessAllowed = true
            opt.deliveryMode = .highQualityFormat
            let sem = DispatchSemaphore(value: 0)
            var ok = true
            PHImageManager.default().requestAVAsset(forVideo: asset, options: opt) { avAsset, _, _ in
                guard let avAsset = avAsset else { ok = false; sem.signal(); return }
                let preset = AVAssetExportSession(asset: avAsset, presetName: AVAssetExportPresetHighestQuality) != nil ? AVAssetExportPresetHighestQuality : AVAssetExportPresetPassthrough
                guard let exporter = AVAssetExportSession(asset: avAsset, presetName: preset) else { ok = false; sem.signal(); return }
                if !exporter.supportedFileTypes.contains(.mp4) { ok = false; sem.signal(); return }
                clearDest()
                exporter.outputURL = destURL
                exporter.outputFileType = .mp4
                exporter.shouldOptimizeForNetworkUse = true
                exporter.exportAsynchronously {
                    ok = (exporter.status == .completed)
                    sem.signal()
                }
            }
            _ = sem.wait(timeout: .now() + 180)
            return ok && FileManager.default.fileExists(atPath: destURL.path)
        } else {
            // Export original bytes to a temp file, then transcode to JPEG
            let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("phasset_export_\(UUID().uuidString).dat")
            let opt = PHAssetResourceRequestOptions()
            opt.isNetworkAccessAllowed = true
            let resources = PHAssetResource.assetResources(for: asset)
            let preferred: [PHAssetResourceType] = [.photo, .fullSizePhoto, .adjustmentBasePhoto]
            let res = resources.first { preferred.contains($0.type) } ?? resources.first
            guard let assetRes = res else { return false }
            var ok = true
            let sem = DispatchSemaphore(value: 0)
            PHAssetResourceManager.default().writeData(for: assetRes, toFile: tmp, options: opt) { error in
                if error != nil { ok = false }
                sem.signal()
            }
            _ = sem.wait(timeout: .now() + 120)
            if !ok { return false }
            let done = writeJpeg(from: tmp.path)
            try? FileManager.default.removeItem(at: tmp)
            return done
        }
    }
}

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
