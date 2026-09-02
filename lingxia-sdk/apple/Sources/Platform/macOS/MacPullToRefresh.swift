#if os(macOS)
import AppKit
import CLingXiaRustAPI
import ObjectiveC
import WebKit

/// Pull-to-refresh for an lxapp page on macOS.
///
/// Attached to the web view rather than to a host, next to the native-component
/// bridge, so every host that mounts an lxapp page gets it from the one call
/// that mounts the page — the shell's view controller and the Runner's
/// simulator alike.
///
/// The shape matches `PullToRefreshHelper` on iOS and Android — a 150pt
/// ceiling, the same rubber band, resting at 64pt, visible for at least 0.8s so
/// an instant refresh still reads as one — and, like Android, the 80pt trigger
/// is measured after the band. That is roughly 70pt of trackpad travel here;
/// iOS reaches its 80 through a scroll view that has already damped the finger,
/// so the same constant asks for a longer drag there.
@MainActor
final class MacPullToRefreshController {
    /// Pull needed to arm a refresh, measured after the rubber band so the
    /// distance means the same thing it does on iOS and Android.
    private static let triggerDistance: CGFloat = 80
    /// Ceiling the rubber band approaches; the pull never exceeds it.
    private static let maxPullDistance: CGFloat = 150
    /// Where the page rests while it refreshes.
    private static let restingDistance: CGFloat = 80 * 0.8
    private static let minVisibleDuration: TimeInterval = 0.8
    /// A classic mouse wheel reports no phases, so there is no release to wait
    /// for: the gesture is over once the notches stop arriving.
    private static let wheelIdleTimeout: TimeInterval = 0.2
    /// Points per line for a wheel that reports coarse deltas.
    private static let wheelLineHeight: CGFloat = 16

    private weak var webView: WKWebView?
    private weak var container: NSView?
    private let indicator = MacRefreshIndicatorView()
    /// The page's own vertical constraints. Offsetting both by the same amount
    /// slides the page without resizing it — a resize would be a full WebKit
    /// relayout and a new viewport for the lxapp — and unlike a layer transform
    /// it keeps AppKit's hit-testing on the pixels the user sees.
    private var pageTop: NSLayoutConstraint?
    private var pageBottom: NSLayoutConstraint?
    /// The strip is exactly as tall as the page has been pulled, so the dots
    /// centre in what the user can actually see rather than in a fixed band
    /// whose middle the page still covers.
    private var indicatorHeight: NSLayoutConstraint?

    /// The document, and every scrollable ancestor of whatever last scrolled,
    /// are at their top. Mirrored from the page: WebKit owns scrolling, so only
    /// the page can answer this.
    private var atTop = true
    /// `enablePullDownRefresh` for the page currently loaded, cached: the check
    /// crosses the FFI and a scroll gesture would ask per event.
    private var enabled = false
    private var pull: CGFloat = 0
    private var isPulling = false
    private var isRefreshing = false
    private var refreshShownAt: Date?
    private var lastWheelAt: Date?

    // MARK: - Attachment

    @discardableResult
    static func attachIfNeeded(to webView: WKWebView, in container: NSView)
        -> MacPullToRefreshController
    {
        if let existing = webView.lxPullToRefreshController {
            existing.rebind(in: container)
            return existing
        }
        let controller = MacPullToRefreshController(webView: webView)
        controller.install(in: container)
        webView.lxPullToRefreshController = controller
        return controller
    }

    /// The controller for a page, addressed the way the runtime addresses one.
    static func controller(appId: String, path: String) -> MacPullToRefreshController? {
        WebViewManager.resolveWebView(appId: appId, path: path)?.lxPullToRefreshController
    }

    private init(webView: WKWebView) {
        self.webView = webView
    }

