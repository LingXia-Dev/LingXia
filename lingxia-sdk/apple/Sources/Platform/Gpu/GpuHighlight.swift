import Foundation
import QuartzCore

/// Press or focus-ring feedback over a GPU page, in the shape and masks the core chose.
final class GpuHighlightLayer: CALayer {
    private var shape: CGPath?
    private var masks: [CGPath] = []
    private var color: CGColor?
    private var ring: CGFloat = 0

    override init() {
        super.init()
        needsDisplayOnBoundsChange = true
        // No implicit fades: feedback tracks the finger.
        actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull(), "hidden": NSNull()]
    }

    override init(layer: Any) { super.init(layer: layer) }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    /// `rects`: 12 floats each as the core sends them; empty hides the layer.
    func update(rects: UnsafeBufferPointer<Float>, argb: UInt32, ringWidth: Float) {
        let count = rects.count / 12
        guard count > 0 else {
            shape = nil
            isHidden = true
            return
        }
        let paths = (0..<count).map { Self.path(Array(rects[($0 * 12)..<($0 * 12 + 12)])) }
        ring = CGFloat(ringWidth)
        shape = ring > 0 ? Self.path(Array(rects[0..<12]), inset: ring / 2) : paths[0]
        masks = Array(paths.dropFirst())
        let channel = { (shift: UInt32) in CGFloat((argb >> shift) & 0xff) / 255 }
        color = CGColor(srgbRed: channel(16), green: channel(8), blue: channel(0), alpha: channel(24))
        isHidden = false
        setNeedsDisplay()
    }

    override func draw(in context: CGContext) {
        guard let shape, let color else { return }
        for mask in masks {
            context.addPath(mask)
            context.clip()
        }
        context.addPath(shape)
        if ring > 0 {
            context.setStrokeColor(color)
            context.setLineWidth(ring)
            context.strokePath()
        } else {
            context.setFillColor(color)
            context.fillPath()
        }
    }

    /// A rounded rect with elliptical corners from top left clockwise, `inset` on every side.
    private static func path(_ v: [Float], inset: CGFloat = 0) -> CGPath {
        let rect = CGRect(x: CGFloat(v[0]), y: CGFloat(v[1]), width: CGFloat(v[2]), height: CGFloat(v[3]))
            .insetBy(dx: inset, dy: inset)
        let path = CGMutablePath()
        guard rect.width > 0, rect.height > 0 else { return path }
        let r = (0..<4).map { (max(0, CGFloat(v[4 + $0 * 2]) - inset), max(0, CGFloat(v[5 + $0 * 2]) - inset)) }
        let corners = [
            (CGPoint(x: rect.minX + r[0].0, y: rect.minY + r[0].1), r[0], CGFloat.pi),
            (CGPoint(x: rect.maxX - r[1].0, y: rect.minY + r[1].1), r[1], CGFloat.pi * 1.5),
            (CGPoint(x: rect.maxX - r[2].0, y: rect.maxY - r[2].1), r[2], 0),
            (CGPoint(x: rect.minX + r[3].0, y: rect.maxY - r[3].1), r[3], CGFloat.pi / 2),
        ]
        for (center, radii, start) in corners {
            if radii.0 > 0, radii.1 > 0 {
                let transform = CGAffineTransform(translationX: center.x, y: center.y).scaledBy(x: radii.0, y: radii.1)
                path.addArc(center: .zero, radius: 1, startAngle: start, endAngle: start + .pi / 2, clockwise: false, transform: transform)
            } else if path.isEmpty {
                // A square corner's center is the corner itself.
                path.move(to: center)
            } else {
                path.addLine(to: center)
            }
        }
        path.closeSubpath()
        return path
    }
}
