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
        let bundle = Bundle.lingxiaResources
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
        let bundle = Bundle.lingxiaResources
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

    /// The running host product's icon — Downloads/Settings, the home lxapp
    /// row, and other chrome that should read as this app, not the SDK mark.
    ///
    /// macOS `AppIcon` follows Apple's icon grid (~10% transparent margin).
    /// Crop that padding so a 16pt sidebar tile matches a full-bleed lxapp PNG.
    @MainActor
    static func hostAppImage() -> NSImage? {
        if let cachedHostAppImage { return cachedHostAppImage }
        guard let raw = NSApp.applicationIconImage else { return nil }
        let image = tightenOpaqueBounds(raw)
        cachedHostAppImage = image
        return image
    }

    @MainActor
    private static var cachedHostAppImage: NSImage?

    /// Bundled LingXia mark for guest lxapps that have no registry artwork.
    static func defaultLxappMark() -> NSImage? {
        Bundle.lingxiaResources.url(
            forResource: "lxapp_default",
            withExtension: "png",
            subdirectory: "icons"
        ).flatMap { NSImage(contentsOf: $0) }
    }

    /// Sidebar / pin artwork for an lxapp. Home is the product itself, so it
    /// always uses the host icon. Other apps use the registry file, then the
    /// SDK default mark.
    @MainActor
    static func lxappImage(appId: String, path: String) -> NSImage? {
        if LxAppCore.isHomeLxApp(appId) {
            return hostAppImage() ?? image(at: path) ?? defaultLxappMark()
        }
        return image(at: path) ?? defaultLxappMark()
    }

    private static func image(at path: String) -> NSImage? {
        guard !path.isEmpty else { return nil }
        return NSImage(contentsOfFile: path)
    }

    /// Square-crop to the opaque plate. Transparent Apple-grid padding is
    /// dropped; artwork that already fills the canvas is left alone.
    private static func tightenOpaqueBounds(_ image: NSImage) -> NSImage {
        guard let tiff = image.tiffRepresentation,
              let rep = NSBitmapImageRep(data: tiff),
              let data = rep.bitmapData,
              rep.hasAlpha,
              rep.samplesPerPixel >= 4
        else { return image }

        let width = rep.pixelsWide
        let height = rep.pixelsHigh
        guard width > 0, height > 0 else { return image }

        let spp = rep.samplesPerPixel
        let alphaIndex = spp - 1
        let bytesPerRow = rep.bytesPerRow
        let threshold: UInt8 = 16

        var minX = width
        var minY = height
        var maxX = 0
        var maxY = 0
        var found = false
        for y in 0..<height {
            let row = data.advanced(by: y * bytesPerRow)
            for x in 0..<width {
                if row[x * spp + alphaIndex] < threshold { continue }
                found = true
                minX = min(minX, x)
                minY = min(minY, y)
                maxX = max(maxX, x)
                maxY = max(maxY, y)
            }
        }
        guard found else { return image }

        let content = max(maxX - minX + 1, maxY - minY + 1)
        // Already full-bleed (less than ~5% inset) — do not re-crop.
        if content * 20 >= max(width, height) * 19 {
            return image
        }

        var startX = (minX + maxX + 1 - content) / 2
        var startY = (minY + maxY + 1 - content) / 2
        startX = max(0, min(startX, width - content))
        startY = max(0, min(startY, height - content))
        let crop = CGRect(x: startX, y: startY, width: content, height: content)
        guard let cropped = rep.cgImage?.cropping(to: crop) else { return image }
        return NSImage(cgImage: cropped, size: NSSize(width: content, height: content))
    }

    /// 16pt menu glyph from `design/icons/svg` (the same PDF iOS capsule
    /// loads via `LxIcon.image(named:)`). No SF Symbol stand-in — missing
    /// names stay empty so platforms cannot drift apart.
    static func menuSymbol(_ name: String) -> NSImage? {
        image(named: name, size: CGSize(width: 16, height: 16))
    }

    /// Lxapp More-action artwork for `NSMenuItem`: 16pt, template when the
    /// source is an SVG so it tints with the menu instead of sitting oversized.
    static func menuImage(fromPath path: String) -> NSImage? {
        guard !path.isEmpty, let source = NSImage(contentsOfFile: path) else {
            return nil
        }
        return TabBarHelper.appKitIcon(source, path: path, size: 16)
    }
}

private class MacOSBundleToken {}
#endif