    private func install(in container: NSView) {
        self.container = container
        indicator.translatesAutoresizingMaskIntoConstraints = false
        indicator.isHidden = true
        indicator.alphaValue = 0
        // Behind the page, revealed by sliding the page down — the model
        // Android uses.
        container.addSubview(indicator, positioned: .below, relativeTo: nil)
        let height = indicator.heightAnchor.constraint(equalToConstant: 0)
        indicatorHeight = height
        NSLayoutConstraint.activate([
            indicator.topAnchor.constraint(equalTo: container.topAnchor),
            indicator.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            indicator.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            height
        ])
        MacNativeBridge.setOverlayOffset(0, in: container)
        refreshEnabledFlag()
    }

    private func rebind(in container: NSView) {
        // A revisited page comes back at rest, whatever it was doing when it
        // was put away.
        isRefreshing = false
        refreshShownAt = nil
        indicator.stopLoading()
        indicator.alphaValue = 0
        indicator.isHidden = true
        pull = 0
        isPulling = false
        // The previous attachment's constraints went with it.
        pageTop = nil
        pageBottom = nil
        guard indicator.superview !== container else {
            indicatorHeight?.constant = 0
            MacNativeBridge.setOverlayOffset(0, in: container)
            refreshEnabledFlag()
            return
        }
        indicator.removeFromSuperview()
        install(in: container)
    }

    /// The web view's constraints, owned here so the pull can move it.
    /// `attachWebViewToContainer` installs whatever this returns instead of its
    /// own full-container pinning.
    func pageConstraints(in container: NSView) -> [NSLayoutConstraint] {
        guard let webView else { return [] }
        let top = webView.topAnchor.constraint(equalTo: container.topAnchor)
        let bottom = webView.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        pageTop = top
        pageBottom = bottom
        return [
            webView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            top,
            bottom
        ]
    }

    /// Re-read the page's declaration. Called when the page behind the web view
    /// changes, so the gesture follows navigation without asking per event.
    func refreshEnabledFlag() {
        guard let webView,
              let appId = webView.appId, !appId.isEmpty,
              let path = webView.currentPath
        else {
            enabled = false
            return
        }
        enabled = isPullDownRefreshEnabled(appId, Self.normalizedPath(path))
        // `setup()` runs on every path change and from every web-view lookup,
        // so an idle page must cost nothing here.
        guard !enabled, isRefreshing || pull != 0 else { return }
        // A page that cannot be pulled must not inherit the previous page's
        // refresh holding the offset open.
        isRefreshing = false
        refreshShownAt = nil
        reset(animated: false)
    }

    /// Whether this page is still the one on screen in its container. The
    /// container and its component overlay are shared with every other page
    /// that has been shown there, so a controller left behind by a page switch
    /// must not keep writing to them.
    private var isMounted: Bool {
        webView?.superview === container && pageTop?.isActive == true
    }

    /// Mirrored from the page by the native bridge's scroll tracker.
    func setAtTop(_ value: Bool) {
        atTop = value
    }

    // MARK: - Gesture

    /// Returns true when the event belongs to a pull and must not reach WebKit.
    ///
    /// WebKit does not bounce a short page on macOS and exposes no equivalent
    /// of iOS's `alwaysBounceVertical`, so the pull is driven from the wheel
    /// deltas rather than read off an overscroll that may never happen.
    func handleScrollWheel(_ event: NSEvent) -> Bool {
        guard enabled, isMounted, !isRefreshing else { return false }

        // A trackpad names the phases of its gesture; a wheel names none.
        guard event.phase.isEmpty, event.momentumPhase.isEmpty else {
            return handleGesture(event)
        }
        return handleWheelNotch(delta: Self.points(from: event))
    }

    private static func points(from event: NSEvent) -> CGFloat {
        event.hasPreciseScrollingDeltas
            ? event.scrollingDeltaY
            : event.scrollingDeltaY * wheelLineHeight
    }

