import Foundation
import os.log
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
import AudioToolbox
import ImageIO
import UniformTypeIdentifiers
import MobileCoreServices
#elseif os(macOS)
import AppKit
#endif

@MainActor
final class LxAppMedia {
    nonisolated(unsafe) static let log = OSLog(subsystem: "LingXia", category: "Media")
}

#if os(iOS)
extension LxAppMedia {
    private final class MediaBundleToken {}

    nonisolated static func getImageInfo(uri: RustStr) -> SwiftImageInfoResult {
        let rawUri = uri.toString()
        guard !rawUri.isEmpty else {
            return imageInfoFailure("URI is empty")
        }

        guard let url = normalizeURL(from: rawUri) else {
            return imageInfoFailure("Invalid URI: \(rawUri)")
        }

        guard url.isFileURL else {
            return imageInfoFailure("Unsupported URI scheme: \(url.scheme ?? "unknown")")
        }

        return imageInfoFromFile(url: url)
    }

    nonisolated static func compressImage(
        source_uri: RustStr,
        quality: Int32,
        target_width: Int32,
        target_height: Int32,
        output_path: RustStr
    ) -> SwiftCompressImageResult {
        let source = source_uri.toString()
        let outputPath = output_path.toString()
        guard !source.isEmpty else {
            return compressImageFailure("source_uri is empty")
        }
        guard !outputPath.isEmpty else {
            return compressImageFailure("output_path is empty")
        }

        let normalizedQuality = max(0, min(100, Int(quality)))
        let width = target_width > 0 ? Int(target_width) : nil
        let height = target_height > 0 ? Int(target_height) : nil
        let destinationURL = URL(fileURLWithPath: outputPath)

        do {
            let parentDir = destinationURL.deletingLastPathComponent()
            if !parentDir.path.isEmpty {
                try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)
            }
        } catch {
            return compressImageFailure("Failed to prepare output path: \(error.localizedDescription)")
        }
        guard let sourceURL = normalizeURL(from: source) else {
            return compressImageFailure("Invalid source URI: \(source)")
        }
        guard sourceURL.isFileURL else {
            return compressImageFailure("Only local file URLs are supported for compression")
        }
        if compressImageInternal(
            sourceURL: sourceURL,
            quality: normalizedQuality,
            compressedWidth: width,
            compressedHeight: height,
            outputURL: destinationURL
        ) == nil {
            try? FileManager.default.removeItem(at: destinationURL)
            return compressImageFailure("Failed to compress image")
        }

