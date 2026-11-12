import Foundation
#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// Pull-to-refresh for a GPU page: the page slides down under the top chrome over a strip
/// with the system spinner, as `PullToRefreshHelper` does for a WebView page.
@MainActor
final class GpuPullToRefresh {
    private static let triggerDistance: CGFloat = 80
    private static let maxPullDistance: CGFloat = 150
    private static let minVisibleDuration: CFTimeInterval = 0.4
    private static let slop: CGFloat = 10

    private weak var page: GpuPageView?
    private let onRefresh: () -> Void
    private let strip = UIView()
    private let spinner = RefreshIndicatorView()

    private var start: CGPoint?
    private var isPulling = false
    private var isRefreshing = false
    private var offset: CGFloat = 0
    private var top: CGFloat = 0
    private var shownAt: CFTimeInterval = 0
    private var pendingDismiss: DispatchWorkItem?

    init(page: GpuPageView, onRefresh: @escaping () -> Void) {
        self.page = page
        self.onRefresh = onRefresh
        strip.isUserInteractionEnabled = false
        strip.clipsToBounds = true
        strip.isHidden = true
        strip.addSubview(spinner)
    }

    /// Read-only state for test automation: idle, pulling, armed, refreshing or ending.
    var phase: String {
        if isRefreshing { return "refreshing" }
        if isPulling { return offset >= Self.triggerDistance ? "armed" : "pulling" }
        return pendingDismiss != nil || offset > 1 ? "ending" : "idle"
    }

    /// How far the page is moved down, in points.
    var indicatorOffset: CGFloat { offset }

    /// The page moved down, so it must not re-measure against where it now sits.
    var holdsPage: Bool { offset > 0 }

    private var busy: Bool { isRefreshing || pendingDismiss != nil }

    // MARK: Touch

    /// A finger went down at `point`, in a frame that does not move with the page.
    func began(_ point: CGPoint) {
        start = busy ? nil : point
        isPulling = false
    }

    /// True while the finger pulls the page; the first true is the core's cue to let go.
    func moved(_ point: CGPoint, atTop: () -> Bool) -> Bool {
        guard let start, !busy else { return false }
        let dx = point.x - start.x
        let dy = point.y - start.y
        if !isPulling {
            guard dy > Self.slop, dy > abs(dx) else {
                if dy <= 0 { self.start = point }
                return false
            }
            // Still scrolling back: a pull starts only from where the page reached its top.
            guard atTop() else {
                self.start = point
                return false
            }
            isPulling = true
            top = chromeTop()
        }
        let raw = max(0, dy - Self.slop)
        setOffset(Self.rubberBand(raw))
        return true
    }

    /// The finger lifted (`up`) or was cancelled; true when it ended a pull.
    func ended(up: Bool) -> Bool {
        start = nil
        guard isPulling else { return false }
        isPulling = false
        if up, offset >= Self.triggerDistance {
            startRefreshing()
        } else {
            settle(to: 0)
        }
        return true
    }

    // MARK: Refresh

    func startRefreshing() {
        guard !isRefreshing else { return }
        if let pendingDismiss {
            // Restarted while lingering: the spinner is still up, keep it.
            pendingDismiss.cancel()
            self.pendingDismiss = nil
            isRefreshing = true
            announce("lx_pull_refresh_refreshing")
            onRefresh()
            return
        }
        isRefreshing = true
        shownAt = CACurrentMediaTime()
        if offset == 0 { top = chromeTop() }
        spinner.startLoading(reduceMotion: UIAccessibility.isReduceMotionEnabled)
        announce("lx_pull_refresh_refreshing")
        settle(to: Self.triggerDistance * 0.8)
        onRefresh()
    }

