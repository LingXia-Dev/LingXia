import Foundation
#if os(iOS)
import UIKit
import QuartzCore
import CLingXiaRustAPI

/// A GPU page's view. Rust owns the page; this view reports its measurements
/// and shows the frames Rust presents.
@MainActor
final class GpuPageView: UIView {
    let token: Int64
    /// The Metal presenter is bound to this view's layer while it is on screen.
    private var attached = false
    /// Paused between requested frames.
    private var vsyncLink: CADisplayLink?
    private lazy var semantics = GpuAccessibility(view: self)
    lazy var ime = GpuIme(host: self)
    var inputReplies: [UInt64: (GpuFrameInputReply?) -> Void] = [:]
    var nextInputReply: UInt64 = 0
    /// UIKit's text input plumbing, kept for the connected editor.
    weak var inputDelegate: UITextInputDelegate?
    var textTokenizer: UITextInputTokenizer?
    /// The edit menu the core's text menu shows through, and the menu it shows.
    var editMenu: UIEditMenuInteraction?
    var textMenu: GpuFrameTextMenu?
    var menuChosen = false
    /// Static-text selection handles and the one a finger holds (end, grab offset).
    var handleLayer: CAShapeLayer?
    var handleGrab: (which: Int32, dx: CGFloat, dy: CGFloat)?
    /// The colours the page asks its host to paint around it; nil before its first frame.
    var hostChrome: GpuHostChrome? {
        didSet {
            layer.backgroundColor = hostChrome?.canvas.cgColor
            NotificationCenter.default.post(name: GpuHostChrome.changed, object: self)
        }
    }

    override var isAccessibilityElement: Bool {
        get { false }
        set {}
    }

    override var accessibilityElements: [Any]? {
        get { semantics.list }
        set {}
    }

    func accessibilityEvent(kind: UInt8, id: UInt32, text: String) {
        semantics.event(kind: kind, id: id, text: text)
    }

    override class var layerClass: AnyClass { CAMetalLayer.self }

