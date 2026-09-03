#if os(macOS)
import AppKit
import QuartzCore
import WebKit

/// The page-swap animation, owned in one place for every macOS host.
///
/// The desktop shell and the runner's phone shape draw different chrome, but
/// the transition between two pages is one behaviour. It was written twice, and
/// the two copies drifted: the same doubled reveal and the same mid-slide
/// jitter had to be found and fixed in each. Whatever changes here changes for
/// both.
///
/// The host keeps the view surgery — the hierarchies genuinely differ — and
/// asks this for the three things that were duplicated: when to slide, what the
/// slide looks like, and whether one is still running.
@MainActor
public final class LxAppPageTransition {
    /// Matches the iOS and Android page transition.
    public static let duration: CFTimeInterval = 0.3

    /// How long a navigation holds the outgoing page waiting for the incoming
    /// one to paint. Past this a slow page has to start moving anyway: a tap
    /// that does nothing for longer than this reads as a dropped input.
    public static let firstPaintGrace: TimeInterval = 0.2

    private var endsAt: CFTimeInterval = 0
    private var firstPaintObservation: NSKeyValueObservation?

    public init() {}

    deinit {
        firstPaintObservation?.invalidate()
    }

    /// Whether a slide installed here is still on screen.
    ///
    /// Anything that would animate — revealing a load placeholder, say — asks
    /// first, because two animations over the same moment read as two
    /// navigations.
    public var isRunning: Bool {
        CACurrentMediaTime() < endsAt
    }

    /// Slide the container's contents in the navigation's direction.
    ///
    /// A layer `CATransition` animates the swap of the outgoing webview subview
    /// for the incoming one: no per-webview transform or constraint juggling,
    /// and it survives Auto Layout re-pinning.
    public func install(_ animation: LxAppAnimation, on container: NSView) {
        guard let layer = container.layer else { return }
        let transition = CATransition()
        transition.duration = Self.duration
        transition.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        switch animation {
        case .push:
            transition.type = .push
            transition.subtype = .fromRight
        case .pop:
            transition.type = .push
            transition.subtype = .fromLeft
        case .fade:
            transition.type = .fade
        case .none:
            return
        }
        layer.add(transition, forKey: "lxNavTransition")
        endsAt = CACurrentMediaTime() + Self.duration
    }

    /// Whether this navigation should wait before it slides.
    ///
    /// A page that has not painted shows its content settling — a framework
    /// mounting, images and fonts landing — inside a frame that is already
    /// moving.
    public static func needsPaintWait(_ webView: WKWebView, animation: LxAppAnimation) -> Bool {
        animation != .none && (webView.isLoading || webView.url == nil)
    }

    /// Perform `swap` once the incoming page can draw itself, or once the grace
    /// period expires.
    ///
    /// Nothing changes on screen while waiting — the outgoing page stays — so
    /// this reads as the animation starting a moment later rather than as a
    /// stall. `stillCurrent` is consulted at commit time so a navigation that
    /// has since been superseded does not swap in a stale page.
    public func whenPagePaints(
        _ webView: WKWebView,
        stillCurrent: @escaping () -> Bool,
        swap: @escaping () -> Void
    ) {
        firstPaintObservation?.invalidate()
        var committed = false
        let commit: () -> Void = { [weak self] in
            guard !committed, stillCurrent() else { return }
            committed = true
            self?.firstPaintObservation?.invalidate()
            self?.firstPaintObservation = nil
            swap()
        }
        firstPaintObservation = webView.observe(\.isLoading, options: [.new]) { _, change in
            // KVO reports changes only, so the not-yet-started `false` never
            // arrives here; this is the load finishing.
            guard change.newValue == false else { return }
            Task { @MainActor in commit() }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.firstPaintGrace) {
            commit()
        }
    }

    /// Drop a pending wait, for a host tearing down or superseding it outright.
    public func cancelPendingWait() {
        firstPaintObservation?.invalidate()
        firstPaintObservation = nil
    }
}
#endif
