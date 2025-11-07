import Foundation
import os.log
import CLingXiaSwiftAPI
import CLingXiaRustAPI

#if os(iOS)
import UIKit
import AudioToolbox
#endif

@MainActor
enum LxAppMedia {
    nonisolated(unsafe) static let log = OSLog(subsystem: "LingXia", category: "Media")
}

#if os(iOS)
import Photos

extension LxAppMedia {
    private final class MediaBundleToken {}

    /// Compress an image with optional quality, width, and height parameters
    /// - Parameters:
    ///   - sourceUrl: URL string to the source image file
    ///   - quality: JPEG compression quality (0-100), default 80
    ///   - compressedWidth: Optional target width (0 means not specified)
    ///   - compressedHeight: Optional target height (0 means not specified)
    /// - Returns: Path string to the compressed image file, or empty string if compression fails
    @objc public static func compressImage(
        sourceUrl: String,
        quality: Int,
        compressedWidth: Int,
        compressedHeight: Int
    ) -> String {
        guard let url = URL(string: sourceUrl) else {
            os_log(.error, log: log, "Invalid source URL: %@", sourceUrl)
            return ""
        }

        let width = compressedWidth > 0 ? compressedWidth : nil
        let height = compressedHeight > 0 ? compressedHeight : nil

        if let resultURL = compressImageInternal(
            sourceURL: url,
            quality: quality,
            compressedWidth: width,
            compressedHeight: height
        ) {
            return resultURL.path
        }
        return ""
    }

    /// Internal compress an image with optional quality, width, and height parameters
    /// - Parameters:
    ///   - sourceURL: URL to the source image file
    ///   - quality: JPEG compression quality (0-100), default 80
    ///   - compressedWidth: Optional target width
    ///   - compressedHeight: Optional target height
    /// - Returns: URL to the compressed image file, or nil if compression fails
    private static func compressImageInternal(
        sourceURL: URL,
        quality: Int = 80,
        compressedWidth: Int? = nil,
        compressedHeight: Int? = nil
    ) -> URL? {
        guard let image = UIImage(contentsOfFile: sourceURL.path) else {
            os_log(.error, log: log, "Failed to load image from %@", sourceURL.path)
            return nil
        }

        var processedImage = image

        // Resize if dimensions are provided
        if let targetWidth = compressedWidth, let targetHeight = compressedHeight {
            processedImage = resizeImage(image, targetWidth: CGFloat(targetWidth), targetHeight: CGFloat(targetHeight))
        } else if let targetWidth = compressedWidth {
            let aspectRatio = image.size.height / image.size.width
            let targetHeight = CGFloat(targetWidth) * aspectRatio
            processedImage = resizeImage(image, targetWidth: CGFloat(targetWidth), targetHeight: targetHeight)
        } else if let targetHeight = compressedHeight {
            let aspectRatio = image.size.width / image.size.height
            let targetWidth = CGFloat(targetHeight) * aspectRatio
            processedImage = resizeImage(image, targetWidth: targetWidth, targetHeight: CGFloat(targetHeight))
        }

        // Compress as JPEG
        let clampedQuality = max(0, min(100, quality))
        let compressionQuality = CGFloat(clampedQuality) / 100.0

        guard let jpegData = processedImage.jpegData(compressionQuality: compressionQuality) else {
            os_log(.error, log: log, "Failed to compress image to JPEG")
            return nil
        }

        // Save to temporary file
        let tempDir = FileManager.default.temporaryDirectory
        let outputURL = tempDir.appendingPathComponent(UUID().uuidString + ".jpg")

        do {
            try jpegData.write(to: outputURL)
            return outputURL
        } catch {
            os_log(.error, log: log, "Failed to write compressed image: %@", error.localizedDescription)
            return nil
        }
    }

    private static func resizeImage(_ image: UIImage, targetWidth: CGFloat, targetHeight: CGFloat) -> UIImage {
        let size = CGSize(width: targetWidth, height: targetHeight)
        let renderer = UIGraphicsImageRenderer(size: size)
        return renderer.image { _ in
            image.draw(in: CGRect(origin: .zero, size: size))
        }
    }

    static func controlImage(named name: String) -> UIImage? {
        #if SWIFT_PACKAGE
        let bundle = Bundle.module
        #else
        let bundle = Bundle(for: MediaBundleToken.self)
        #endif

        if let image = UIImage(named: name, in: bundle, compatibleWith: nil) {
            return image
        }

        if let pdfURL = bundle.url(forResource: name, withExtension: "pdf") {
            return renderPDF(at: pdfURL)
        }

        return nil
    }

    private static func renderPDF(at url: URL) -> UIImage? {
        guard
            let dataProvider = CGDataProvider(url: url as CFURL),
            let document = CGPDFDocument(dataProvider),
            let page = document.page(at: 1)
        else {
            return nil
        }

        let pageRect = page.getBoxRect(.mediaBox)
        let renderer = UIGraphicsImageRenderer(size: pageRect.size)
        return renderer.image { context in
            let cgContext = context.cgContext
            cgContext.saveGState()
            cgContext.translateBy(x: 0, y: pageRect.height)
            cgContext.scaleBy(x: 1, y: -1)
            cgContext.drawPDFPage(page)
            cgContext.restoreGState()
        }.withRenderingMode(.alwaysOriginal)
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
