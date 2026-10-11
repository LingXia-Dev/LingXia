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
/// A trackpad or wheel pull is measured like `PullToRefreshHelper` on iOS and
/// Android — a 150pt ceiling and the same rubber band, with the 80pt trigger
/// measured after the band, roughly 70pt of trackpad travel here — but the
/// page itself never moves: a desktop page stays put under the pointer.
///
/// Most desktop refreshes have no pull behind them (the context menu, ⌘R,
/// `lx.startPullDownRefresh()`), so a running refresh always shows the same
/// way: the page stays where it is and a full-width bar flows along its top
/// edge, for at least 0.6s before it completes so an instant refresh still
/// reads as one. A pull fills that same bar as it nears the trigger and,
/// released past it, sets the bar running in place.
@MainActor
final class MacPullToRefreshController {
    /// Pull needed to arm a refresh, measured after the rubber band so the
    /// distance means the same thing it does on iOS and Android.
    private static let triggerDistance: CGFloat = 80
    /// Ceiling the rubber band approaches; the pull never exceeds it.
    private static let maxPullDistance: CGFloat = 150
    private static let minVisibleDuration: TimeInterval = 0.6
    /// A classic mouse wheel reports no phases, so there is no release to wait
    /// for: the gesture is over once the notches stop arriving.
    private static let wheelIdleTimeout: TimeInterval = 0.2
    /// Points per line for a wheel that reports coarse deltas.
    private static let wheelLineHeight: CGFloat = 16

    private weak var webView: WKWebView?
    private weak var container: NSView?
    /// Fills as the page is pulled, and runs along its top while it refreshes.
    private let bar = MacRefreshBarView()

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
    // Set while the bar keeps running after a stop to honour minVisibleDuration.
    private var pendingFinish: DispatchWorkItem?
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

    /// Controllers that have refreshed, by page webtag: a stop has to reach
    /// the bar even after the runtime has dropped the page it names.
    private static let refreshed =
        NSMapTable<NSString, MacPullToRefreshController>.strongToWeakObjects()

    /// The controller for a page, addressed the way the runtime addresses one.
    static func controller(appId: String, webtag: String) -> MacPullToRefreshController? {
        if let known = refreshed.object(forKey: webtag as NSString) { return known }
        guard let page = WebViewManager.page(appId: appId, webtag: webtag) else { return nil }
        return WebViewManager.pageWebView(appId: appId, pageInstanceId: page.pageInstanceId)?
            .lxPullToRefreshController
    }

    private init(webView: WKWebView) {
        self.webView = webView
    }

    private func install(in container: NSView) {
        self.container = container
        refreshEnabledFlag()
    }

    private func rebind(in container: NSView) {
        // A revisited page comes back at rest, whatever it was doing when it
        // was put away.
        isRefreshing = false
        refreshShownAt = nil
        cancelPendingFinish()
        bar.detach()
        pull = 0
        isPulling = false
        install(in: container)
    }

    /// The web view is leaving this container. Take the bar with it so a
    /// later page does not inherit it.
    func unmount() {
        isRefreshing = false
        refreshShownAt = nil
        cancelPendingFinish()
        bar.detach()
        pull = 0
        isPulling = false
        container = nil
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
        // A page that cannot refresh must not inherit the previous page's
        // refresh or a pull holding the offset open.
        isRefreshing = false
        refreshShownAt = nil
        cancelPendingFinish()
        bar.hide()
        cancelPull()
    }

    /// Whether this page is still the one on screen in its container. The
    /// container and its component overlay are shared with every other page
    /// that has been shown there, so a controller left behind by a page switch
    /// must not keep writing to them.
    private var isMounted: Bool {
        container != nil && webView?.superview === container
    }

