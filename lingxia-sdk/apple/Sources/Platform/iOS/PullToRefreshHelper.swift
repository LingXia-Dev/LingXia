#if os(iOS)
import UIKit
import WebKit
import os.log

/// Pull-to-refresh with a system activity indicator in the host theme's colour.
@MainActor
class PullToRefreshHelper: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "PullToRefresh")
    
    weak var webView: WKWebView?
    private var refreshIndicator: RefreshIndicatorView?
    private var isRefreshing = false
    private var isEnabled = true
    private var onRefresh: (() -> Void)?
    // What startRefreshing added to contentInset.top and has not yet taken back.
    private var appliedInsetTop: CGFloat = 0
    private var shownAt: CFTimeInterval = 0
    // Set while the indicator lingers after a stop to honour minVisibleDuration.
    private var pendingDismiss: DispatchWorkItem?

    private let triggerDistance: CGFloat = 80.0
    private let maxPullDistance: CGFloat = 150.0
    // A refresh that ends at once would only flash the spinner.
    private let minVisibleDuration: CFTimeInterval = 0.4
    
    init(webView: WKWebView, onRefresh: @escaping () -> Void) {
        self.webView = webView
        self.onRefresh = onRefresh
        super.init()
        setupRefreshIndicator()
    }
    
    @MainActor
    private func setupRefreshIndicator() {
        guard let webView = webView else { return }
        
        let indicator = RefreshIndicatorView(frame: CGRect(x: 0, y: 0, width: webView.bounds.width, height: maxPullDistance))
        indicator.translatesAutoresizingMaskIntoConstraints = false
        indicator.isHidden = true
        indicator.alpha = 0
        indicator.isUserInteractionEnabled = false
        
        webView.addSubview(indicator)
        
        NSLayoutConstraint.activate([
            indicator.topAnchor.constraint(equalTo: webView.topAnchor),
            indicator.leadingAnchor.constraint(equalTo: webView.leadingAnchor),
            indicator.trailingAnchor.constraint(equalTo: webView.trailingAnchor),
            indicator.heightAnchor.constraint(equalToConstant: maxPullDistance)
        ])
        
        self.refreshIndicator = indicator
        webView.scrollView.addObserver(self, forKeyPath: "contentOffset", options: [.new], context: nil)
    }
    
    @MainActor
    func setEnabled(_ enabled: Bool) {
        isEnabled = enabled
        os_log("Pull-to-refresh enabled=%{public}@", log: Self.log, type: .info, enabled ? "true" : "false")
        if enabled {
            webView?.scrollView.alwaysBounceVertical = true
        } else {
            // Put the indicator away now, lingering or not.
            let showing = isRefreshing || pendingDismiss != nil
            isRefreshing = false
            cancelPendingDismiss()
            if showing {
                // Dropped, not finished: nothing to announce.
                dismiss(announce: false)
            } else {
                resetState()
            }
        }
    }
    
    override func observeValue(forKeyPath keyPath: String?, of object: Any?, change: [NSKeyValueChangeKey : Any]?, context: UnsafeMutableRawPointer?) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self, keyPath == "contentOffset", let webView = self.webView, self.isEnabled,
                  !self.isRefreshing, self.pendingDismiss == nil else { return }
            
            let offset = webView.scrollView.contentOffset.y + webView.scrollView.adjustedContentInset.top
            let pullDistance = max(0, -offset)
            
            if pullDistance > 1 {
                self.updatePullState(pullDistance: pullDistance)
            } else {
                self.resetState()
            }
            
            if !webView.scrollView.isDragging && pullDistance >= self.triggerDistance {
                self.startRefreshing()
            }
        }
    }
    
    @MainActor
    private func updatePullState(pullDistance: CGFloat) {
        guard let indicator = refreshIndicator else { return }
        let clampedDistance = rubberBandClamp(distance: pullDistance, maxDistance: maxPullDistance)
        let progress = min(1.0, clampedDistance / triggerDistance)
        
        if indicator.isHidden {
            indicator.applyTint(spinnerColor())
            indicator.topInset = safeTop()
            indicator.isHidden = false
        }
        indicator.alpha = min(1.0, progress * 1.5)
        indicator.setProgress(progress)
    }
    
    private func rubberBandClamp(distance: CGFloat, maxDistance: CGFloat) -> CGFloat {
        let coefficient: CGFloat = 0.55
        let x = distance / maxDistance
        let numerator = 1.0 - exp(-coefficient * x)
        let denominator = 1.0 - exp(-coefficient)
        return maxDistance * (numerator / denominator)
    }
    
    @MainActor
    func startRefreshing() {
        guard !isRefreshing, isEnabled, let webView = webView, let indicator = refreshIndicator else { return }

        if pendingDismiss != nil {
            // Restarted while lingering: the spinner is still up, keep it.
            cancelPendingDismiss()
            isRefreshing = true
            UIAccessibility.post(notification: .announcement, argument: L10n.string("lx_pull_refresh_refreshing"))
            onRefresh?()
            return
        }

        isRefreshing = true
        shownAt = CACurrentMediaTime()

        indicator.applyTint(spinnerColor())
        indicator.topInset = safeTop()
        indicator.isHidden = false
        indicator.alpha = 1.0
        indicator.startLoading(reduceMotion: UIAccessibility.isReduceMotionEnabled)
        UIAccessibility.post(notification: .announcement, argument: L10n.string("lx_pull_refresh_refreshing"))

        // Hold below the status bar and notch on a page without a navigation
        // bar, so the spinner has the same room there as under one.
        let holdInset = safeTop() + triggerDistance * 0.8
        appliedInsetTop = holdInset
        let restingInset = webView.scrollView.contentInset.top + holdInset
        UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseOut]) {
            webView.scrollView.contentInset.top = restingInset
            webView.scrollView.contentOffset.y = -restingInset
        }

        onRefresh?()
        os_log("Pull-to-refresh started", log: Self.log, type: .info)
    }

    /// Ends the refresh. Returns at once; the indicator itself lingers until
    /// it has been up for `minVisibleDuration`.
    @MainActor
    func endRefreshing() {
        guard isRefreshing else { return }
        isRefreshing = false

        let remaining = minVisibleDuration - (CACurrentMediaTime() - shownAt)
        guard remaining > 0 else {
            dismiss()
            return
        }
        let work = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated {
                self?.pendingDismiss = nil
                self?.dismiss()
            }
        }
        pendingDismiss = work
        DispatchQueue.main.asyncAfter(deadline: .now() + remaining, execute: work)
    }

    private func cancelPendingDismiss() {
        pendingDismiss?.cancel()
        pendingDismiss = nil
    }

    private func dismiss(announce: Bool = true) {
        refreshIndicator?.stopLoading()
        if announce {
            UIAccessibility.post(notification: .announcement, argument: L10n.string("lx_pull_refresh_refreshed"))
        }

        let inset = takeAppliedInset()
        if let webView = webView {
            UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseOut]) {
                webView.scrollView.contentInset.top -= inset
            } completion: { [weak self] _ in
                self?.resetState()
            }
        }
        os_log("Pull-to-refresh ended", log: Self.log, type: .info)
    }

    /// The part of the web view the status bar and notch cover. Under a
    /// navigation bar it is zero, because UIKit computes the web view's
    /// safe area that way; nothing here checks for one.
    private func safeTop() -> CGFloat {
        webView?.safeAreaInsets.top ?? 0
    }

    private func spinnerColor() -> UIColor? {
        let dark = WebViewManager.resolvedDarkAppearance(appId: webView?.appId)
        return WebViewManager.declaredRefreshIndicatorColor(dark: dark)
    }

    /// The inset still owed back to the scroll view; zero after this, so it is paid once.
    private func takeAppliedInset() -> CGFloat {
        defer { appliedInsetTop = 0 }
        return appliedInsetTop
    }
    
    @MainActor
    private func resetState() {
        refreshIndicator?.isHidden = true
        refreshIndicator?.alpha = 0
        refreshIndicator?.setProgress(0)
    }
    
    deinit {
        let owedInset = appliedInsetTop
        let targetWebView = webView
        let indicator = refreshIndicator

        targetWebView?.scrollView.removeObserver(self, forKeyPath: "contentOffset")

        let cleanup = {
            if owedInset != 0, let webView = targetWebView {
                webView.scrollView.contentInset.top -= owedInset
            }
            indicator?.stopLoading()
            indicator?.removeFromSuperview()
        }
        
        if Thread.isMainThread { cleanup() } else { DispatchQueue.main.async(execute: cleanup) }
    }
}

