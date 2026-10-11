#if os(iOS)
import UIKit
import WebKit

/// Mounts only into an unambiguous, root-owned scrolling layer. WebKit's
/// subview structure is not API: a missing/replaced layer must be recoverable.
@MainActor
final class NativeRootComposition {
    let container: UIView
    private weak var overlay: UIView?
    private weak var webView: WKWebView?
    private weak var anchor: UIScrollView?
    private var savedPanEnabled: Bool?
    private var descriptor: [String: Any]?
    private var retry: Task<Void, Never>?
    private var active = true
    private var tornDown = false
    private(set) var origin = CGPoint.zero
    private(set) var sameLayer = false
    var changed: (() -> Void)?

    init(container: UIView, overlay: UIView, webView: WKWebView?) {
        self.container = container
        self.overlay = overlay
        self.webView = webView
        overlay.addSubview(container)
        (container as? NativeRootContainerView)?.windowChanged = { [weak self] in
            // WebKit may discard a scrolling layer without a DOM mutation.
            Task { @MainActor [weak self] in
                guard let self, !self.tornDown, self.active, self.sameLayer,
                      let webView = self.webView, webView.window != nil,
                      self.anchor?.isDescendant(of: webView) != true else { return }
                self.update(self.descriptor)
            }
        }
    }

    func update(_ descriptor: [String: Any]?) {
        guard !tornDown else { return }
        retry?.cancel()
        retry = nil
        self.descriptor = descriptor
        resolve()
        if descriptor != nil, !sameLayer, active { scheduleRetry() }
    }

    func setActive(_ active: Bool) {
        self.active = active
        container.isHidden = !active
        if active { update(descriptor) }
        else { retry?.cancel(); retry = nil }
    }

    func teardown() {
        tornDown = true
        (container as? NativeRootContainerView)?.windowChanged = nil
        retry?.cancel()
        retry = nil
        changed = nil
        restoreAnchor()
        NativeComponentHitRouter.shared.unregisterNativeView(container)
        container.removeFromSuperview()
    }

    private func restoreAnchor() {
        if let anchor, let savedPanEnabled {
            anchor.panGestureRecognizer.isEnabled = savedPanEnabled
        }
        anchor = nil
        savedPanEnabled = nil
    }

    private func scheduleRetry() {
        guard retry == nil else { return }
        // WebKit commits its layer tree after the JS geometry message. Bound
        // retries instead of running a display link on every idle page.
        retry = Task { @MainActor [weak self] in
            for _ in 0..<12 {
                do { try await Task.sleep(nanoseconds: 16_000_000) }
                catch { return }
                guard let self, self.active, !Task.isCancelled else { return }
                self.resolve()
                if self.sameLayer { break }
            }
            self?.retry = nil
        }
    }

    private func resolve() {
        guard let overlay else { return }
        let previousMode = sameLayer
        var target: UIScrollView?
        if active, let webView, let descriptor,
           let viewport = Self.rect(descriptor["viewportRect"]),
           let content = Self.rect(descriptor["contentRect"]),
           let extent = descriptor["scrollExtent"] as? [String: Any],
           let x = extent["x"] as? NSNumber, let y = extent["y"] as? NSNumber,
           x.doubleValue.isFinite, y.doubleValue.isFinite,
           (16...268).contains(x.doubleValue), (16...268).contains(y.doubleValue),
           abs(webView.scrollView.zoomScale - 1) < 0.001 {
            let signature = CGSize(width: x.doubleValue, height: y.doubleValue)
            if let anchor, anchor.isDescendant(of: webView), Self.matches(anchor, extent: signature, size: viewport.size) {
                target = anchor
            } else {
                var matches: [UIScrollView] = []
                func visit(_ view: UIView) {
                    // Never search another native component's subtree.
                    if view === container || view === overlay { return }
                    if let scroll = view as? UIScrollView,
                       NSStringFromClass(type(of: scroll)).contains("WKChildScrollView"),
                       Self.matches(scroll, extent: signature, size: viewport.size) {
                        let frame = webView.convert(scroll.bounds, from: scroll)
                        if abs(frame.minX - viewport.minX) <= 2 && abs(frame.minY - viewport.minY) <= 2 {
                            matches.append(scroll)
                        }
                    }
                    view.subviews.forEach(visit)
                }
                visit(webView.scrollView)
                // Equal-sized or coincident roots must never share a mount.
                if matches.count == 1 { target = matches[0] }
            }
            if target != nil { origin = content.origin }
        }
        if let target {
            if anchor !== target {
                restoreAnchor()
                anchor = target
                savedPanEnabled = target.panGestureRecognizer.isEnabled
                target.panGestureRecognizer.isEnabled = false
            }
            if container.superview !== target { target.addSubview(container) }
            container.frame = CGRect(origin: .zero, size: target.bounds.size)
            sameLayer = true
            NativeComponentHitRouter.shared.registerNativeView(container, in: target, requiresWebKitHit: true)
        } else {
            restoreAnchor()
            if container.superview !== overlay { overlay.addSubview(container) }
            origin = .zero
            sameLayer = false
            if let webView {
                NativeComponentHitRouter.shared.registerNativeView(container, in: webView.scrollView)
            }
        }
        // Geometry must be recomputed even if only the coordinate basis changed.
        changed?()
        if previousMode != sameLayer, sameLayer { retry?.cancel(); retry = nil }
    }

    static func matches(_ scroll: UIScrollView, extent: CGSize, size: CGSize) -> Bool {
        abs(scroll.bounds.width - size.width) <= 1 && abs(scroll.bounds.height - size.height) <= 1 &&
            abs(scroll.contentSize.width - scroll.bounds.width - extent.width) <= 1 &&
            abs(scroll.contentSize.height - scroll.bounds.height - extent.height) <= 1 &&
            abs(scroll.contentOffset.x) <= 0.5 && abs(scroll.contentOffset.y) <= 0.5
    }

    static func rect(_ raw: Any?) -> CGRect? {
        guard let raw = raw as? [String: Any],
              let x = raw["x"] as? NSNumber, let y = raw["y"] as? NSNumber,
              let width = raw["width"] as? NSNumber, let height = raw["height"] as? NSNumber,
              [x, y, width, height].allSatisfy({ $0.doubleValue.isFinite }),
              width.doubleValue > 0, height.doubleValue > 0
        else { return nil }
        return CGRect(x: x.doubleValue, y: y.doubleValue, width: width.doubleValue, height: height.doubleValue)
    }
}

@MainActor
class NativeRootContainerView: UIView {
    var windowChanged: (() -> Void)?

    override func didMoveToWindow() {
        super.didMoveToWindow()
        windowChanged?()
    }
}
#endif