    private func handleGesture(_ event: NSEvent) -> Bool {
        // Momentum is what the page coasts on after the fingers lift; counting
        // it would arm a refresh from a flick the user never pulled through.
        guard event.momentumPhase.isEmpty else { return isPulling }

        // The boundary events are always forwarded: WebKit then sees a whole
        // gesture, with only the movement removed, instead of one that began
        // and never ended.
        if event.phase.contains(.began) {
            pull = 0
            isPulling = false
            return false
        }
        if event.phase.contains(.cancelled) {
            // The system took the gesture away; that is not a release.
            if isPulling {
                isPulling = false
                reset(animated: true)
            }
            return false
        }
        if event.phase.contains(.ended) {
            if isPulling { release() }
            return false
        }

        let delta = Self.points(from: event)
        if !isPulling {
            guard atTop, delta > 0 else { return false }
            isPulling = true
            pull = 0
        }
        return accumulate(delta)
    }

    /// A wheel has no release, so the gesture ends when the notches stop.
    private func handleWheelNotch(delta: CGFloat) -> Bool {
        let now = Date()
        if !isPulling {
            // Only a fresh burst can pull. The notches that carried the page to
            // the top belong to that scroll, and so does the overshoot they run
            // into once it gets there — a wheel almost always overshoots, and
            // treating that as a pull would bounce the page on every arrival.
            let continuing = lastWheelAt.map { now.timeIntervalSince($0) < Self.wheelIdleTimeout }
                ?? false
            lastWheelAt = now
            guard !continuing, atTop, delta > 0 else { return false }
            isPulling = true
            pull = 0
        }
        lastWheelAt = now
        let consumed = accumulate(delta)
        guard isPulling else { return consumed }
        // A wheel has no release to wait for, and its notches can arrive slower
        // than the idle timeout: waiting for the gesture to "end" would unwind
        // every pull before it ever reached the trigger. Crossing the trigger
        // is the commitment; the timer only unwinds one that stops short.
        if Self.rubberBand(pull) >= Self.triggerDistance {
            isPulling = false
            startRefreshing()
            return true
        }
        scheduleWheelRelease(after: now)
        return consumed
    }

    private func accumulate(_ delta: CGFloat) -> Bool {
        pull += delta
        guard pull > 0 else {
            // Pulled back past the top: hand the scroll back to the page.
            isPulling = false
            reset(animated: false)
            return false
        }
        applyPull()
        return true
    }

