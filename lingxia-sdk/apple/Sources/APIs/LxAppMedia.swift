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
extension LxAppMedia {
    private final class MediaBundleToken {}

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
