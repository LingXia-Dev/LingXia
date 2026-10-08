#if os(macOS)
import AppKit
import QuartzCore

/// The visible state of a refresh on macOS: a full-width bar along the top
/// edge of the page in the theme colour, with a brighter band flowing along
/// it while the refresh runs.
///
/// Most desktop refreshes have no pull behind them (the context menu, ⌘R,
/// `lx.startPullDownRefresh()`), so the page never moves for one: the bar is
/// laid over it, never takes focus or clicks, and is not an accessibility
/// element; the controller announces the refresh instead. Every animation
/// runs in Core Animation, so the bar keeps moving while the main thread is
/// busy reloading the page. Each transition starts from what is on screen,
/// so a restart while the bar finishes never flashes.
@MainActor
final class MacRefreshBarView: NSView {
    private typealias Timeline = MacRefreshBarTimeline

    private static let barHeight: CGFloat = 3

    /// Layer tree: `content` carries the fade in and out, `pulse` the Reduce
    /// Motion breathing; inside, the translucent base, the band and the solid
    /// fill that "done" turns on. No layer has a transform, so geometry set on
    /// one run cannot leak into the next.
    private let content = CALayer()
    private let pulse = CALayer()
    private let base = CALayer()
    private let band = CAGradientLayer()
    private let solid = CALayer()
    private var timeline = Timeline()
    private var color = NSColor.secondaryLabelColor
    private var constraintsInContainer: [NSLayoutConstraint] = []
    private var laidOutWidth: CGFloat = -1
    /// Bumped on every change, so a finished fade only hides the bar if
    /// nothing restarted it.
    private var generation = 0

