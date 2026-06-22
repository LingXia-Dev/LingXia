#if os(macOS)
import AppKit

/// Animated three-dot pull-to-refresh indicator. Hosted inside the view controller's
/// in-layout refresh strip (between the navigation bar and the web view), not as a
/// floating overlay, so an active refresh pushes the web content down.
@MainActor
final class MacRefreshIndicatorView: NSView {
    private let dots: [CALayer] = (0..<3).map { _ in CALayer() }
    private let dotRadius: CGFloat = 4.0
    private let dotSpacing: CGFloat = 13.0

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

    /// Explicitly set the dot color. The host derives a color that contrasts with the page
    /// background, so the indicator stays visible regardless of the view's effective appearance
    /// (semantic colors like secondaryLabelColor can resolve to a near-invisible tint against a
    /// background-matched strip).
    func setDotColor(_ color: NSColor) {
        let cgColor = color.cgColor
        for dot in dots {
            dot.backgroundColor = cgColor
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
