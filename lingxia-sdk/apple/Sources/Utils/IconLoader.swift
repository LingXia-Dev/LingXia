import Foundation

#if os(iOS)
import UIKit

/// Icon loader for LingXia SDK - loads icons from generated assets (PDF from SVG)
enum LxIcon {
    /// Load control icon by name from SDK bundle, optionally scaled to a specific size
    /// Icons are stored as PDF files generated from SVG sources
    static func image(named name: String, size: CGSize? = nil) -> UIImage? {
        guard let baseImage = loadImage(named: name) else { return nil }

        // If no size specified, return the base image
        guard let targetSize = size else { return baseImage }

        // Scale to target size
        let renderer = UIGraphicsImageRenderer(size: targetSize)
        let scaledImage = renderer.image { _ in
            baseImage.draw(in: CGRect(origin: .zero, size: targetSize))
        }
        return scaledImage.withRenderingMode(.alwaysTemplate)
    }

    private static func loadImage(named name: String) -> UIImage? {
        #if SWIFT_PACKAGE
        let bundle = Bundle.module
        #else
        let bundle = Bundle(for: MediaBundleToken.self)
        #endif

        if let image = UIImage(named: name, in: bundle, compatibleWith: nil) {
            return image
        }

        // loading PDF from icons subdirectory (Resources/icons)
        if let pdfURL = bundle.url(forResource: name, withExtension: "pdf", subdirectory: "icons") {
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

        let rect = page.getBoxRect(.mediaBox)
        let scale = UIScreen.main.scale
        let size = CGSize(width: rect.width * scale, height: rect.height * scale)

        UIGraphicsBeginImageContextWithOptions(CGSize(width: rect.width, height: rect.height), false, scale)
        defer { UIGraphicsEndImageContext() }

        guard let ctx = UIGraphicsGetCurrentContext() else { return nil }
        ctx.setFillColor(UIColor.clear.cgColor)
        ctx.fill(CGRect(origin: .zero, size: size))

        ctx.translateBy(x: 0, y: rect.height)
        ctx.scaleBy(x: 1, y: -1)
        ctx.drawPDFPage(page)

        return UIGraphicsGetImageFromCurrentImageContext()?.withRenderingMode(.alwaysTemplate)
    }
}

private class MediaBundleToken {}

#elseif os(macOS)
import AppKit

/// Icon loader for LingXia SDK - loads icons from generated assets
enum LxIcon {
    /// Load control icon by name from SDK bundle, optionally scaled to a specific size
    /// Icons are stored as PDF files generated from SVG sources
    static func image(named name: String, size: CGSize? = nil) -> NSImage? {
        guard let baseImage = loadImage(named: name) else { return nil }
        guard let targetSize = normalizedSize(size) else { return baseImage }

        if let copiedImage = baseImage.copy() as? NSImage {
            copiedImage.size = targetSize
            copiedImage.isTemplate = true
            return copiedImage
        }

        baseImage.size = targetSize
        baseImage.isTemplate = true
        return baseImage
    }

    private static func loadImage(named name: String) -> NSImage? {
        #if SWIFT_PACKAGE
        let bundle = Bundle.module
        #else
        let bundle = Bundle(for: MacOSBundleToken.self)
        #endif

        if let image = bundle.image(forResource: name) {
            image.isTemplate = true
            return image
        }

        if let pdfURL = bundle.url(forResource: name, withExtension: "pdf", subdirectory: "icons"),
           let image = NSImage(contentsOf: pdfURL) {
            image.isTemplate = true
            return image
        }

        return nil
    }

    private static func normalizedSize(_ size: CGSize?) -> CGSize? {
        guard let size else { return nil }
        guard size.width > 0, size.height > 0 else { return nil }
        return size
    }
}

private class MacOSBundleToken {}
#endif
