#if os(macOS)
import AppKit
import WebKit
import os.log

@MainActor
final class MacPullToRefreshHelper {
    private static let log = OSLog(subsystem: "LingXia", category: "PullToRefresh")

    weak var webView: WKWebView?
    private weak var hostView: NSView?
    private var indicatorContainer: NSView?
    private var indicator: MacRefreshIndicatorView?
    private var indicatorTopConstraint: NSLayoutConstraint?
    private var isRefreshing = false

    init(webView: WKWebView) {
        self.webView = webView
        setupIndicatorIfNeeded()
    }

    func startRefreshing() {
        guard !isRefreshing else { return }
        setupIndicatorIfNeeded()
        guard let indicatorContainer = indicatorContainer, let indicator = indicator else { return }

        isRefreshing = true
        indicatorContainer.isHidden = false
        indicator.startLoading()
        indicatorContainer.alphaValue = 0.0
        indicatorTopConstraint?.constant = -6.0
        hostView?.layoutSubtreeIfNeeded()

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.22
            context.timingFunction = CAMediaTimingFunction(name: .easeOut)
            indicatorContainer.animator().alphaValue = 1.0
            indicatorTopConstraint?.animator().constant = 10.0
            hostView?.layoutSubtreeIfNeeded()
        }

        os_log("macOS pull-to-refresh started", log: Self.log, type: .info)
    }

    func endRefreshing() {
        guard isRefreshing else { return }
        isRefreshing = false

        guard let indicatorContainer = indicatorContainer else { return }
        indicator?.stopLoading()

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.18
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            indicatorContainer.animator().alphaValue = 0.0
            indicatorTopConstraint?.animator().constant = -6.0
            hostView?.layoutSubtreeIfNeeded()
        } completionHandler: { [weak indicatorContainer] in
            Task { @MainActor in
                indicatorContainer?.isHidden = true
            }
        }

        os_log("macOS pull-to-refresh ended", log: Self.log, type: .info)
    }

    private func setupIndicatorIfNeeded() {
        guard let webView = webView else { return }
        guard let currentHostView = webView.superview else { return }

        if let hostView = hostView,
           hostView === currentHostView,
           indicatorContainer?.superview === hostView {
            return
        }

        indicatorContainer?.removeFromSuperview()
        hostView = currentHostView

        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        container.layer?.cornerRadius = 14
        container.layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.94).cgColor
        container.layer?.borderWidth = 1
        container.layer?.borderColor = NSColor.separatorColor.withAlphaComponent(0.4).cgColor
        container.layer?.shadowColor = NSColor.black.withAlphaComponent(0.22).cgColor
        container.layer?.shadowOpacity = 1
        container.layer?.shadowRadius = 10
        container.layer?.shadowOffset = CGSize(width: 0, height: 4)
        container.alphaValue = 0.0
        container.isHidden = true

        let dots = MacRefreshIndicatorView()
        dots.translatesAutoresizingMaskIntoConstraints = false

        container.addSubview(dots)
        currentHostView.addSubview(container)

        let top = container.topAnchor.constraint(equalTo: webView.topAnchor, constant: -6)
        indicatorTopConstraint = top

        NSLayoutConstraint.activate([
            top,
            container.centerXAnchor.constraint(equalTo: webView.centerXAnchor),
            container.widthAnchor.constraint(equalToConstant: 64),
            container.heightAnchor.constraint(equalToConstant: 32),
            dots.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            dots.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            dots.topAnchor.constraint(equalTo: container.topAnchor),
            dots.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])

        indicatorContainer = container
        indicator = dots
    }

    deinit {
        let container = indicatorContainer
        let spinner = indicator
        Task { @MainActor in
            spinner?.stopLoading()
            container?.removeFromSuperview()
        }
    }
}

private final class MacRefreshIndicatorView: NSView {
    private let dots: [CALayer] = (0..<3).map { _ in CALayer() }
    private let dotRadius: CGFloat = 3.4
    private let dotSpacing: CGFloat = 12.0

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.masksToBounds = false

        for dot in dots {
            dot.backgroundColor = NSColor.secondaryLabelColor.cgColor
            dot.cornerRadius = dotRadius
            dot.frame = CGRect(x: 0, y: 0, width: dotRadius * 2, height: dotRadius * 2)
            layer?.addSublayer(dot)
        }
    }

    convenience init() {
        self.init(frame: .zero)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func layout() {
        super.layout()
        let centerX = bounds.width / 2
        let centerY = bounds.height / 2
        for (i, dot) in dots.enumerated() {
            dot.position = CGPoint(
                x: centerX + CGFloat(i - 1) * dotSpacing,
                y: centerY
            )
        }
    }

    func startLoading() {
        for (index, dot) in dots.enumerated() {
            dot.removeAllAnimations()

            let opacity = CAKeyframeAnimation(keyPath: "opacity")
            opacity.values = [0.28, 1.0, 0.28]
            opacity.keyTimes = [0, 0.24, 1]
            opacity.duration = 0.72
            opacity.repeatCount = .infinity

            let scale = CAKeyframeAnimation(keyPath: "transform.scale")
            scale.values = [0.85, 1.18, 0.85]
            scale.keyTimes = [0, 0.24, 1]
            scale.duration = 0.72
            scale.repeatCount = .infinity

            let group = CAAnimationGroup()
            group.animations = [opacity, scale]
            group.duration = 0.72
            group.repeatCount = .infinity
            group.isRemovedOnCompletion = false
            group.beginTime = CACurrentMediaTime() + Double(index) * 0.12
            dot.add(group, forKey: "loading")
        }
    }

    func stopLoading() {
        for dot in dots {
            dot.removeAllAnimations()
            dot.opacity = 0.5
            dot.transform = CATransform3DIdentity
        }
    }
}
#endif