    init(token: Int64) {
        self.token = token
        super.init(frame: .zero)
        isOpaque = true
        layer.contentsGravity = .topLeft
        let center = NotificationCenter.default
        // No GPU work in the background: iOS ends an app that submits it.
        center.addObserver(self, selector: #selector(suspend), name: UIApplication.didEnterBackgroundNotification, object: nil)
        center.addObserver(self, selector: #selector(resume), name: UIApplication.willEnterForegroundNotification, object: nil)
        center.addObserver(self, selector: #selector(resume), name: UIAccessibility.darkerSystemColorsStatusDidChangeNotification, object: nil)
        center.addObserver(self, selector: #selector(resume), name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
        center.addObserver(self, selector: #selector(resume), name: UIContentSizeCategory.didChangeNotification, object: nil)
        center.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        center.addObserver(self, selector: #selector(windowFocusChanged(_:)), name: UIApplication.didBecomeActiveNotification, object: nil)
        center.addObserver(self, selector: #selector(windowFocusChanged(_:)), name: UIApplication.willResignActiveNotification, object: nil)
    }

    @objc private func suspend() {
        guard attached else { return }
        attached = false
        gpuPageSuspend(token)
    }

    /// An inactive app (Control Center, an incoming call) ends the gesture in flight.
    @objc private func windowFocusChanged(_ note: Notification) {
        let active = note.name == UIApplication.didBecomeActiveNotification
        if !active { tracked = nil }
        gpuPageWindowFocus(token, active)
        ime.updateCaret()
    }

    @objc private func resume() {
        report()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    override func layoutSubviews() {
        super.layoutSubviews()
        report()
    }

    override func safeAreaInsetsDidChange() {
        super.safeAreaInsetsDidChange()
        report()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil {
            if let tracked {
                send(3, tracked)
                self.tracked = nil
            }
            cancelLongPress()
            stopFlingNow()
            // A requested frame still comes, without the display.
            let pending = vsyncLink.map { !$0.isPaused } ?? false
            vsyncLink?.invalidate()
            vsyncLink = nil
            suspend()
            if pending { gpuPageVsync(token) }
        }
        report()
        gpuPageSetVisible(token, window != nil && !isHidden)
    }

    override func traitCollectionDidChange(_ previous: UITraitCollection?) {
        super.traitCollectionDidChange(previous)
        report()
    }

    /// Dynamic Type as `[authored, shown]` pairs: every size through 256, then doubling sizes,
    /// so the curve holds wherever the system bends it.
    private static func textScale() -> RustVec<Float> {
        let metrics = UIFontMetrics(forTextStyle: .body)
        let points = RustVec<Float>()
        var sizes = Array(0...256)
        var large = 512
        while large <= 16384 { sizes.append(large); large *= 2 }
        for size in sizes {
            points.push(value: Float(size))
            points.push(value: Float(metrics.scaledValue(for: CGFloat(size))))
        }
        return points
    }

    /// Points of the keyboard over the bottom of this view.
    private var keyboardHeight: CGFloat = 0

    @objc private func keyboardChanged(_ note: Notification) {
        guard let window,
              let end = note.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect
        else { return }
        let frame = convert(window.convert(end, from: nil), from: window)
        keyboardHeight = max(0, bounds.maxY - max(frame.minY, bounds.minY))
        if frame.minY >= bounds.maxY { keyboardHeight = 0 }
        report()
    }

    /// Host chrome drawn over this view's edges: bars above it among its siblings that span
    /// most of its width and cross its top or bottom (an immersive tab bar, a clear title bar).
    private var chrome: (top: CGFloat, bottom: CGFloat) {
        if let raised { return raised.chrome }
        guard let parent = superview, let index = parent.subviews.firstIndex(of: self) else { return (0, 0) }
        var top: CGFloat = 0
        var bottom: CGFloat = 0
        for bar in parent.subviews[(index + 1)...] where !bar.isHidden && bar.alpha > 0.01 {
            let rect = convert(bar.frame, from: parent).intersection(bounds)
            guard !rect.isNull, rect.width >= bounds.width / 2, rect.height < bounds.height / 2 else { continue }
            if rect.minY <= bounds.minY { top = max(top, rect.maxY - bounds.minY) }
            if rect.maxY >= bounds.maxY { bottom = max(bottom, bounds.maxY - rect.minY) }
        }
        return (top, bottom)
    }

    /// Set while a top-layer dialog lifts this view above the bars: where it sat, and the
    /// chrome the page keeps padding by meanwhile.
    private var raised: (index: Int, chrome: (top: CGFloat, bottom: CGFloat))?

    func topLayer(_ modal: Bool) {
        guard let parent = superview else { return }
        if modal, raised == nil, let index = parent.subviews.firstIndex(of: self) {
            raised = (index, chrome)
            parent.bringSubviewToFront(self)
        } else if !modal, let index = raised?.index {
            raised = nil
            parent.insertSubview(self, at: min(index, parent.subviews.count - 1))
            report()
        }
    }

    /// The host changed its bars; the page re-measures what covers it.
    func chromeChanged() {
        report()
    }

    private func report() {
        guard let window, bounds.width > 0, bounds.height > 0 else { return }
        let scale = window.screen.scale
        let insets = safeAreaInsets
        let shorter = min(window.bounds.width, window.bounds.height)
        gpuPageMeasured(
            token,
            UInt32((bounds.width * scale).rounded()),
            UInt32((bounds.height * scale).rounded()),
            Float(scale),
            traitCollection.userInterfaceStyle == .dark,
            Float(insets.top * scale),
            Float(insets.right * scale),
            Float(insets.bottom * scale),
            Float(insets.left * scale),
            Float(chrome.top * scale),
            Float(chrome.bottom * scale),
            Float(keyboardHeight * scale),
            Float(shorter),
            Locale.preferredLanguages.first ?? "en",
            UIAccessibility.isReduceMotionEnabled,
            UIAccessibility.isDarkerSystemColorsEnabled,
            Self.textScale(),
            GpuPageView.hourCycle(),
            UInt8(Calendar.current.firstWeekday)
        )
        layer.contentsScale = scale
        if !attached, UIApplication.shared.applicationState != .background {
            attached = true
            gpuPageAttach(
                token,
                UInt(bitPattern: Unmanaged.passUnretained(self).toOpaque()),
                UInt32((bounds.width * scale).rounded()),
                UInt32((bounds.height * scale).rounded())
            )
        }
    }

    /// Calls `gpuPageVsync` on the next display refresh.
    func requestVsync() -> Bool {
        guard window != nil else { return false }
        if vsyncLink == nil {
            let link = CADisplayLink(target: GpuVsyncTarget(self), selector: #selector(GpuVsyncTarget.tick))
            link.add(to: .main, forMode: .common)
            vsyncLink = link
        }
        vsyncLink?.isPaused = false
        return true
    }

    // MARK: Touch

    /// The touch the page follows; a second finger cancels the gesture, as on Android.
    private weak var tracked: UITouch?
    /// Recent positions (device px) with their timestamps, for the release velocity.
    private var samples: [(time: TimeInterval, x: CGFloat, y: CGFloat)] = []
    private var longPress: Timer?
    private var flingLink: CADisplayLink?
    private var fling: (region: String, horizontal: Bool, velocity: Double, start: CFTimeInterval, travelled: Double)?

    /// UIKit's pan hysteresis and the slowest release that still flings, in points.
    private static let slopPoints: CGFloat = 10
    private static let minFlingPoints: CGFloat = 50

    private func pixels(_ touch: UITouch) -> (CGFloat, CGFloat) {
        let point = touch.location(in: self)
        let scale = window?.screen.scale ?? UIScreen.main.scale
        return (point.x * scale, point.y * scale)
    }

    @discardableResult
    private func send(_ phase: UInt8, _ touch: UITouch, velocity: (CGFloat, CGFloat) = (0, 0)) -> Bool {
        let (x, y) = pixels(touch)
        let scale = window?.screen.scale ?? UIScreen.main.scale
        return gpuPagePointer(
            token, phase, Float(x), Float(y), Float(Self.slopPoints * scale),
            Float(velocity.0), Float(velocity.1), Float(Self.minFlingPoints * scale))
    }

    private func record(_ touch: UITouch) {
        let (x, y) = pixels(touch)
        samples.append((touch.timestamp, x, y))
        // The last 100 ms decide the release velocity.
        samples.removeAll { touch.timestamp - $0.time > 0.1 }
    }

    private func releaseVelocity() -> (CGFloat, CGFloat) {
        guard let first = samples.first, let last = samples.last, last.time > first.time else { return (0, 0) }
        let dt = CGFloat(last.time - first.time)
        return ((last.x - first.x) / dt, (last.y - first.y) / dt)
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard tracked == nil, let touch = touches.first else {
            if let tracked {
                send(3, tracked)
                self.tracked = nil
            }
            return
        }
        if gpuPageInputBlocked(token) { return }
        // A hardware keyboard's keys reach the page once it is touched.
        if Self.hardwareKeyboard, !isFirstResponder { becomeFirstResponder() }
        if handleTouch(0, at: touch.location(in: self)) {
            tracked = touch
            return
        }
        stopFlingNow()
        tracked = touch
        samples = []
        record(touch)
        send(0, touch)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let tracked, touches.contains(tracked) else { return }
        if handleTouch(1, at: tracked.location(in: self)) { return }
        record(tracked)
        send(1, tracked)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let tracked, touches.contains(tracked) else { return }
        self.tracked = nil
        if handleTouch(2, at: tracked.location(in: self)) { return }
        record(tracked)
        send(2, tracked, velocity: releaseVelocity())
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let tracked, touches.contains(tracked) else { return }
        self.tracked = nil
        if handleTouch(3, at: tracked.location(in: self)) { return }
        send(3, tracked)
    }

    func armLongPress() {
        longPress?.invalidate()
        // UIKit's long-press duration.
        longPress = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: false) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.longPress = nil
                gpuPageLongPress(self.token)
            }
        }
    }

    func cancelLongPress() {
        longPress?.invalidate()
        longPress = nil
    }

    /// UIScrollView's deceleration: velocity falls by this factor every millisecond.
    private static let deceleration = Double(UIScrollView.DecelerationRate.normal.rawValue)

    func startFling(region: String, horizontal: Bool, velocity: Float) {
        stopFlingNow()
        fling = (region, horizontal, Double(velocity), CACurrentMediaTime(), 0)
        let link = CADisplayLink(target: GpuFlingTarget(self), selector: #selector(GpuFlingTarget.tick))
        link.add(to: .main, forMode: .common)
        flingLink = link
    }

    func stopFlingNow() {
        flingLink?.invalidate()
        flingLink = nil
        fling = nil
    }

    fileprivate func flingFrame() {
        guard var current = fling else { return stopFlingNow() }
        let ms = (CACurrentMediaTime() - current.start) * 1000
        let rate = Self.deceleration
        // Distance after `ms` of exponential decay from the release velocity (px/s).
        let distance = current.velocity / 1000 * (pow(rate, ms) - 1) / log(rate)
        let speed = abs(current.velocity * pow(rate, ms))
        let step = distance - current.travelled
        current.travelled = distance
        fling = current
        let moved = gpuPageScrollBy(token, current.region, Float(step))
        if !moved || speed < 20 {
            stopFlingNow()
        }
    }

    fileprivate func vsync() {
        vsyncLink?.isPaused = true
        gpuPageVsync(token)
    }

    // MARK: Feedback

    /// Press and focus-ring layers over the frame, made on first use.
    private var highlights: [GpuHighlightLayer] = []

    func feedback(ring: Bool, rects: UnsafeBufferPointer<Float>, argb: UInt32, width: Float) {
        if highlights.isEmpty {
            if rects.isEmpty { return }
            highlights = [GpuHighlightLayer(), GpuHighlightLayer()]
            highlights.forEach { layer.addSublayer($0) }
        }
        let highlight = highlights[ring ? 1 : 0]
        highlight.frame = bounds
        highlight.contentsScale = window?.screen.scale ?? UIScreen.main.scale
        highlight.update(rects: rects, argb: argb, ringWidth: width)
    }

    func show(pixels: UnsafeRawPointer, length: Int, width: Int, height: Int) -> Bool {
        guard width > 0, height > 0, length == width * height * 4,
              let provider = CGDataProvider(data: Data(bytes: pixels, count: length) as CFData),
              let image = CGImage(
                  width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
                  bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                  provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
        else { return false }
        layer.contentsScale = window?.screen.scale ?? UIScreen.main.scale
        layer.contents = image
        return true
    }
}

/// Colours for the host chrome around a GPU page, chosen by the core from its palette.
struct GpuHostChrome {
    /// Posted on the main thread with the page view as object.
    static let changed = Notification.Name("LingXiaGpuHostChromeChanged")
    let canvas: UIColor
    let bar: UIColor
    let barLine: UIColor
    let content: UIColor
    let contentMuted: UIColor
    let accent: UIColor
    let dark: Bool
}

/// A fling's display link target that does not retain the view.
@MainActor
private final class GpuFlingTarget: NSObject {
    weak var view: GpuPageView?
    init(_ view: GpuPageView) { self.view = view }
    @objc func tick() { view?.flingFrame() }
}

/// A display link retains its target; this keeps it from retaining the view.
@MainActor
private final class GpuVsyncTarget: NSObject {
    weak var view: GpuPageView?
    init(_ view: GpuPageView) { self.view = view }
    @objc func tick() { view?.vsync() }
}

/// GPU page views by token; Rust calls these on the main thread.
public enum GpuPages {
    @MainActor private static var views: [Int64: GpuPageView] = [:]

    /// The view for a page token, created on first use.
    @MainActor static func view(for token: Int64) -> GpuPageView {
        if let view = views[token] { return view }
        let view = GpuPageView(token: token)
        views[token] = view
        return view
    }

    static func presentPixels(token: Int64, pixels: UInt, length: UInt, width: UInt32, height: UInt32) -> Bool {
        MainActor.assumeIsolated {
            guard let view = views[token], let pointer = UnsafeRawPointer(bitPattern: pixels) else {
                return false
            }
            return view.show(pixels: pointer, length: Int(length), width: Int(width), height: Int(height))
        }
    }

    static func armLongPress(token: Int64) {
        MainActor.assumeIsolated { views[token]?.armLongPress() }
    }

    static func cancelLongPress(token: Int64) {
        MainActor.assumeIsolated { views[token]?.cancelLongPress() }
    }

    static func fling(token: Int64, region: RustStr, horizontal: Bool, velocity: Float) {
        let region = region.toString()
        MainActor.assumeIsolated { views[token]?.startFling(region: region, horizontal: horizontal, velocity: velocity) }
    }

    static func accessibilityEvent(token: Int64, kind: UInt8, id: UInt32, text: RustStr) {
        let text = text.toString()
        MainActor.assumeIsolated { views[token]?.accessibilityEvent(kind: kind, id: id, text: text) }
    }

    static func feedback(token: Int64, ring: Bool, shape: UInt, count: UInt, color: UInt32, width: Float) {
        MainActor.assumeIsolated {
            let rects = UnsafeBufferPointer(start: UnsafePointer<Float>(bitPattern: shape), count: Int(count))
            views[token]?.feedback(ring: ring, rects: rects, argb: color, width: width)
        }
    }

    static func topLayer(token: Int64, modal: Bool) {
        MainActor.assumeIsolated { views[token]?.topLayer(modal) }
    }

    static func haptic(token: Int64) {
        MainActor.assumeIsolated { UIImpactFeedbackGenerator(style: .medium).impactOccurred() }
    }

    static func copyText(text: RustStr) {
        let text = text.toString()
        MainActor.assumeIsolated { UIPasteboard.general.string = text }
    }

    static func imePresented(token: Int64, generation: UInt64, node: RustStr) {
        let node = node.toString()
        MainActor.assumeIsolated {
            guard let view = views[token] else { return }
            view.ime.presented(GpuPageView.decodeNode(node), ticket: view.ime.capture())
            view.updateSelectionHandles()
        }
    }

    static func imeFocus(token: Int64, generation: UInt64, node: RustStr, touch: Bool, x: Float, y: Float, word: Bool) {
        let node = node.toString()
        MainActor.assumeIsolated {
            guard let view = views[token], let node = GpuPageView.decodeNode(node) else { return }
            if touch {
                view.ime.touch(node, generation: generation, x: x, y: y, word: word)
            } else {
                view.ime.focus(node, generation: generation)
            }
        }
    }

    static func imeActivation(token: Int64) {
        MainActor.assumeIsolated {
            if let ime = views[token]?.ime, ime.isActive { ime.clear(blur: true) }
        }
    }
    static func textMenu(token: Int64, menu: RustStr) {
        let menu = menu.toString()
        MainActor.assumeIsolated { views[token]?.showTextMenu(menu) }
    }
    static func hostChrome(token: Int64, canvas: UInt32, bar: UInt32, bar_line barLine: UInt32, content: UInt32, content_muted contentMuted: UInt32, accent: UInt32, dark: Bool) {
        MainActor.assumeIsolated {
            guard let view = views[token] else { return }
            let color = { (argb: UInt32) in
                UIColor(red: CGFloat((argb >> 16) & 0xff) / 255, green: CGFloat((argb >> 8) & 0xff) / 255,
                        blue: CGFloat(argb & 0xff) / 255, alpha: CGFloat(argb >> 24) / 255)
            }
            view.hostChrome = GpuHostChrome(
                canvas: color(canvas), bar: color(bar), barLine: color(barLine), content: color(content),
                contentMuted: color(contentMuted), accent: color(accent), dark: dark)
        }
    }
    static func imeReset(token: Int64) {
        MainActor.assumeIsolated { views[token]?.ime.reset() }
    }

    static func imeEdit(token: Int64, generation: UInt64, node: RustStr, replace: Bool, text: RustStr, start: Int32, end: Int32) -> UInt8 {
        let (node, text) = (node.toString(), text.toString())
        return MainActor.assumeIsolated {
            guard let view = views[token], let node = GpuPageView.decodeNode(node) else { return GpuIme.failed }
            return view.ime.edit(node, generation: generation, text: replace ? text : nil, start: Int(start), end: Int(end))
        }
    }

    static func inputReply(token: Int64, reply: UInt64, present: Bool, value: RustStr) {
        let value = value.toString()
        MainActor.assumeIsolated { views[token]?.inputReply(reply, value: present ? value : nil) }
    }

    static func stopFling(token: Int64) {
        MainActor.assumeIsolated { views[token]?.stopFlingNow() }
    }

    static func requestVsync(token: Int64) -> Bool {
        guard Thread.isMainThread else { return false }
        return MainActor.assumeIsolated { views[token]?.requestVsync() ?? false }
    }

    static func dispose(token: Int64) {
        MainActor.assumeIsolated {
            views.removeValue(forKey: token)?.removeFromSuperview()
        }
    }

    /// Whether `window` shows a GPU page, whose Metal frames layer rendering cannot see.
    @MainActor static func shows(in window: UIWindow) -> Bool {
        views.values.contains { $0.window === window && !$0.isHidden }
    }

    /// The GPU page token of a page of the app, or 0 when it is not a GPU page.
    static func token(appId: String, sessionId: UInt64, pageInstanceId: String?) -> Int64 {
        pageBinding(appId, sessionId, pageInstanceId ?? "").gpu_token
    }
}
#endif
