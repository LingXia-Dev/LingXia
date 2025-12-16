import UIKit

/// Icon loader for LingXia SDK - loads icons from generated assets (PDF from SVG)
public enum LxIcon {
    /// Load control icon by name from SDK bundle
    /// Icons are stored as PDF files generated from SVG sources
    public static func image(named name: String) -> UIImage? {
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
