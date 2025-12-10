#if os(iOS)
import UIKit
import WebKit
import os.log

/// 3-dots pull-to-refresh indicator matching Android implementation using Core Animation
@MainActor
class PullToRefreshHelper: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "PullToRefresh")
    
    weak var webView: WKWebView?
    private var refreshIndicator: RefreshIndicatorView?
    private var isRefreshing = false
    private var isEnabled = true
    private var onRefresh: (() -> Void)?
    private var originalContentInsetTop: CGFloat = 0
    
    private let triggerDistance: CGFloat = 80.0
    private let maxPullDistance: CGFloat = 150.0
    
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
            isRefreshing = false
            resetState()
        }
    }
    
    override func observeValue(forKeyPath keyPath: String?, of object: Any?, change: [NSKeyValueChangeKey : Any]?, context: UnsafeMutableRawPointer?) {
        DispatchQueue.main.async { [weak self] in
            guard let self = self, keyPath == "contentOffset", let webView = self.webView, self.isEnabled, !self.isRefreshing else { return }
            
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
        
        indicator.isHidden = false
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
        
        isRefreshing = true
        originalContentInsetTop = webView.scrollView.contentInset.top
        
        indicator.isHidden = false
        indicator.alpha = 1.0
        indicator.startLoading()
        
        let refreshPosition = triggerDistance * 0.8
        UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseOut]) {
            webView.scrollView.contentInset.top = self.originalContentInsetTop + refreshPosition
            webView.scrollView.contentOffset.y = -(self.originalContentInsetTop + refreshPosition)
        }
        
        onRefresh?()
        os_log("Pull-to-refresh started", log: Self.log, type: .info)
    }
    
    @MainActor
    func endRefreshing() {
        guard isRefreshing, let webView = webView else { return }
        
        isRefreshing = false
        refreshIndicator?.stopLoading()
        
        UIView.animate(withDuration: 0.25, delay: 0, options: [.curveEaseOut]) {
            webView.scrollView.contentInset.top = self.originalContentInsetTop
        } completion: { [weak self] _ in
            self?.resetState()
        }
        os_log("Pull-to-refresh ended", log: Self.log, type: .info)
    }
    
    @MainActor
    private func resetState() {
        refreshIndicator?.isHidden = true
        refreshIndicator?.alpha = 0
        refreshIndicator?.setProgress(0)
    }
    
    deinit {
        let wasRefreshing = isRefreshing
        let restoreTo = originalContentInsetTop
        let targetWebView = webView
        let indicator = refreshIndicator
        
        targetWebView?.scrollView.removeObserver(self, forKeyPath: "contentOffset")
        
        let cleanup = {
            if wasRefreshing, let webView = targetWebView {
                webView.scrollView.contentInset.top = restoreTo
            }
            indicator?.stopLoading()
            indicator?.removeFromSuperview()
        }
        
        if Thread.isMainThread { cleanup() } else { DispatchQueue.main.async(execute: cleanup) }
    }
}

private class RefreshIndicatorView: UIView {
    private let dots: [CALayer] = (0..<3).map { _ in CALayer() }
    private let dotRadius: CGFloat = 3.5
    private let dotSpacing: CGFloat = 12.0
    private let dotColor = UIColor(white: 0.53, alpha: 1.0).cgColor
    
    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        dots.forEach {
            $0.backgroundColor = dotColor
            $0.cornerRadius = dotRadius
            $0.frame = CGRect(x: 0, y: 0, width: dotRadius * 2, height: dotRadius * 2)
            layer.addSublayer($0)
        }
    }
    
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    
    override func layoutSubviews() {
        super.layoutSubviews()
        let cx = bounds.width / 2.0
        let cy: CGFloat = 40.0
        for (i, dot) in dots.enumerated() {
            dot.position = CGPoint(x: cx + CGFloat(i - 1) * dotSpacing, y: cy)
        }
    }
    
    func setProgress(_ progress: CGFloat) {
        let scale = max(0, min(1.0, progress))
        let alpha = 0.4 + 0.6 * scale
        let dotScale = 0.5 + 0.5 * scale
        
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        dots.forEach {
            $0.opacity = Float(alpha)
            $0.transform = CATransform3DMakeScale(dotScale, dotScale, 1.0)
            $0.removeAllAnimations()
        }
        CATransaction.commit()
    }
    
    func startLoading() {
        dots.enumerated().forEach { (i, dot) in
            dot.transform = CATransform3DIdentity
            dot.opacity = 0.3
            dot.removeAllAnimations()
            
            let anim = CAKeyframeAnimation(keyPath: "opacity")
            anim.values = [0.3, 1.0, 0.3]
            anim.keyTimes = [0, 0.2, 1]
            anim.duration = 0.6
            anim.repeatCount = .infinity
            anim.beginTime = CACurrentMediaTime() + (Double(i) * 0.2)
            dot.add(anim, forKey: "loading")
        }
    }
    
    func stopLoading() {
        dots.forEach { $0.removeAllAnimations() }
    }
}
#endif