    /// The page declared `enablePullDownRefresh` and is on screen, so a
    /// refresh command (⌘R) applies to it.
    var canRefresh: Bool {
        enabled && isMounted
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
                cancelPull()
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
            cancelPull()
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
            self.cancelPull()
        }
    }

    private func release() {
        isPulling = false
        if Self.rubberBand(pull) >= Self.triggerDistance {
            startRefreshing()
        } else {
            cancelPull()
        }
    }

    // MARK: - Presentation

    private func applyPull() {
        guard let container else { return }
        let dark = WebViewManager.resolvedDarkAppearance(appId: webView?.appId)
        bar.showPull(
            Self.rubberBand(pull) / Self.triggerDistance, in: container,
            color: WebViewManager.declaredRefreshIndicatorColor(dark: dark))
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

    // MARK: - Refresh state

    /// Also the entry point for `lx.startPullDownRefresh()`, the context menu
    /// and ⌘R, so every refresh looks the same: the page at rest and the bar
    /// running along its top. A start while the bar finishes the previous
    /// refresh takes it straight back to running.
    func startRefreshing() {
        guard enabled, isMounted else { return }
        if pendingFinish != nil {
            // Restarted before the bar began to complete: it is still running.
            cancelPendingFinish()
            reportRefresh()
            return
        }
        guard !isRefreshing else { return }
        isRefreshing = true
        isPulling = false
        refreshShownAt = Date()
        pull = 0
        if let container {
            let dark = WebViewManager.resolvedDarkAppearance(appId: webView?.appId)
            bar.start(
                in: container, color: WebViewManager.declaredRefreshIndicatorColor(dark: dark))
        }
        announce("lx_pull_refresh_refreshing")
        reportRefresh()
    }

    /// Fire `onPullDownRefresh` for this page, or end a refresh it cannot get.
    private func reportRefresh() {
        guard let webView, let appId = webView.appId, let webtag = webView.pageWebTag else {
            finishRefreshing()
            return
        }
        Self.refreshed.setObject(self, forKey: webtag as NSString)
        _ = onLxappEvent(appId, LxAppEvent.pullDownRefresh, webtag)
    }

    func endRefreshing() {
        guard isRefreshing, pendingFinish == nil else { return }
        let elapsed = refreshShownAt.map { Date().timeIntervalSince($0) } ?? Self.minVisibleDuration
        let remaining = Self.minVisibleDuration - elapsed
        guard remaining <= 0 else {
            let work = DispatchWorkItem { [weak self] in
                MainActor.assumeIsolated {
                    self?.pendingFinish = nil
                    self?.finishRefreshing()
                }
            }
            pendingFinish = work
            DispatchQueue.main.asyncAfter(deadline: .now() + remaining, execute: work)
            return
        }
        finishRefreshing()
    }

    private func cancelPendingFinish() {
        pendingFinish?.cancel()
        pendingFinish = nil
    }

    private func finishRefreshing() {
        guard isRefreshing else { return }
        isRefreshing = false
        refreshShownAt = nil
        bar.complete()
        announce("lx_pull_refresh_refreshed")
    }

    /// Tell VoiceOver; the bar itself is silent. Never called for a refresh
    /// dropped by leaving the page.
    private func announce(_ key: String) {
        let element: Any
        if let window = webView?.window {
            element = window
        } else {
            element = NSApplication.shared
        }
        NSAccessibility.post(
            element: element,
            notification: .announcementRequested,
            userInfo: [
                .announcement: L10n.string(key),
                .priority: NSAccessibilityPriorityLevel.medium.rawValue,
            ])
    }

    /// Drop a pull that will not refresh.
    private func cancelPull() {
        pull = 0
        isPulling = false
        bar.cancelPull()
    }

    private static func normalizedPath(_ path: String) -> String {
        if let cut = path.firstIndex(where: { $0 == "?" || $0 == "#" }) {
            return String(path[..<cut])
        }
        return path
    }

    deinit {
        // The bar lives in the container, not in the web view, so it outlives
        // this controller unless it is taken down here. A deinit runs wherever
        // the last reference was dropped, which for a web view is not
        // necessarily the main thread.
        let bar = bar
        let cleanup = { @Sendable in
            MainActor.assumeIsolated { bar.detach() }
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
