import Foundation
#if os(macOS)
import AppKit
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
        guard schemeFade == nil, window != nil, !isHiddenOrHasHiddenAncestor,
              let host = schemeHost, host.bounds.width > 0, host.bounds.height > 0,
              let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds)
        else { return }
        host.cacheDisplay(in: host.bounds, to: rep)
        let overlay = SchemeFadeView(frame: host.bounds)
        overlay.autoresizingMask = [.width, .height]
        overlay.wantsLayer = true
        overlay.layer?.contents = rep.cgImage
        overlay.layer?.contentsGravity = .resize
        // AppKit's cache leaves a Metal layer blank; the page's last frame fills it.
        if let frame = lastFrame() {
            let page = CALayer()
            page.contents = frame
            page.contentsGravity = .resize
            page.frame = convert(bounds, to: overlay)
            overlay.layer?.addSublayer(page)
        }
        host.addSubview(overlay, positioned: .above, relativeTo: nil)
        schemeFade = overlay
        schemeFadeStarted = CACurrentMediaTime()
        os_log("scheme fade: began", log: fadeLog, type: .debug)
        gpuPageCover(token)
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.schemeFadeTimeout) { [weak self, weak overlay] in
            MainActor.assumeIsolated {
                guard let self, let overlay, self.schemeFade === overlay else { return }
                self.endSchemeFade(timedOut: true)
            }
        }
    }

    func endSchemeFade(timedOut: Bool = false) {
        guard let overlay = schemeFade else { return }
        schemeFade = nil
        let ms = Int((CACurrentMediaTime() - schemeFadeStarted) * 1000)
        os_log("scheme fade: %{public}@ after %d ms", log: fadeLog, type: .debug,
               timedOut ? "timed out" : "page shown", ms)
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            overlay.removeFromSuperview()
            return
        }
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = Self.schemeFadeDuration
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            overlay.animator().alphaValue = 0
        }, completionHandler: {
            MainActor.assumeIsolated { overlay.removeFromSuperview() }
        })
    }

    /// The page column: the view of the controller showing this page, with its bars.
    private var schemeHost: NSView? {
        var view = superview
        while let current = view {
            if current.nextResponder is NSViewController { return current }
            view = current.superview
        }
        return superview
    }
}

/// A picture over the page column that never takes the pointer.
private final class SchemeFadeView: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}
#endif
