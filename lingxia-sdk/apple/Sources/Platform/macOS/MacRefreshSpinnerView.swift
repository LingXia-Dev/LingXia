#if os(macOS)
import AppKit

/// The mobile pull-to-refresh indicator, for a runner phone or tablet frame.
/// It mirrors iOS's `RefreshIndicatorView`: a `UIActivityIndicatorView`
/// (`.medium`) spoke spinner fixed 40pt below the covered top edge, still and
/// growing from 60% while the page is pulled, ticking round once it refreshes.
/// It lives in the strip a pull reveals behind the page, which clips it until
/// the page has moved far enough to show it.
@MainActor
final class MacRefreshSpinnerView: NSView {
    /// Where the spinner centres below `topInset`, as on iOS.
    private static let spinnerCenterY: CGFloat = 40

    /// Pinned by Auto Layout so it holds still on screen while the strip, and
    /// the page edge with it, animates.
    private let glyph = SpokeSpinnerView()
    private var glyphCenterY: NSLayoutConstraint?

    /// Height at the top of the strip covered by host chrome (a simulated
    /// status bar), the way iOS reads the web view's safe area.
    var topInset: CGFloat = 0 {
        didSet { glyphCenterY?.constant = topInset + Self.spinnerCenterY }
    }

    init() {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.masksToBounds = true
        setAccessibilityElement(false)

        glyph.translatesAutoresizingMaskIntoConstraints = false
        addSubview(glyph)
        let centerY = glyph.centerYAnchor.constraint(
            equalTo: topAnchor, constant: Self.spinnerCenterY)
        glyphCenterY = centerY
        NSLayoutConstraint.activate([
            glyph.widthAnchor.constraint(equalToConstant: SpokeSpinnerView.side),
            glyph.heightAnchor.constraint(equalToConstant: SpokeSpinnerView.side),
            glyph.centerXAnchor.constraint(equalTo: centerXAnchor),
            centerY,
        ])
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    /// The theme's refresh colour, else one readable on the strip.
    func setColor(_ color: NSColor) {
        glyph.color = color
    }

    /// Pulling: the spinner holds still and grows into place.
    func setProgress(_ progress: CGFloat) {
        guard !glyph.isSpinning else { return }
        glyph.scale = 0.6 + 0.4 * min(max(progress, 0), 1)
    }

    /// Reduce Motion keeps the spinner still.
    func startSpinning(reduceMotion: Bool) {
        glyph.scale = 1
        if !reduceMotion { glyph.startSpinning() }
    }

    func stopSpinning() {
        glyph.stopSpinning()
    }
}

/// Eight spokes fading counter-clockwise from the leading one, stepping a
/// spoke at a time like the system spinner. Drawn rather than built from
/// sublayers: AppKit rebuilds a moved view's backing layers, and a runner
/// device switch moves the page between windows.
@MainActor
private final class SpokeSpinnerView: NSView {
    /// `UIActivityIndicatorView.Style.medium`.
    static let side: CGFloat = 20
    private static let spokes = 8
    private static let spokeSize = CGSize(width: 2, height: 5.5)
    /// From the centre to the middle of a spoke.
    private static let spokeRadius: CGFloat = 6.25
    /// One turn takes eight of these.
    private static let tick: TimeInterval = 0.1

    var color: NSColor = .secondaryLabelColor {
        didSet { needsDisplay = true }
    }
    var scale: CGFloat = 1 {
        didSet { if scale != oldValue { needsDisplay = true } }
    }
    private var phase = 0
    private var timer: Timer?
    var isSpinning: Bool { timer != nil }

    override var isFlipped: Bool { true }

    func startSpinning() {
        guard timer == nil else { return }
        let timer = Timer(timeInterval: Self.tick, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.phase = (self.phase + 1) % Self.spokes
                self.needsDisplay = true
            }
        }
        // Common modes, so it keeps turning while the user scrolls the page.
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    func stopSpinning() {
        timer?.invalidate()
        timer = nil
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil { stopSpinning() }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        let step = 2 * CGFloat.pi / CGFloat(Self.spokes)
        let size = Self.spokeSize
        let spoke = CGRect(
            x: -size.width / 2, y: -Self.spokeRadius - size.height / 2,
            width: size.width, height: size.height)
        let path = CGPath(
            roundedRect: spoke, cornerWidth: size.width / 2, cornerHeight: size.width / 2,
            transform: nil)
        context.saveGState()
        context.translateBy(x: bounds.midX, y: bounds.midY)
        context.scaleBy(x: scale, y: scale)
        for index in 0..<Self.spokes {
            // Flipped, so a positive angle is clockwise: spoke `phase` leads
            // and the ones behind it, counter-clockwise, fade.
            let behind = (phase - index + Self.spokes) % Self.spokes
            context.saveGState()
            context.rotate(by: CGFloat(index) * step)
            context.setFillColor(
                color.withAlphaComponent(
                    color.alphaComponent * (1 - 0.7 * CGFloat(behind) / CGFloat(Self.spokes - 1))
                ).cgColor)
            context.addPath(path)
            context.fillPath()
            context.restoreGState()
        }
        context.restoreGState()
    }
}
#endif