    /// Ends the refresh; the spinner lingers until it has been up for the minimum time.
    func endRefreshing() {
        guard isRefreshing else { return }
        isRefreshing = false
        let remaining = Self.minVisibleDuration - (CACurrentMediaTime() - shownAt)
        guard remaining > 0 else { return dismiss() }
        let work = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                self?.pendingDismiss = nil
                self?.dismiss()
            }
        }
        pendingDismiss = work
        DispatchQueue.main.asyncAfter(deadline: .now() + remaining, execute: work)
    }

    /// The page is leaving or no longer refreshes: put everything back at once.
    func detach() {
        pendingDismiss?.cancel()
        pendingDismiss = nil
        isRefreshing = false
        isPulling = false
        start = nil
        spinner.stopLoading()
        setOffset(0)
        strip.removeFromSuperview()
    }

    private func dismiss() {
        spinner.stopLoading()
        announce("lx_pull_refresh_refreshed")
        settle(to: 0)
    }

    // MARK: Presentation

    private func settle(to target: CGFloat) {
        UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseOut, .beginFromCurrentState]) {
            self.setOffset(target, finished: false)
        } completion: { _ in
            // A later pull or refresh may have moved it again meanwhile.
            if self.offset == target { self.setOffset(target) }
        }
    }

    /// `finished` false keeps the strip up while an animation is still closing it.
    private func setOffset(_ value: CGFloat, finished: Bool = true) {
        guard let page else { return }
        offset = value
        if value > 0, strip.superview !== page.superview, let parent = page.superview {
            // Directly above the page and below the bars, so the page slides out from under them.
            parent.insertSubview(strip, aboveSubview: page)
        }
        strip.backgroundColor = page.hostChrome?.canvas
        strip.frame = CGRect(x: page.frame.minX, y: top, width: page.bounds.width, height: value)
        if value > 0 { strip.isHidden = false } else if finished { strip.isHidden = true }
        spinner.frame = CGRect(x: 0, y: 0, width: strip.bounds.width, height: Self.triggerDistance)
        spinner.applyTint(spinnerColor())
        if !busy {
            spinner.setProgress(min(1, value / Self.triggerDistance))
            spinner.alpha = min(1, value / Self.triggerDistance * 1.5)
        } else {
            spinner.alpha = 1
        }
        page.transform = value > 0 ? CGAffineTransform(translationX: 0, y: value) : .identity
        if value <= 0, finished { page.pullReleased() }
    }

    /// Where the page's content starts: below the bars over it, else below the status bar.
    private func chromeTop() -> CGFloat {
        guard let page else { return 0 }
        return max(page.chromeTop, page.safeAreaInsets.top)
    }

    private func spinnerColor() -> UIColor? {
        let appId = page.map { gpuPageInfo($0.token).app_id.toString() }
        let dark = WebViewManager.resolvedDarkAppearance(appId: appId)
        return WebViewManager.declaredRefreshIndicatorColor(dark: dark)
    }

    private func announce(_ key: String) {
        UIAccessibility.post(notification: .announcement, argument: L10n.string(key))
    }

    private static func rubberBand(_ distance: CGFloat) -> CGFloat {
        let coefficient: CGFloat = 0.55
        let x = distance / maxPullDistance
        return maxPullDistance * (1 - exp(-coefficient * x)) / (1 - exp(-coefficient))
    }
}

/// Test automation reads the indicator by symbol lookup, so builds without this SDK still
/// link. Main thread only; false when the page has no indicator.
@_cdecl("lx_ios_pull_refresh_state")
public func lxIosPullRefreshState(
    _ appId: UnsafePointer<CChar>, _ webtag: UnsafePointer<CChar>,
    _ phase: UnsafeMutablePointer<CChar>, _ phaseCapacity: Int,
    _ offset: UnsafeMutablePointer<Double>
) -> Bool {
    let appId = String(cString: appId)
    let webtag = String(cString: webtag)
    let state: (String, Double)? = MainActor.assumeIsolated {
        guard let refresh = GpuPages.view(appId: appId, webtag: webtag)?.pullToRefresh else { return nil }
        return (refresh.phase, Double(refresh.indicatorOffset))
    }
    guard let state else { return false }
    let bytes = Array(state.0.utf8.prefix(phaseCapacity - 1))
    for (index, byte) in bytes.enumerated() { phase[index] = CChar(bitPattern: byte) }
    phase[bytes.count] = 0
    offset.pointee = state.1
    return true
}
#endif
