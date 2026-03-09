#if os(iOS)
import UIKit
import Photos
import CLingXiaRustAPI

extension LxAppMedia {
    // Copy album media to destination.
    // Images are normalized to JPEG; videos keep original playable bytes.
    // Supported URIs: phasset:<localIdentifier> only (album assets)
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
        if !isVideo {
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
            let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(
                "phasset_video_\(UUID().uuidString).\(ext.isEmpty ? "mov" : ext)"
            )
            let opt = PHAssetResourceRequestOptions()
            opt.isNetworkAccessAllowed = true
            let resources = PHAssetResource.assetResources(for: asset)
            let preferred: [PHAssetResourceType] = [.video, .fullSizeVideo, .pairedVideo]
            let res = resources.first { preferred.contains($0.type) } ?? resources.first
            guard let assetRes = res else { return false }
            let sem = DispatchSemaphore(value: 0)
            var ok = false
            try? FileManager.default.removeItem(at: tmp)
            PHAssetResourceManager.default().writeData(for: assetRes, toFile: tmp, options: opt) { error in
                ok = (error == nil)
                sem.signal()
            }
            let waitResult = sem.wait(timeout: .now() + 180)
            guard waitResult == .success, ok else {
                try? FileManager.default.removeItem(at: tmp)
                clearDest()
                return false
            }
            clearDest()
            do {
                try FileManager.default.moveItem(at: tmp, to: destURL)
                let size = (try? FileManager.default.attributesOfItem(atPath: destURL.path)[.size] as? NSNumber)?
                    .int64Value ?? 0
                return size > 0
            } catch {
                try? FileManager.default.removeItem(at: tmp)
                clearDest()
                return false
            }
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
    static var preparing: String { "lx_camera_preparing".localized }
    static var ready: String { "lx_camera_tap_to_capture".localized }
    static var switching: String { "lx_camera_switching".localized }
}
#endif