    /// Unwind a wheel pull that stopped short of the trigger.
    private func scheduleWheelRelease(after stamp: Date) {
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.wheelIdleTimeout) { [weak self] in
            guard let self, self.isPulling, self.lastWheelAt == stamp else { return }
            self.isPulling = false
            self.reset(animated: true)
        }
    }

    private func release() {
        isPulling = false
        if Self.rubberBand(pull) >= Self.triggerDistance {
            startRefreshing()
        } else {
            reset(animated: true)
        }
    }

    // MARK: - Presentation

    private func applyPull() {
        let travel = Self.rubberBand(pull)
        indicator.isHidden = false
        indicator.alphaValue = min(1, (travel / Self.triggerDistance) * 1.5)
        paintIndicator()
        setPageOffset(travel, animated: false)
    }

    /// The same curve iOS and Android use, so the pull decelerates identically
    /// on all three.
    private static func rubberBand(_ distance: CGFloat) -> CGFloat {
        guard distance > 0 else { return 0 }
        let coefficient: CGFloat = 0.55
        let x = distance / maxPullDistance
        let numerator = 1 - exp(-coefficient * x)
        let denominator = 1 - exp(-coefficient)
        return maxPullDistance * (numerator / denominator)
    }

    /// Slide the page down to reveal the indicator behind it. Both edges move,
    /// so the web view keeps its size and the lxapp keeps its viewport.
    private func setPageOffset(_ offset: CGFloat, animated: Bool) {
        guard isMounted, let pageTop, let pageBottom, let container else { return }
        MacNativeBridge.setOverlayOffset(offset, in: container)
        guard animated else {
            pageTop.constant = offset
            pageBottom.constant = offset
            indicatorHeight?.constant = offset
            container.layoutSubtreeIfNeeded()
            return
        }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.25
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            context.allowsImplicitAnimation = true
            pageTop.animator().constant = offset
            pageBottom.animator().constant = offset
            indicatorHeight?.animator().constant = offset
            container.layoutSubtreeIfNeeded()
        }
    }

    /// Keep the revealed strip on the page's own colour, and the dots readable
    /// against it. `underPageBackgroundColor` is WebKit's overscroll colour and
    /// tracks the document background.
    private func paintIndicator() {
        let background = webView?.underPageBackgroundColor ?? .windowBackgroundColor
        indicator.wantsLayer = true
        indicator.layer?.backgroundColor = background.cgColor
        indicator.setDotColor(Self.contrastingDotColor(for: background))
    }

    private static func contrastingDotColor(for background: NSColor) -> NSColor {
        guard let rgb = background.usingColorSpace(.sRGB) ?? background.usingColorSpace(.deviceRGB)
        else { return NSColor.black.withAlphaComponent(0.55) }
        let luminance = 0.299 * rgb.redComponent + 0.587 * rgb.greenComponent
            + 0.114 * rgb.blueComponent
        return luminance > 0.5
            ? NSColor.black.withAlphaComponent(0.55)
            : NSColor.white.withAlphaComponent(0.85)
    }

    // MARK: - Refresh state

    /// Also the entry point for `lx.startPullDownRefresh()`, so a programmatic
    /// refresh and a pulled one look the same.
    func startRefreshing() {
        guard enabled, isMounted, !isRefreshing else { return }
        isRefreshing = true
        isPulling = false
        pull = Self.restingDistance
        refreshShownAt = Date()
        indicator.isHidden = false
        indicator.alphaValue = 1
        paintIndicator()
        indicator.startLoading()
        setPageOffset(Self.restingDistance, animated: true)

        guard let webView, let appId = webView.appId else { return }
        _ = onLxappEvent(appId, LxAppEvent.pullDownRefresh, webView.currentPath ?? "")
    }

    func endRefreshing() {
        guard isRefreshing else { return }
        let elapsed = refreshShownAt.map { Date().timeIntervalSince($0) } ?? Self.minVisibleDuration
        let remaining = Self.minVisibleDuration - elapsed
        guard remaining <= 0 else {
            DispatchQueue.main.asyncAfter(deadline: .now() + remaining) { [weak self] in
                self?.finishRefreshing()
            }
            return
        }
        finishRefreshing()
    }

    private func finishRefreshing() {
        guard isRefreshing else { return }
        isRefreshing = false
        refreshShownAt = nil
        reset(animated: true)
    }

    private func reset(animated: Bool) {
        pull = 0
        isPulling = false
        setPageOffset(0, animated: animated)
        indicator.stopLoading()
        guard animated else {
            indicator.alphaValue = 0
            indicator.isHidden = true
            return
        }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.18
            indicator.animator().alphaValue = 0
        } completionHandler: { [weak self] in
            MainActor.assumeIsolated {
                guard let self, !self.isRefreshing else { return }
                self.indicator.isHidden = true
            }
        }
    }

    private static func normalizedPath(_ path: String) -> String {
        if let cut = path.firstIndex(where: { $0 == "?" || $0 == "#" }) {
            return String(path[..<cut])
        }
        return path
    }

    deinit {
        // The indicator lives in the container, not in the web view, so it
        // outlives this controller unless it is taken down here. A deinit runs
        // wherever the last reference was dropped, which for a web view is not
        // necessarily the main thread.
        let view = indicator
        let cleanup = { @Sendable in
            MainActor.assumeIsolated {
                view.stopLoading()
                view.removeFromSuperview()
            }
        }
        if Thread.isMainThread {
            cleanup()
        } else {
            DispatchQueue.main.async(execute: cleanup)
        }
    }
}

extension WKWebView {
    private static var pullToRefreshKey: UInt8 = 0

    var lxPullToRefreshController: MacPullToRefreshController? {
        get { objc_getAssociatedObject(self, &Self.pullToRefreshKey) as? MacPullToRefreshController }
        set {
            objc_setAssociatedObject(
                self, &Self.pullToRefreshKey, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        }
    }
}
#endif
