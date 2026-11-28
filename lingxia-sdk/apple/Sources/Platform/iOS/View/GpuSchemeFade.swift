import Foundation
#if os(iOS)
import UIKit
import OSLog
import QuartzCore
import CLingXiaRustAPI

private let fadeLog = OSLog(subsystem: "LingXia", category: "Appearance")

/// Native bars repaint the moment the scheme changes while the page needs a frame, so the
/// page column holds a picture of itself until the page shows the new scheme, then fades.
extension GpuPageView {
    static let schemeFadeTimeout: TimeInterval = 0.3
    static let schemeFadeDuration: TimeInterval = 0.2

    func beginSchemeFade() {
        guard schemeFade == nil, window != nil, !isHidden,
              let host = schemeHost, host.bounds.width > 0, host.bounds.height > 0,
              let picture = host.snapshotView(afterScreenUpdates: false)
        else { return }
        picture.frame = host.bounds
        picture.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        picture.isUserInteractionEnabled = false
        picture.accessibilityElementsHidden = true
        // A snapshot can leave a Metal layer blank; the page's last frame fills it.
        if let frame = lastFrame() {
            let page = CALayer()
            page.contents = frame
            page.contentsGravity = .resize
            page.frame = convert(bounds, to: host)
            picture.layer.addSublayer(page)
        }
        host.addSubview(picture)
        schemeFade = picture
        schemeFadeStarted = CACurrentMediaTime()
        os_log("scheme fade: began", log: fadeLog, type: .debug)
        gpuPageCover(token)
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.schemeFadeTimeout) { [weak self, weak picture] in
            MainActor.assumeIsolated {
                guard let self, let picture, self.schemeFade === picture else { return }
                self.endSchemeFade(timedOut: true)
            }
        }
    }

    func endSchemeFade(timedOut: Bool = false) {
        guard let picture = schemeFade else { return }
        schemeFade = nil
        let ms = Int((CACurrentMediaTime() - schemeFadeStarted) * 1000)
        os_log("scheme fade: %{public}@ after %d ms", log: fadeLog, type: .debug,
               timedOut ? "timed out" : "page shown", ms)
        if UIAccessibility.isReduceMotionEnabled {
            picture.removeFromSuperview()
            return
        }
        UIView.animate(withDuration: Self.schemeFadeDuration, delay: 0, options: [.curveEaseOut, .allowUserInteraction]) {
            picture.alpha = 0
        } completion: { _ in
            picture.removeFromSuperview()
        }
    }

    /// The page column: the view of the controller showing this page, with its bars.
    private var schemeHost: UIView? {
        var view = superview
        while let current = view {
            if current.next is UIViewController { return current }
            view = current.superview
        }
        return superview
    }
}
#endif