    init() {
        super.init(frame: .zero)
        wantsLayer = true
        layer?.masksToBounds = true
        translatesAutoresizingMaskIntoConstraints = false
        isHidden = true
        setAccessibilityElement(false)
        band.startPoint = CGPoint(x: 0, y: 0.5)
        band.endPoint = CGPoint(x: 1, y: 0.5)
        band.anchorPoint = .zero
        band.locations = [0, Timeline.bandEdge, 1 - Timeline.bandEdge, 1].map { NSNumber(value: $0) }
        pulse.addSublayer(base)
        pulse.addSublayer(band)
        pulse.addSublayer(solid)
        content.addSublayer(pulse)
        layer?.addSublayer(content)
        resetLayers()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool { false }

    /// Shows the bar over everything else in `container`, or brings a bar
    /// that is still finishing the previous refresh straight back to running.
    /// `color` is the theme's refresh colour; `nil` follows the appearance.
    func start(in container: NSView, color: NSColor?) {
        // Last in the container, so later siblings (the next page, component
        // overlays) never cover it. Moved without `hide()`, which would lose
        // a finish the new run carries on from.
        if superview !== container || container.subviews.last !== self {
            NSLayoutConstraint.deactivate(constraintsInContainer)
            removeFromSuperview()
            container.addSubview(self)
            constraintsInContainer = [
                topAnchor.constraint(equalTo: container.topAnchor),
                leadingAnchor.constraint(equalTo: container.leadingAnchor),
                trailingAnchor.constraint(equalTo: container.trailingAnchor),
                heightAnchor.constraint(equalToConstant: Self.barHeight),
            ]
            // `layout()` starts the sweep once the width is known.
            NSLayoutConstraint.activate(constraintsInContainer)
        }
        let now = CACurrentMediaTime()
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        let start = timeline.start(at: now, reduceMotion: reduceMotion)
        guard start != .unchanged else { return }
        generation += 1
        self.color = color ?? .secondaryLabelColor
        isHidden = false
        layoutLayers()
        let fresh = start == .fresh
        if fresh { removeAnimations() }
        // Fade in from nothing, or from the finish a resumed run interrupts.
        let solidity: Float = reduceMotion ? 1 : 0
        fade(content, from: fresh ? 0 : nil, to: 1, over: Timeline.fadeIn)
        fade(solid, from: fresh ? solidity : nil, to: solidity, over: Timeline.fadeIn)
        fade(pulse, from: fresh ? 1 : nil, to: 1, over: Timeline.fadeIn)
        if reduceMotion {
            let breathing = CABasicAnimation(keyPath: "opacity")
            breathing.fromValue = 1
            breathing.toValue = Timeline.breathLow
            breathing.duration = Timeline.breathPeriod / 2
            breathing.autoreverses = true
            breathing.repeatCount = .infinity
            breathing.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            breathing.beginTime = now + Timeline.fadeIn
            pulse.add(breathing, forKey: "breath")
        } else {
            pulse.removeAnimation(forKey: "breath")
        }
        setBandVisible(!reduceMotion)
        if !reduceMotion && band.animation(forKey: "sweep") == nil {
            addSweep(at: now)
        }
    }

    /// The refresh finished: turn solid, then fade out.
    func complete() {
        let now = CACurrentMediaTime()
        guard timeline.complete(at: now) else { return }
        generation += 1
        pulse.removeAnimation(forKey: "breath")
        fade(pulse, to: 1, over: Timeline.solidDuration)
        fade(solid, to: 1, over: Timeline.solidDuration)
        fade(content, to: 1, over: Timeline.solidDuration)
        // Overrides the turn to solid once it begins, and holds at zero.
        let fadeOut = CABasicAnimation(keyPath: "opacity")
        fadeOut.fromValue = 1
        fadeOut.toValue = 0
        fadeOut.duration = Timeline.fadeOut
        fadeOut.beginTime = now + Timeline.solidDuration
        content.add(fadeOut, forKey: "fadeOut")
        setModel(content, 0)
        let generation = generation
        let span = Timeline.solidDuration + Timeline.fadeOut
        DispatchQueue.main.asyncAfter(deadline: .now() + span) { [weak self] in
            MainActor.assumeIsolated {
                guard let self, self.generation == generation else { return }
                self.hide()
            }
        }
    }

    /// Takes the bar down at once, for a refresh that was dropped.
    func hide() {
        generation += 1
        timeline.reset()
        removeAnimations()
        resetLayers()
        isHidden = true
    }

    /// Leaves the container with the page that owned it.
    func detach() {
        hide()
        NSLayoutConstraint.deactivate(constraintsInContainer)
        constraintsInContainer = []
        removeFromSuperview()
    }

    override func layout() {
        super.layout()
        guard bounds.width != laidOutWidth else { return }
        layoutLayers()
        // The sweep is in points: restart it, at the same phase, for the new
        // width.
        if timeline.isShown(at: CACurrentMediaTime()) && !timeline.reduceMotion {
            addSweep(at: CACurrentMediaTime())
        }
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        layoutLayers()
    }

    private func layoutLayers() {
        let bounds = bounds
        laidOutWidth = bounds.width
        var cgColor = color.cgColor
        effectiveAppearance.performAsCurrentDrawingAppearance { cgColor = color.cgColor }
        let clear = cgColor.copy(alpha: 0) ?? cgColor
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for layer in [content, pulse, base, solid] {
            layer.bounds = CGRect(origin: .zero, size: bounds.size)
            layer.position = CGPoint(x: bounds.midX, y: bounds.midY)
        }
        band.bounds = CGRect(
            x: 0, y: 0, width: bounds.width * Timeline.bandShare, height: bounds.height)
        band.position = CGPoint(x: -Timeline.bandShare * bounds.width, y: 0)
        base.backgroundColor = cgColor.copy(alpha: cgColor.alpha * Timeline.baseAlpha)
        solid.backgroundColor = cgColor
        band.colors = [clear, cgColor, cgColor, clear]
        CATransaction.commit()
    }

    /// The idle look: nothing shown, no band.
    private func resetLayers() {
        setModel(content, 0)
        setModel(pulse, 1)
        setModel(solid, 0)
        setBandVisible(false)
    }

    private func removeAnimations() {
        for layer in [content, pulse, band, solid] {
            layer.removeAllAnimations()
        }
    }

    private func addSweep(at now: CFTimeInterval) {
        let width = Double(bounds.width)
        guard width > 0 else { return }
        let sweep = CABasicAnimation(keyPath: "position.x")
        sweep.fromValue = -Timeline.bandShare * width
        sweep.toValue = width
        sweep.duration = Timeline.sweepPeriod
        sweep.repeatCount = .infinity
        sweep.timeOffset = timeline.sweepPhase(at: now)
        band.add(sweep, forKey: "sweep")
    }

    /// Animates `layer`'s opacity linearly to `value`, from `origin` or else
    /// from what is on screen.
    private func fade(
        _ layer: CALayer, from origin: Float? = nil, to value: Float, over duration: TimeInterval
    ) {
        let shown = origin ?? layer.presentation()?.opacity ?? layer.opacity
        layer.removeAnimation(forKey: "fadeOut")
        let animation = CABasicAnimation(keyPath: "opacity")
        animation.fromValue = shown
        animation.toValue = value
        animation.duration = duration
        layer.add(animation, forKey: "fade")
        setModel(layer, value)
    }

    private func setModel(_ layer: CALayer, _ opacity: Float) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        layer.opacity = opacity
        CATransaction.commit()
    }

    private func setBandVisible(_ visible: Bool) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        band.isHidden = !visible
        CATransaction.commit()
    }
}
#endif
