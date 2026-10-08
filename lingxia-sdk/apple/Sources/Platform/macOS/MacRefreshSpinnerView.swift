#if os(macOS)
import AppKit

/// Arc for a pull-to-refresh pull: it sits in the strip a trackpad pull
/// reveals behind the page and grows with the pull.
///
/// Drawn rather than an `NSProgressIndicator`, which AppKit gives no public way
/// to tint.
@MainActor
final class MacRefreshSpinnerView: NSView {
    /// Arc shown at the start of a pull, so the spinner never vanishes to a dot.
    private static let minimumSweep: CGFloat = 0.08
    /// Arc once the pull is far enough to refresh.
    private static let fullSweep: CGFloat = 0.75
    /// The ring's centre line; the stroke is centred on it.
    private static let radius: CGFloat = 9
    private static let lineWidth: CGFloat = 2.5

    private let arc = CAShapeLayer()

    init() {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.masksToBounds = true
        setAccessibilityElement(false)

        let (radius, lineWidth) = (Self.radius, Self.lineWidth)
        let side = (radius + lineWidth) * 2
        arc.bounds = CGRect(x: 0, y: 0, width: side, height: side)
        // A full circle from 12 o'clock, clockwise; strokeEnd shows a part of it.
        let path = CGMutablePath()
        path.addArc(
            center: CGPoint(x: side / 2, y: side / 2), radius: radius,
            startAngle: .pi / 2, endAngle: .pi / 2 - 2 * .pi, clockwise: true)
        arc.path = path
        arc.fillColor = nil
        arc.strokeColor = NSColor.secondaryLabelColor.cgColor
        arc.lineWidth = lineWidth
        arc.lineCap = .round
        arc.strokeEnd = Self.minimumSweep
        layer?.addSublayer(arc)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// Explicitly set the arc colour. The host passes the theme's refresh
    /// colour or one that contrasts with the page background, so the spinner
    /// stays visible regardless of the view's effective appearance.
    func setColor(_ color: NSColor) {
        withoutImplicitAnimation { arc.strokeColor = color.cgColor }
    }

    /// Grow the arc with the pull: `progress` is the pull over the trigger
    /// distance, so the arc is as full as it gets once a release would refresh.
    func setProgress(_ progress: CGFloat) {
        let clamped = min(max(progress, 0), 1)
        withoutImplicitAnimation {
            arc.strokeEnd = Self.minimumSweep
                + (Self.fullSweep - Self.minimumSweep) * clamped
        }
    }

    override func layout() {
        super.layout()
        withoutImplicitAnimation {
            arc.position = CGPoint(x: bounds.midX, y: bounds.midY)
        }
    }

    private func withoutImplicitAnimation(_ body: () -> Void) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        body()
        CATransaction.commit()
    }
}
#endif
