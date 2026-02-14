import Foundation
import os.log
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
import AVFoundation
import AudioToolbox
import ImageIO
import UniformTypeIdentifiers
import MobileCoreServices
#elseif os(macOS)
import AppKit
import AVFoundation
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

    nonisolated static func getVideoInfo(uri: RustStr) -> SwiftVideoInfoResult {
        let rawUri = uri.toString()
        guard !rawUri.isEmpty else {
            return videoInfoFailure("URI is empty")
        }
        guard let sourceURL = normalizeURL(from: rawUri) else {
            return videoInfoFailure("Invalid URI: \(rawUri)")
        }
        guard sourceURL.isFileURL else {
            return videoInfoFailure("Unsupported URI scheme: \(sourceURL.scheme ?? "unknown")")
        }

        let asset = AVURLAsset(url: sourceURL)
        guard let videoTrack = asset.tracks(withMediaType: .video).first else {
            return videoInfoFailure("No video track found")
        }

        let transformedSize = videoTrack.naturalSize.applying(videoTrack.preferredTransform)
        let width = Int(abs(transformedSize.width.rounded()))
        let height = Int(abs(transformedSize.height.rounded()))

        let durationSeconds = CMTimeGetSeconds(asset.duration)
        let durationMs: UInt64
        if durationSeconds.isFinite && durationSeconds >= 0 {
            durationMs = UInt64((durationSeconds * 1000.0).rounded())
        } else {
            durationMs = 0
        }

        let rotation = normalizedRotationDegrees(videoTrack.preferredTransform)
        let bitrate = videoTrack.estimatedDataRate > 0 ? UInt64(videoTrack.estimatedDataRate.rounded()) : nil
        let fps = videoTrack.nominalFrameRate > 0 ? videoTrack.nominalFrameRate : nil
        let mimeType = mimeTypeFromExtension(sourceURL.pathExtension.lowercased())

        return SwiftVideoInfoResult(
            success: true,
            error: RustString(""),
            width: UInt32(clamping: width),
            height: UInt32(clamping: height),
            duration_ms: durationMs,
            rotation: rotation ?? 0,
            has_rotation: rotation != nil,
            bitrate: bitrate ?? 0,
            has_bitrate: bitrate != nil,
            fps: fps ?? 0,
            has_fps: fps != nil,
            mime_type: RustString(mimeType)
        )
    }

    nonisolated static func extractVideoThumbnail(
        source_uri: RustStr,
        quality: Int32,
        target_width: Int32,
        target_height: Int32,
        time_ms: Int64,
        output_path: RustStr
    ) -> SwiftVideoThumbnailResult {
        let source = source_uri.toString()
        let outputPath = output_path.toString()
        guard !source.isEmpty else {
            return extractVideoThumbnailFailure("source_uri is empty")
        }
        guard !outputPath.isEmpty else {
            return extractVideoThumbnailFailure("output_path is empty")
        }
        guard let sourceURL = normalizeURL(from: source) else {
            return extractVideoThumbnailFailure("Invalid source URI: \(source)")
        }
        guard sourceURL.isFileURL else {
            return extractVideoThumbnailFailure("Only local file URLs are supported for thumbnail extraction")
        }

        let destinationURL = URL(fileURLWithPath: outputPath)
        do {
            let parentDir = destinationURL.deletingLastPathComponent()
            if !parentDir.path.isEmpty {
                try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)
            }
        } catch {
            return extractVideoThumbnailFailure("Failed to prepare output path: \(error.localizedDescription)")
        }

        let asset = AVURLAsset(url: sourceURL)
        let imageGenerator = AVAssetImageGenerator(asset: asset)
        imageGenerator.appliesPreferredTrackTransform = true
        // Avoid snapping to distant keyframes so timeMs has predictable effect.
        imageGenerator.requestedTimeToleranceBefore = .zero
        imageGenerator.requestedTimeToleranceAfter = .zero

        let requestedSeconds = max(0.0, Double(time_ms) / 1000.0)
        let requestedTime = CMTime(seconds: requestedSeconds, preferredTimescale: 600)

        let cgImage: CGImage
        do {
            var actualTime = CMTime.zero
            cgImage = try imageGenerator.copyCGImage(at: requestedTime, actualTime: &actualTime)
        } catch {
            return extractVideoThumbnailFailure("Failed to decode frame: \(error.localizedDescription)")
        }

        var image = UIImage(cgImage: cgImage)
        let maxWidth = target_width > 0 ? Int(target_width) : nil
        let maxHeight = target_height > 0 ? Int(target_height) : nil
        if let resized = resizeVideoImage(image, maxWidth: maxWidth, maxHeight: maxHeight) {
            image = resized
        }

        let normalizedQuality = max(0, min(100, Int(quality)))
        guard let data = image.jpegData(compressionQuality: CGFloat(normalizedQuality) / 100.0) else {
            return extractVideoThumbnailFailure("Failed to encode JPEG")
        }
        do {
            try data.write(to: destinationURL, options: .atomic)
        } catch {
            return extractVideoThumbnailFailure("Failed to write thumbnail: \(error.localizedDescription)")
        }

        return SwiftVideoThumbnailResult(
            success: true,
            error: RustString(""),
            path: RustString(destinationURL.path),
            width: UInt32(clamping: Int(image.size.width.rounded())),
            height: UInt32(clamping: Int(image.size.height.rounded())),
            mime_type: RustString("image/jpeg")
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

    nonisolated private static func normalizedRotationDegrees(_ transform: CGAffineTransform) -> Int32? {
        let epsilon: CGFloat = 0.001
        let isZero = { (value: CGFloat) -> Bool in abs(value) < epsilon }
        let equals = { (lhs: CGFloat, rhs: CGFloat) -> Bool in abs(lhs - rhs) < epsilon }

        if isZero(transform.a), equals(transform.b, 1), equals(transform.c, -1), isZero(transform.d) {
            return 90
        }
        if isZero(transform.a), equals(transform.b, -1), equals(transform.c, 1), isZero(transform.d) {
            return 270
        }
        if equals(transform.a, -1), isZero(transform.b), isZero(transform.c), equals(transform.d, -1) {
            return 180
        }
        if equals(transform.a, 1), isZero(transform.b), isZero(transform.c), equals(transform.d, 1) {
            return 0
        }
        return nil
    }

    nonisolated private static func resizeVideoImage(
        _ image: UIImage,
        maxWidth: Int?,
        maxHeight: Int?
    ) -> UIImage? {
        let originalWidth = image.size.width
        let originalHeight = image.size.height
        guard originalWidth > 0, originalHeight > 0 else {
            return nil
        }

        var ratio: CGFloat = 1
        if let width = maxWidth, let height = maxHeight {
            let widthRatio = CGFloat(width) / originalWidth
            let heightRatio = CGFloat(height) / originalHeight
            ratio = min(widthRatio, heightRatio)
        } else if let width = maxWidth {
            ratio = CGFloat(width) / originalWidth
        } else if let height = maxHeight {
            ratio = CGFloat(height) / originalHeight
        } else {
            return nil
        }

        if ratio >= 1 {
            return nil
        }
        let targetWidth = max(1, Int((originalWidth * ratio).rounded()))
        let targetHeight = max(1, Int((originalHeight * ratio).rounded()))
        return resizeImage(image, targetWidth: CGFloat(targetWidth), targetHeight: CGFloat(targetHeight))
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

    nonisolated private static func videoInfoFailure(_ message: String) -> SwiftVideoInfoResult {
        return SwiftVideoInfoResult(
            success: false,
            error: RustString(message),
            width: 0,
            height: 0,
            duration_ms: 0,
            rotation: 0,
            has_rotation: false,
            bitrate: 0,
            has_bitrate: false,
            fps: 0,
            has_fps: false,
            mime_type: RustString("")
        )
    }

    nonisolated private static func extractVideoThumbnailFailure(_ message: String) -> SwiftVideoThumbnailResult {
        return SwiftVideoThumbnailResult(
            success: false,
            error: RustString(message),
            path: RustString(""),
            width: 0,
            height: 0,
            mime_type: RustString("")
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
// MARK: - macOS Implementations

extension LxAppMedia {
    nonisolated static func getImageInfo(uri: RustStr) -> SwiftImageInfoResult {
        return SwiftImageInfoResult(
            success: false,
            error: RustString("Not implemented on macOS - use Rust implementation"),
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
            error: RustString("Not implemented on macOS - use Rust implementation"),
            path: RustString("")
        )
    }

    nonisolated static func getVideoInfo(uri: RustStr) -> SwiftVideoInfoResult {
        let rawUri = uri.toString()
        guard !rawUri.isEmpty else {
            return videoInfoFailureMac("URI is empty")
        }
        guard let sourceURL = normalizeURLMac(from: rawUri) else {
            return videoInfoFailureMac("Invalid URI: \(rawUri)")
        }
        guard sourceURL.isFileURL else {
            return videoInfoFailureMac("Unsupported URI scheme: \(sourceURL.scheme ?? "unknown")")
        }

        let asset = AVURLAsset(url: sourceURL)
        guard let videoTrack = asset.tracks(withMediaType: .video).first else {
            return videoInfoFailureMac("No video track found")
        }

        let transformedSize = videoTrack.naturalSize.applying(videoTrack.preferredTransform)
        let width = Int(abs(transformedSize.width.rounded()))
        let height = Int(abs(transformedSize.height.rounded()))

        let durationSeconds = CMTimeGetSeconds(asset.duration)
        let durationMs: UInt64
        if durationSeconds.isFinite && durationSeconds >= 0 {
            durationMs = UInt64((durationSeconds * 1000.0).rounded())
        } else {
            durationMs = 0
        }

        let rotation = normalizedRotationDegreesMac(videoTrack.preferredTransform)
        let bitrate = videoTrack.estimatedDataRate > 0 ? UInt64(videoTrack.estimatedDataRate.rounded()) : nil
        let fps = videoTrack.nominalFrameRate > 0 ? videoTrack.nominalFrameRate : nil
        let mimeType = inferVideoMimeTypeMac(sourceURL.pathExtension.lowercased())

        return SwiftVideoInfoResult(
            success: true,
            error: RustString(""),
            width: UInt32(clamping: width),
            height: UInt32(clamping: height),
            duration_ms: durationMs,
            rotation: rotation ?? 0,
            has_rotation: rotation != nil,
            bitrate: bitrate ?? 0,
            has_bitrate: bitrate != nil,
            fps: fps ?? 0,
            has_fps: fps != nil,
            mime_type: RustString(mimeType)
        )
    }

    nonisolated static func extractVideoThumbnail(
        source_uri: RustStr,
        quality: Int32,
        target_width: Int32,
        target_height: Int32,
        time_ms: Int64,
        output_path: RustStr
    ) -> SwiftVideoThumbnailResult {
        let source = source_uri.toString()
        let outputPath = output_path.toString()
        guard !source.isEmpty else {
            return extractVideoThumbnailFailureMac("source_uri is empty")
        }
        guard !outputPath.isEmpty else {
            return extractVideoThumbnailFailureMac("output_path is empty")
        }
        guard let sourceURL = normalizeURLMac(from: source) else {
            return extractVideoThumbnailFailureMac("Invalid source URI: \(source)")
        }
        guard sourceURL.isFileURL else {
            return extractVideoThumbnailFailureMac("Only local file URLs are supported for thumbnail extraction")
        }

        let destinationURL = URL(fileURLWithPath: outputPath)
        do {
            let parentDir = destinationURL.deletingLastPathComponent()
            if !parentDir.path.isEmpty {
                try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)
            }
        } catch {
            return extractVideoThumbnailFailureMac("Failed to prepare output path: \(error.localizedDescription)")
        }

        let asset = AVURLAsset(url: sourceURL)
        let imageGenerator = AVAssetImageGenerator(asset: asset)
        imageGenerator.appliesPreferredTrackTransform = true
        // Avoid snapping to distant keyframes so timeMs has predictable effect.
        imageGenerator.requestedTimeToleranceBefore = .zero
        imageGenerator.requestedTimeToleranceAfter = .zero

        let requestedSeconds = max(0.0, Double(time_ms) / 1000.0)
        let requestedTime = CMTime(seconds: requestedSeconds, preferredTimescale: 600)

        let cgImage: CGImage
        do {
            var actualTime = CMTime.zero
            cgImage = try imageGenerator.copyCGImage(at: requestedTime, actualTime: &actualTime)
        } catch {
            return extractVideoThumbnailFailureMac("Failed to decode frame: \(error.localizedDescription)")
        }

        let maxWidth = target_width > 0 ? Int(target_width) : nil
        let maxHeight = target_height > 0 ? Int(target_height) : nil
        let finalImage = resizeCGImageMac(cgImage, maxWidth: maxWidth, maxHeight: maxHeight) ?? cgImage

        let bitmapRep = NSBitmapImageRep(cgImage: finalImage)
        let normalizedQuality = max(0, min(100, Int(quality)))
        let properties: [NSBitmapImageRep.PropertyKey: Any] = [
            .compressionFactor: Double(normalizedQuality) / 100.0
        ]
        guard let jpegData = bitmapRep.representation(using: .jpeg, properties: properties) else {
            return extractVideoThumbnailFailureMac("Failed to encode JPEG")
        }
        do {
            try jpegData.write(to: destinationURL, options: .atomic)
        } catch {
            return extractVideoThumbnailFailureMac("Failed to write thumbnail: \(error.localizedDescription)")
        }

        return SwiftVideoThumbnailResult(
            success: true,
            error: RustString(""),
            path: RustString(destinationURL.path),
            width: UInt32(clamping: bitmapRep.pixelsWide),
            height: UInt32(clamping: bitmapRep.pixelsHigh),
            mime_type: RustString("image/jpeg")
        )
    }

    nonisolated private static func normalizeURLMac(from path: String) -> URL? {
        if path.hasPrefix("file://") {
            return URL(string: path)
        }
        if let url = URL(string: path), url.scheme != nil {
            return url
        }
        return URL(fileURLWithPath: path)
    }

    nonisolated private static func inferVideoMimeTypeMac(_ ext: String) -> String {
        switch ext.lowercased() {
        case "mp4", "m4v":
            return "video/mp4"
        case "mov":
            return "video/quicktime"
        case "webm":
            return "video/webm"
        case "mkv":
            return "video/x-matroska"
        case "avi":
            return "video/x-msvideo"
        case "3gp", "3gpp":
            return "video/3gpp"
        default:
            return ""
        }
    }

    nonisolated private static func normalizedRotationDegreesMac(_ transform: CGAffineTransform) -> Int32? {
        let epsilon: CGFloat = 0.001
        let isZero = { (value: CGFloat) -> Bool in abs(value) < epsilon }
        let equals = { (lhs: CGFloat, rhs: CGFloat) -> Bool in abs(lhs - rhs) < epsilon }

        if isZero(transform.a), equals(transform.b, 1), equals(transform.c, -1), isZero(transform.d) {
            return 90
        }
        if isZero(transform.a), equals(transform.b, -1), equals(transform.c, 1), isZero(transform.d) {
            return 270
        }
        if equals(transform.a, -1), isZero(transform.b), isZero(transform.c), equals(transform.d, -1) {
            return 180
        }
        if equals(transform.a, 1), isZero(transform.b), isZero(transform.c), equals(transform.d, 1) {
            return 0
        }
        return nil
    }

    nonisolated private static func resizeCGImageMac(
        _ image: CGImage,
        maxWidth: Int?,
        maxHeight: Int?
    ) -> CGImage? {
        let originalWidth = image.width
        let originalHeight = image.height
        guard originalWidth > 0, originalHeight > 0 else {
            return nil
        }

        var ratio: Double = 1
        if let width = maxWidth, let height = maxHeight {
            let widthRatio = Double(width) / Double(originalWidth)
            let heightRatio = Double(height) / Double(originalHeight)
            ratio = min(widthRatio, heightRatio)
        } else if let width = maxWidth {
            ratio = Double(width) / Double(originalWidth)
        } else if let height = maxHeight {
            ratio = Double(height) / Double(originalHeight)
        } else {
            return nil
        }

        if ratio >= 1 {
            return nil
        }

        let targetWidth = max(1, Int((Double(originalWidth) * ratio).rounded()))
        let targetHeight = max(1, Int((Double(originalHeight) * ratio).rounded()))

        guard let colorSpace = image.colorSpace ?? CGColorSpace(name: CGColorSpace.sRGB) else {
            return nil
        }
        guard let context = CGContext(
            data: nil,
            width: targetWidth,
            height: targetHeight,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: targetWidth, height: targetHeight))
        return context.makeImage()
    }

    nonisolated private static func videoInfoFailureMac(_ message: String) -> SwiftVideoInfoResult {
        return SwiftVideoInfoResult(
            success: false,
            error: RustString(message),
            width: 0,
            height: 0,
            duration_ms: 0,
            rotation: 0,
            has_rotation: false,
            bitrate: 0,
            has_bitrate: false,
            fps: 0,
            has_fps: false,
            mime_type: RustString("")
        )
    }

    nonisolated private static func extractVideoThumbnailFailureMac(_ message: String) -> SwiftVideoThumbnailResult {
        return SwiftVideoThumbnailResult(
            success: false,
            error: RustString(message),
            path: RustString(""),
            width: 0,
            height: 0,
            mime_type: RustString("")
        )
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