        return SwiftCompressImageResult(
            success: true,
            error: RustString(""),
            path: RustString(destinationURL.path)
        )
    }

    /// Internal compress an image with optional quality, width, and height parameters
    /// - Parameters:
    ///   - sourceURL: URL to the source image file
    ///   - quality: JPEG compression quality (0-100), default 80
    ///   - compressedWidth: Optional target width
    ///   - compressedHeight: Optional target height
    /// - Returns: URL to the compressed image file, or nil if compression fails
    nonisolated private static func compressImageInternal(
        sourceURL: URL,
        quality: Int = 80,
        compressedWidth: Int? = nil,
        compressedHeight: Int? = nil,
        outputURL: URL? = nil
    ) -> URL? {
        guard let image = UIImage(contentsOfFile: sourceURL.path) else {
            os_log(.error, log: log, "Failed to load image from %@", sourceURL.path)
            return nil
        }

        let destination = outputURL ?? FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".jpg")
        guard writeCompressedImage(
            image: image,
            destinationURL: destination,
            quality: quality,
            targetWidth: compressedWidth,
            targetHeight: compressedHeight
        ) else {
            return nil
        }

        return destination
    }

    nonisolated private static func resizeImage(_ image: UIImage, targetWidth: CGFloat, targetHeight: CGFloat) -> UIImage {
        let size = CGSize(width: targetWidth, height: targetHeight)
        let renderer = UIGraphicsImageRenderer(size: size)
        return renderer.image { _ in
            image.draw(in: CGRect(origin: .zero, size: size))
        }
    }

    nonisolated private static func makeCompressedImageData(
        image: UIImage,
        quality: Int,
        targetWidth: Int?,
        targetHeight: Int?
    ) -> Data? {
        var processedImage = image

        if let width = targetWidth, let height = targetHeight {
            processedImage = resizeImage(image, targetWidth: CGFloat(width), targetHeight: CGFloat(height))
        } else if let width = targetWidth {
            let aspectRatio = image.size.height / image.size.width
            let computedHeight = CGFloat(width) * aspectRatio
            processedImage = resizeImage(image, targetWidth: CGFloat(width), targetHeight: computedHeight)
        } else if let height = targetHeight {
            let aspectRatio = image.size.width / image.size.height
            let computedWidth = CGFloat(height) * aspectRatio
            processedImage = resizeImage(image, targetWidth: computedWidth, targetHeight: CGFloat(height))
        }

        let clampedQuality = max(0, min(100, quality))
        let compressionQuality = CGFloat(clampedQuality) / 100.0

        guard let jpegData = processedImage.jpegData(compressionQuality: compressionQuality) else {
            os_log(.error, log: log, "Failed to compress image to JPEG")
            return nil
        }

        return jpegData
    }

    nonisolated private static func writeCompressedImage(
        image: UIImage,
        destinationURL: URL,
        quality: Int,
        targetWidth: Int?,
        targetHeight: Int?
    ) -> Bool {
        guard let jpegData = makeCompressedImageData(
            image: image,
            quality: quality,
            targetWidth: targetWidth,
            targetHeight: targetHeight
        ) else {
            return false
        }

        do {
            try jpegData.write(to: destinationURL, options: .atomic)
            return true
        } catch {
            os_log(.error, log: log, "Failed to write compressed image: %@", error.localizedDescription)
            return false
        }
    }

    private struct ImageInfoPayload {
        let width: Int
        let height: Int
        let mimeType: String
    }

    nonisolated private static func imageInfoFromFile(url: URL) -> SwiftImageInfoResult {
        guard let info = readImageProperties(url: url) else {
            return imageInfoFailure("Failed to inspect image at \(url.path)")
        }
        return imageInfoSuccess(info)
    }

    nonisolated private static func readImageProperties(url: URL) -> ImageInfoPayload? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else {
            return nil
        }
        guard let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any] else {
            return nil
        }

        let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.intValue ?? 0
        let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.intValue ?? 0
        let uti = CGImageSourceGetType(source) as String?
        var mimeType = preferredMimeType(for: uti)
        if mimeType.isEmpty {
            mimeType = mimeTypeFromExtension(url.pathExtension.lowercased())
        }

        return ImageInfoPayload(
            width: width,
            height: height,
            mimeType: mimeType
        )
    }

    nonisolated private static func preferredMimeType(for uti: String?) -> String {
        guard let uti = uti, !uti.isEmpty else {
            return ""
        }
        if #available(iOS 14.0, *) {
            return UTType(uti)?.preferredMIMEType ?? ""
        } else {
            return (UTTypeCopyPreferredTagWithClass(uti as CFString, kUTTagClassMIMEType)?.takeRetainedValue() as String?) ?? ""
        }
    }

    nonisolated private static func mimeTypeFromExtension(_ ext: String) -> String {
        guard !ext.isEmpty else { return "" }
        if #available(iOS 14.0, *) {
            return UTType(filenameExtension: ext)?.preferredMIMEType ?? ""
        } else {
            guard let uti = UTTypeCreatePreferredIdentifierForTag(
                kUTTagClassFilenameExtension,
                ext as CFString,
                nil
            )?.takeRetainedValue() else {
                return ""
            }
            return (UTTypeCopyPreferredTagWithClass(uti, kUTTagClassMIMEType)?.takeRetainedValue() as String?) ?? ""
        }
    }

    nonisolated private static func normalizeURL(from path: String) -> URL? {
        if path.hasPrefix("file://") {
            return URL(string: path)
        }
        if let url = URL(string: path), url.scheme != nil {
            return url
        }
        return URL(fileURLWithPath: path)
    }

    nonisolated private static func imageInfoFailure(_ message: String) -> SwiftImageInfoResult {
        return SwiftImageInfoResult(
            success: false,
            error: RustString(message),
            width: 0,
            height: 0,
            mime_type: RustString("")
        )
    }

    nonisolated private static func imageInfoSuccess(_ info: ImageInfoPayload) -> SwiftImageInfoResult {
        return SwiftImageInfoResult(
            success: true,
            error: RustString(""),
            width: UInt32(clamping: info.width),
            height: UInt32(clamping: info.height),
            mime_type: RustString(info.mimeType)
        )
    }

    nonisolated private static func compressImageFailure(_ message: String) -> SwiftCompressImageResult {
        return SwiftCompressImageResult(
            success: false,
            error: RustString(message),
            path: RustString("")
        )
    }

    enum CaptureFeedback {
        private static func play(soundID: SystemSoundID) {
            AudioServicesPlaySystemSound(soundID)
            if #available(iOS 10.0, *) {
                let generator = UIImpactFeedbackGenerator(style: .light)
                generator.prepare()
                generator.impactOccurred()
            }
        }

        static func playShutter() {
            play(soundID: 1108)
        }

        static func playRecordStart() {
            play(soundID: 1117)
        }

        static func playRecordStop() {
            play(soundID: 1118)
        }
    }

    static func topViewController(
        base: UIViewController? = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap { $0.windows }
            .first(where: { $0.isKeyWindow })?.rootViewController
    ) -> UIViewController? {
        if let nav = base as? UINavigationController {
            return topViewController(base: nav.visibleViewController)
        }
        if let tab = base as? UITabBarController {
            if let selected = tab.selectedViewController {
                return topViewController(base: selected)
            }
        }
        if let presented = base?.presentedViewController {
            return topViewController(base: presented)
        }
        return base
    }
}
#endif

#if os(macOS)
// MARK: - macOS Stub Implementations

extension LxAppMedia {
    nonisolated static func getImageInfo(uri: RustStr) -> SwiftImageInfoResult {
        return SwiftImageInfoResult(
            success: false,
            error: RustString("Not implemented on macOS"),
            width: 0,
            height: 0,
            mime_type: RustString("")
        )
    }

    nonisolated static func compressImage(
        source_uri: RustStr,
        quality: Int32,
        target_width: Int32,
        target_height: Int32,
        output_path: RustStr
    ) -> SwiftCompressImageResult {
        return SwiftCompressImageResult(
            success: false,
            error: RustString("Not implemented on macOS"),
            path: RustString("")
        )
    }

    nonisolated static func previewMedia(items_json: RustStr) -> Bool {
        os_log("previewMedia not implemented on macOS", log: log, type: .error)
        return false
    }

    nonisolated static func scanCode(
        scan_types_json: RustStr,
        only_from_camera: Bool,
        callback_id: UInt64
    ) -> Bool {
        os_log("scanCode not implemented on macOS", log: log, type: .error)
        return false
    }

    nonisolated static func copyAlbumMediaToFile(
        uri: RustStr,
        destination_path: RustStr,
        media_type: Int32
    ) -> Bool {
        os_log("copyAlbumMediaToFile not implemented on macOS", log: log, type: .error)
        return false
    }
}
#endif