private class RefreshIndicatorView: UIView {
    private let spinner = UIActivityIndicatorView(style: .medium)
    // Where the spinner sits in the strip the pull opens, below [topInset].
    private let spinnerCenterY: CGFloat = 40.0
    var topInset: CGFloat = 0 {
        didSet { if topInset != oldValue { setNeedsLayout() } }
    }

    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        spinner.hidesWhenStopped = false
        addSubview(spinner)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func layoutSubviews() {
        super.layoutSubviews()
        spinner.center = CGPoint(x: bounds.width / 2.0, y: topInset + spinnerCenterY)
    }

    /// `nil` keeps the system colour.
    func applyTint(_ color: UIColor?) {
        spinner.color = color ?? .secondaryLabel
    }

    /// Pulling: the spinner holds still and grows into place.
    func setProgress(_ progress: CGFloat) {
        guard !spinner.isAnimating else { return }
        let scale = 0.6 + 0.4 * max(0, min(1.0, progress))
        spinner.transform = CGAffineTransform(scaleX: scale, y: scale)
    }

    /// Reduce Motion keeps the spinner still.
    func startLoading(reduceMotion: Bool) {
        spinner.transform = .identity
        if !reduceMotion {
            spinner.startAnimating()
        }
    }

    func stopLoading() {
        spinner.stopAnimating()
    }
}
#endif
