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
    /// Recovery chrome after a failure, and whether it hides the page entirely.
    var recoveryPanel: UIView?
    var recoveryCovers = false
    /// Set while the page declares `enablePullDownRefresh`.
    var pullToRefresh: GpuPullToRefresh? {
        didSet { if oldValue !== pullToRefresh { oldValue?.detach() } }
    }
    /// The touch now belongs to a pull; the core had it cancelled.
    private var pulling = false
    var menuChosen = false
    /// Static-text selection handles and the one a finger holds (end, grab offset).
    var handleLayer: CAShapeLayer?
    var handleGrab: (which: Int32, dx: CGFloat, dy: CGFloat)?
    let webContainer: GpuWebViewContainer
    let metalView = GpuMetalSurfaceView()
    /// The colours the page asks its host to paint around it; nil before its first frame.
    var hostChrome: GpuHostChrome? {
        didSet {
            layer.backgroundColor = hostChrome?.canvas.cgColor
            NotificationCenter.default.post(name: GpuHostChrome.changed, object: self)
            // The bars take the page's colours: the snapshots under a dialog follow.
            recaptureBars()
        }
    }

    override var isAccessibilityElement: Bool {
        get { false }
        set {}
    }

    override var accessibilityElements: [Any]? {
        get {
            let base = semantics.list + webContainer.accessibleWebViews
            guard let panel = recoveryPanel else { return base }
            return recoveryCovers ? [panel] : base + [panel]
        }
        set {}
    }

    func accessibilityEvent(kind: UInt8, id: UInt32, text: String) {
        semantics.event(kind: kind, id: id, text: text)
    }

    init(token: Int64) {
        self.token = token
        self.webContainer = GpuWebViewContainer(token: token)
        super.init(frame: .zero)
        isOpaque = true
        addSubview(webContainer)
        addSubview(metalView)
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
        center.addObserver(self, selector: #selector(barChanged(_:)), name: GpuHostChrome.barChanged, object: nil)
    }

    @objc private func suspend() {
        guard attached else { return }
        attached = false
        gpuPageSuspend(token)
    }

    /// Off screen and not kept: drop the layer's last frame and drawables; the next attach
    /// sizes the layer again, and the page background shows until its first frame.
    func releaseSurface() {
        guard window == nil, let layer = metalView.layer as? CAMetalLayer else { return }
        suspend()
        layer.contents = nil
        layer.drawableSize = CGSize(width: 1, height: 1)
    }

    /// An inactive app (Control Center, an incoming call) ends the gesture in flight.
    @objc private func windowFocusChanged(_ note: Notification) {
        let active = note.name == UIApplication.didBecomeActiveNotification
        if !active { tracked = nil; automated = nil }
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
        webContainer.frame = bounds
        metalView.frame = bounds
        report()
        if raised != nil, bounds.size != capturedSize { recaptureBars() }
    }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard !isHidden, isUserInteractionEnabled, alpha > 0.01, self.point(inside: point, with: event) else {
            return nil
        }
        if let panel = recoveryPanel, !panel.isHidden,
           let hit = panel.hitTest(convert(point, to: panel), with: event) {
            return hit
        }
        if webContainer.hasActiveMounts, !gpuPageInputBlocked(token) {
            let webId = gpuPageWebViewAt(token, Float(point.x), Float(point.y)).toString()
            if !webId.isEmpty, let wv = webContainer.webView(for: webId) {
                let local = convert(point, to: wv)
                return wv.hitTest(local, with: event) ?? wv
            }
        }
        return self
    }

    func syncWebViews(_ json: String) {
        let hasWeb = webContainer.sync(json: json)
        let opaque = !hasWeb
        if metalView.isOpaque != opaque {
            metalView.isOpaque = opaque
            (metalView.layer as? CAMetalLayer)?.isOpaque = opaque
            metalView.backgroundColor = opaque ? nil : .clear
        }
    }

    func closeRequest() -> Bool {
        gpuPageCloseRequest(token)
    }

    func captureCompositedPng() -> Data? {
        guard webContainer.hasActiveMounts, bounds.width > 1, bounds.height > 1 else {
            return nil
        }
        let format = UIGraphicsImageRendererFormat()
        format.opaque = true
        format.scale = window?.screen.scale ?? UIScreen.main.scale
        let image = UIGraphicsImageRenderer(bounds: bounds, format: format).image { _ in
            drawHierarchy(in: bounds, afterScreenUpdates: false)
        }
        return image.pngData()
    }

    override func willMove(toSuperview newSuperview: UIView?) {
        super.willMove(toSuperview: newSuperview)
        // Removed with its dialog open: the bars it hid must not stay hidden for the next page.
        guard newSuperview !== superview, let raised else { return }
        self.raised = nil
        raiseFallback?.cancel()
        raiseFallback = nil
        barSnapshots.removeAll()
        raised.silenced.forEach { $0.accessibilityElementsHidden = false }
    }

    override func safeAreaInsetsDidChange() {
        super.safeAreaInsetsDidChange()
        report()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        // A page that leaves mid-gesture must not keep the back swipe switched off.
        if window == nil { claimGesture(false) }
        if window == nil {
            if let tracked {
                send(3, tracked)
                self.tracked = nil
            }
            if let automated {
                send(3, at: automated)
                self.automated = nil
            }
            cancelLongPress()
            stopFlingNow()
            pulling = false
            pullToRefresh?.detach()
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
        if traitCollection.userInterfaceStyle != previous?.userInterfaceStyle { recaptureBars() }
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
    /// `silenced` are the covered views this page hid from VoiceOver.
    private var raised: (index: Int, chrome: (top: CGFloat, bottom: CGFloat), covered: [UIView], silenced: [UIView])?

    /// Recognizers standing down while the View holds a gesture, and where it began on screen.
    private var heldOff: [UIGestureRecognizer] = []
    private var touchStart: CGPoint?

    /// The View took the gesture under the finger (a drag, a scroll, a slider) or let it go.
    /// Enclosing scroll views and the back swipe stand down meanwhile; a touch that began at
    /// a screen edge keeps the back swipe, which owns that edge.
    func claimGesture(_ exclusive: Bool) {
        guard exclusive else {
            heldOff.forEach { $0.isEnabled = true }
            heldOff = []
            return
        }
        guard heldOff.isEmpty, let window else { return }
        let edge: CGFloat = 24
        let atEdge = touchStart.map { $0.x < edge || $0.x > window.bounds.width - edge } ?? false
        var recognizers: [UIGestureRecognizer] = []
        var ancestor = superview
        while let view = ancestor {
            if let scroll = view as? UIScrollView { recognizers.append(scroll.panGestureRecognizer) }
            if !atEdge {
                recognizers += (view.gestureRecognizers ?? []).filter { $0 is UIScreenEdgePanGestureRecognizer }
            }
            ancestor = view.superview
        }
        // One already tracking keeps the gesture; disabling it now would cancel the user's.
        heldOff = recognizers.filter { $0.isEnabled && $0.state == .possible }
        heldOff.forEach { $0.isEnabled = false }
    }

    func topLayer(_ modal: Bool) {
        guard let parent = superview else { return }
        if modal, raised == nil, let index = parent.subviews.firstIndex(of: self) {
            // The bars it covers leave VoiceOver too, as under any modal.
            let covered = Array(parent.subviews[(index + 1)...])
            let silenced = covered.filter { !$0.accessibilityElementsHidden }
            silenced.forEach { $0.accessibilityElementsHidden = true }
            raised = (index, chrome, covered, silenced)
            // The page comes over the bars once a frame draws them under its dialog, so no
            // frame shows neither; a page that never does is raised anyway.
            if captureBars(afterUpdates: false) {
                let fallback = DispatchWorkItem { [weak self] in self?.raise() }
                raiseFallback = fallback
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.25, execute: fallback)
            } else {
                raise()
            }
        } else if !modal, let raised {
            raiseFallback?.cancel()
            raiseFallback = nil
            if !barSnapshots.isEmpty {
                barSnapshots.removeAll()
                gpuPageDeclineChromeSnapshot(token)
            }
            self.raised = nil
            raised.silenced.forEach { $0.accessibilityElementsHidden = false }
            parent.insertSubview(self, at: min(raised.index, parent.subviews.count - 1))
            report()
        }
        UIAccessibility.post(notification: .screenChanged, argument: nil)
    }

    /// The host changed its bars; the page re-measures what covers it.
    func chromeChanged() {
        report()
        recaptureBars()
    }

    /// The generation of the bar snapshot handed to the page, by edge (0 top, 1 bottom).
    private var barSnapshots: [UInt8: UInt64] = [:]
    /// Raises the page when no frame shows its bar snapshots in time.
    private var raiseFallback: DispatchWorkItem?
    private var recapturePending = false
    /// The size the bars were captured at.
    private var capturedSize: CGSize = .zero

    private func raise() {
        raiseFallback?.cancel()
        raiseFallback = nil
        guard raised != nil, let parent = superview, parent.subviews.last !== self else { return }
        parent.bringSubviewToFront(self)
    }

    /// The presented frame shows the bar snapshots up to `generation`; 0 when it shows none.
    func chromeSnapshot(_ generation: UInt64) {
        guard let raised, !barSnapshots.isEmpty else { return }
        if generation == 0 {
            // The page dropped them (its surface changed): the real bars show until it has new ones.
            barSnapshots.removeAll()
            if let parent = superview { parent.insertSubview(self, at: min(raised.index, parent.subviews.count - 1)) }
            recaptureBars()
        } else if let newest = barSnapshots.values.max(), generation >= newest {
            raise()
        }
    }

    @objc private func barChanged(_ note: Notification) {
        guard let changed = note.object as? UIView,
              raised?.covered.contains(where: { changed.isDescendant(of: $0) }) == true
        else { return }
        recaptureBars()
    }

    /// A bar under the dialog changed (selection, badge, appearance, size): hand the page a
    /// fresh copy once the change has drawn.
    private func recaptureBars() {
        guard raised != nil, !recapturePending else { return }
        recapturePending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.recapturePending = false
            guard self.raised != nil, self.captureBars(afterUpdates: true) else { return }
            // Lowered after the page dropped the old ones: it rises again on the new frame.
            if self.raiseFallback == nil, self.superview?.subviews.last !== self {
                let fallback = DispatchWorkItem { [weak self] in self?.raise() }
                self.raiseFallback = fallback
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.25, execute: fallback)
            }
        }
    }

    /// Copies each covered bar on an edge into the page, to draw under its top layer.
    @discardableResult
    private func captureBars(afterUpdates: Bool) -> Bool {
        guard let raised, let window else { return false }
        let scale = window.screen.scale
        capturedSize = bounds.size
        var handed = false
        for bar in raised.covered where !bar.isHidden && bar.alpha > 0.01 {
            let full = convert(bar.bounds, from: bar)
            let rect = full.intersection(bounds)
            guard !rect.isNull, rect.width >= bounds.width / 2, rect.height < bounds.height / 2 else { continue }
            let edge: UInt8
            if rect.minY <= bounds.minY { edge = 0 } else if rect.maxY >= bounds.maxY { edge = 1 } else { continue }
            guard let image = Self.pixels(of: bar, scale: scale, afterUpdates: afterUpdates) else { continue }
            let generation = image.data.withUnsafeBytes { raw in
                gpuPageChromeSnapshot(
                    token, edge, UInt(bitPattern: raw.baseAddress), UInt(raw.count),
                    UInt32(image.width), UInt32(image.height),
                    Float((full.minX - bounds.minX) * scale), Float((full.minY - bounds.minY) * scale),
                    Float(full.width * scale), Float(full.height * scale))
            }
            guard generation > 0 else { continue }
            barSnapshots[edge] = generation
            handed = true
        }
        return handed
    }

    /// `bar` drawn as premultiplied RGBA8 rows, top first, at `scale`.
    private static func pixels(of bar: UIView, scale: CGFloat, afterUpdates: Bool) -> (data: Data, width: Int, height: Int)? {
        let width = Int((bar.bounds.width * scale).rounded())
        let height = Int((bar.bounds.height * scale).rounded())
        guard width > 0, height > 0, let space = CGColorSpace(name: CGColorSpace.sRGB) else { return nil }
        var data = Data(count: width * height * 4)
        let drawn = data.withUnsafeMutableBytes { raw -> Bool in
            guard let context = CGContext(
                data: raw.baseAddress, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width * 4, space: space,
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
            else { return false }
            // UIKit draws top-down.
            context.translateBy(x: 0, y: CGFloat(height))
            context.scaleBy(x: scale, y: -scale)
            UIGraphicsPushContext(context)
            defer { UIGraphicsPopContext() }
            return bar.drawHierarchy(in: bar.bounds, afterScreenUpdates: afterUpdates)
        }
        return drawn ? (data, width, height) : nil
    }

    /// The bars' cover over the top edge, for the pull's strip.
    var chromeTop: CGFloat { chrome.top }

    /// The pull put the page back where it is laid out.
    func pullReleased() {
        report()
    }

    private func report() {
        // A pull moves the page down; it keeps the measurements of where it is laid out.
        if pullToRefresh?.holdsPage == true { return }
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
        metalView.layer.contentsScale = scale
        if !attached, UIApplication.shared.applicationState != .background {
            attached = true
            gpuPageAttach(
                token,
                UInt(bitPattern: Unmanaged.passUnretained(metalView).toOpaque()),
                UInt32((bounds.width * scale).rounded()),
                UInt32((bounds.height * scale).rounded())
            )
        }
    }

    /// Calls `gpuPageVsync` on the next display refresh.
    func requestVsync() -> Bool {
        guard window != nil else { return false }
        if vsyncLink == nil {
            let link = CADisplayLink(target: GpuVsyncTarget(self), selector: #selector(GpuVsyncTarget.tick(_:)))
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

    private func pixels(_ point: CGPoint) -> (CGFloat, CGFloat) {
        let scale = window?.screen.scale ?? UIScreen.main.scale
        return (point.x * scale, point.y * scale)
    }

    @discardableResult
    private func send(_ phase: UInt8, _ touch: UITouch, velocity: (CGFloat, CGFloat) = (0, 0)) -> Bool {
        gpuPageInputAge(token, gpuInputAge(touch.timestamp))
        return send(phase, at: touch.location(in: self), velocity: velocity)
    }

    @discardableResult
    private func send(_ phase: UInt8, at point: CGPoint, velocity: (CGFloat, CGFloat) = (0, 0)) -> Bool {
        let scale = window?.screen.scale ?? UIScreen.main.scale
        return gpuPagePointer(
            token, phase, Float(point.x * scale), Float(point.y * scale), Float(Self.slopPoints * scale),
            Float(velocity.0), Float(velocity.1), Float(Self.minFlingPoints * scale))
    }

    private func record(_ touch: UITouch) {
        record(touch.location(in: self), at: touch.timestamp)
    }

    private func record(_ point: CGPoint, at time: TimeInterval) {
        let (x, y) = pixels(point)
        samples.append((time, x, y))
        // The last 100 ms decide the release velocity.
        samples.removeAll { time - $0.time > 0.1 }
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
        touchStart = touch.location(in: window)
        // A hardware keyboard's keys reach the page once it is touched.
        if Self.hardwareKeyboard, !isFirstResponder { becomeFirstResponder() }
        if handleTouch(0, at: touch.location(in: self)) {
            tracked = touch
            return
        }
        stopFlingNow()
        tracked = touch
        samples = []
        pulling = false
        pullToRefresh?.began(touch.location(in: nil))
        record(touch)
        send(0, touch)
    }

    /// Whether a move belongs to a pull to refresh; on its first, the core lets the touch go.
    private func pulls(at point: CGPoint, from local: CGPoint) -> Bool {
        guard let refresh = pullToRefresh else { return false }
        let pulls = refresh.moved(point) { [token] in
            let scale = self.window?.screen.scale ?? UIScreen.main.scale
            return !gpuPageCanScrollVertically(token, Float(local.x * scale), Float(local.y * scale), -1)
        }
        if pulls, !pulling {
            pulling = true
            cancelLongPress()
            send(3, at: local)
        }
        return pulls
    }

    /// The touch ended (`up`) or was cancelled: true when it ended a pull the core no longer has.
    private func endsPull(up: Bool) -> Bool {
        let ended = pullToRefresh?.ended(up: up) ?? false
        defer { pulling = false }
        return ended || pulling
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let tracked, touches.contains(tracked) else { return }
        if handleTouch(1, at: tracked.location(in: self)) { return }
        if pulls(at: tracked.location(in: nil), from: tracked.location(in: self)) { return }
        record(tracked)
        send(1, tracked)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let tracked, touches.contains(tracked) else { return }
        self.tracked = nil
        if handleTouch(2, at: tracked.location(in: self)) { return }
        if endsPull(up: true) { return }
        record(tracked)
        send(2, tracked, velocity: releaseVelocity())
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let tracked, touches.contains(tracked) else { return }
        self.tracked = nil
        if handleTouch(3, at: tracked.location(in: self)) { return }
        if endsPull(up: false) { return }
        send(3, tracked)
    }

    /// Where an automated finger rests; automation has no `UITouch` to follow.
    private var automated: CGPoint?

    /// A touch phase from automation (0 down, 1 move, 2 up, 3 cancel) at a view point. It
    /// takes the path a finger's touches take once UIKit has delivered them here; false
    /// when the page takes no input now.
    @objc func automationTouch(_ phase: Int, x: Double, y: Double) -> Bool {
        let point = CGPoint(x: x, y: y)
        let now = CACurrentMediaTime()
        switch phase {
        case 0:
            if let tracked {
                send(3, tracked)
                self.tracked = nil
            }
            if gpuPageInputBlocked(token) { return false }
            touchStart = convert(point, to: nil)
            automated = point
            if handleTouch(0, at: point) { return true }
            stopFlingNow()
            samples = []
            pulling = false
            pullToRefresh?.began(point)
            record(point, at: now)
            send(0, at: point)
            return true
        case 1:
            guard automated != nil else { return false }
            automated = point
            if handleTouch(1, at: point) { return true }
            // Automation's points do not move with the page, as a finger on the glass does not.
            if pulls(at: point, from: point) { return true }
            record(point, at: now)
            send(1, at: point)
            return true
        default:
            guard automated != nil else { return false }
            automated = nil
            let up = phase == 2
            if handleTouch(up ? 2 : 3, at: point) { return true }
            if endsPull(up: up) { return true }
            if up {
                record(point, at: now)
                send(2, at: point, velocity: releaseVelocity())
            } else {
                send(3, at: point)
            }
            return true
        }
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

    fileprivate func vsync(_ link: CADisplayLink) {
        vsyncLink?.isPaused = true
        let (presentIn, interval) = gpuDisplayTiming(link)
        gpuPageVsyncAt(token, presentIn, interval)
    }

    // MARK: Feedback

    /// Press and focus-ring layers over the frame, made on first use.
    private var highlights: [GpuHighlightLayer] = []

    func feedback(ring: Bool, rects: UnsafeBufferPointer<Float>, argb: UInt32, width: Float) {
        if highlights.isEmpty {
            if rects.isEmpty { return }
            highlights = [GpuHighlightLayer(), GpuHighlightLayer()]
            highlights.forEach { metalView.layer.addSublayer($0) }
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
        metalView.layer.contentsScale = window?.screen.scale ?? UIScreen.main.scale
        metalView.layer.contents = image
        return true
    }
}

/// Colours for the host chrome around a GPU page, chosen by the core from its palette.
struct GpuHostChrome {
    /// Posted on the main thread with the page view as object.
    static let changed = Notification.Name("LingXiaGpuHostChromeChanged")
    /// A host bar posts it, as object, after it changed what it draws.
    static let barChanged = Notification.Name("LingXiaHostBarChanged")
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
    @objc func tick(_ link: CADisplayLink) { view?.vsync(link) }
}

/// The refresh a display link frame shows on, from now, and the refresh interval, in µs.
/// `targetTimestamp` shares `CACurrentMediaTime`'s clock.
func gpuDisplayTiming(_ link: CADisplayLink) -> (Int64, UInt64) {
    let presentIn = (link.targetTimestamp - CACurrentMediaTime()) * 1_000_000
    let interval = max(0, link.targetTimestamp - link.timestamp) * 1_000_000
    return (Int64(presentIn.rounded()), UInt64(interval.rounded()))
}

/// How long ago an event stamped on the system uptime clock happened, in µs.
func gpuInputAge(_ timestamp: TimeInterval) -> UInt64 {
    UInt64(max(0, CACurrentMediaTime() - timestamp) * 1_000_000)
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

    static func chromeSnapshot(token: Int64, generation: UInt64) {
        MainActor.assumeIsolated { views[token]?.chromeSnapshot(generation) }
    }

    static func recovery(token: Int64, message: Int32, action: Int32, covers: Bool) {
        MainActor.assumeIsolated { views[token]?.showRecovery(message: message, action: action, covers: covers) }
    }

    static func claim(token: Int64, exclusive: Bool) {
        MainActor.assumeIsolated { views[token]?.claimGesture(exclusive) }
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

    static func releaseSurface(token: Int64) {
        MainActor.assumeIsolated { views[token]?.releaseSurface() }
    }

    static func syncWebViews(token: Int64, mounts: RustStr) {
        let json = mounts.toString()
        MainActor.assumeIsolated { views[token]?.syncWebViews(json) }
    }

    static func captureCompositedPng(token: Int64) -> RustVec<UInt8> {
        let out = RustVec<UInt8>()
        let png: Data? = MainActor.assumeIsolated {
            views[token]?.captureCompositedPng()
        }
        if let data = png {
            for byte in data {
                out.push(value: byte)
            }
        }
        return out
    }

    static func dispose(token: Int64) {
        MainActor.assumeIsolated {
            guard let view = views.removeValue(forKey: token) else { return }
            view.webContainer.clearAll()
            view.pullToRefresh = nil
            view.removeFromSuperview()
        }
    }

    /// The GPU view showing the page `webtag` of `appId`.
    @MainActor static func view(appId: String, webtag: String) -> GpuPageView? {
        views.values.first {
            let info = gpuPageInfo($0.token)
            return info.app_id.toString() == appId && info.webtag.toString() == webtag
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
