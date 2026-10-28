import Foundation
#if os(macOS)
import AppKit
import QuartzCore
import CLingXiaRustAPI

/// A GPU page's view. Rust owns the page; this view reports its measurements
/// and shows the frames Rust presents.
@MainActor
final class GpuPageView: NSView {
    let token: Int64
    /// The Metal presenter is bound to this view's layer for the current window.
    private var attached = false
    private lazy var semantics = MacGpuAccessibility(view: self)
    lazy var ime = GpuIme(host: self)
    var inputReplies: [UInt64: (GpuFrameInputReply?) -> Void] = [:]
    var nextInputReply: UInt64 = 0
    /// The colours the page asks its host to paint around it; nil before its first frame.
    var chrome: GpuHostChrome? {
        didSet {
            layer?.backgroundColor = chrome?.canvas.cgColor
            NotificationCenter.default.post(name: GpuHostChrome.changed, object: self)
        }
    }
    /// The text menu on screen, closed when the core takes it away.
    var textMenu: NSMenu?
    /// The open date or time picker of the connected editor.
    var valuePicker: GpuValuePickerDelegate?

    override func isAccessibilityElement() -> Bool { false }

    override func accessibilityChildren() -> [Any]? { semantics.list }

    func accessibilityEvent(kind: UInt8, id: UInt32, text: String) {
        semantics.event(kind: kind, id: id, text: text)
    }
    /// Paused between requested frames; follows the display the view is on.
    private var vsyncLink: AnyObject?

    init(token: Int64) {
        self.token = token
        super.init(frame: .zero)
        wantsLayer = true
        layerContentsRedrawPolicy = .never
        layer?.contentsGravity = .topLeft
        layer?.isOpaque = true
        NSWorkspace.shared.notificationCenter.addObserver(
            self, selector: #selector(accessibilityChanged),
            name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
    }

    @objc private func accessibilityChanged() {
        report()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is unavailable") }

    override var isFlipped: Bool { true }

    override func makeBackingLayer() -> CALayer {
        let layer = CAMetalLayer()
        layer.isOpaque = true
        layer.maximumDrawableCount = 3
        return layer
    }

    override func layout() {
        super.layout()
        report()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        NotificationCenter.default.removeObserver(self)
        // A requested frame still comes, on the new window's display or none.
        var pending = false
        if #available(macOS 14.0, *), let link = vsyncLink as? CADisplayLink {
            pending = !link.isPaused
            link.invalidate()
        }
        vsyncLink = nil
        if pending, !requestVsync() { gpuPageVsync(token) }
        if window == nil, attached {
            attached = false
            gpuPageSuspend(token)
        }
        if let window {
            let center = NotificationCenter.default
            let update = #selector(visibilityChanged)
            center.addObserver(self, selector: update, name: NSWindow.didChangeOcclusionStateNotification, object: window)
            center.addObserver(self, selector: update, name: NSWindow.didMiniaturizeNotification, object: window)
            center.addObserver(self, selector: update, name: NSWindow.didDeminiaturizeNotification, object: window)
            center.addObserver(self, selector: update, name: NSApplication.didHideNotification, object: nil)
            center.addObserver(self, selector: update, name: NSApplication.didUnhideNotification, object: nil)
            let focus = #selector(windowFocusChanged)
            center.addObserver(self, selector: focus, name: NSWindow.didBecomeKeyNotification, object: window)
            center.addObserver(self, selector: focus, name: NSWindow.didResignKeyNotification, object: window)
        }
        report()
        gpuPageSetVisible(token, window != nil && !isHiddenOrHasHiddenAncestor)
        visibilityChanged()
    }

    /// Core holds frames while nothing of the window can be seen; a minimized or hidden window
    /// also releases its presenter.
    @objc private func visibilityChanged() {
        guard let window else { return }
        let shown = !window.isMiniaturized && !NSApp.isHidden
        let visible = shown && window.occlusionState.contains(.visible)
        // A frame asked of a display link that has just stopped comes now instead.
        if !visible, #available(macOS 14.0, *), let link = vsyncLink as? CADisplayLink, !link.isPaused {
            link.isPaused = true
            gpuPageVsync(token)
        }
        gpuPageWindowVisible(token, visible)
        if !shown, attached {
            attached = false
            gpuPageSuspend(token)
        } else if shown, !attached {
            report()
        }
    }

    @objc private func windowFocusChanged() {
        gpuPageWindowFocus(token, window?.isKeyWindow ?? false)
        ime.updateCaret()
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        report()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        report()
    }

    private func report() {
        guard let window, bounds.width > 0, bounds.height > 0 else { return }
        let presentable = !window.isMiniaturized && !NSApp.isHidden
        let scale = window.backingScaleFactor
        let insets = safeAreaInsets
        let screen = window.screen?.frame.size ?? window.frame.size
        // The lxapp's chosen scheme, as its WebView pages get it; a float's own window
        // does not carry the main window's appearance.
        let appId = gpuPageInfo(token).app_id.toString()
        let dark = appId.isEmpty
            ? effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            : WebViewManager.resolvedDarkAppearance(appId: appId)
        gpuPageMeasured(
            token,
            UInt32((bounds.width * scale).rounded()),
            UInt32((bounds.height * scale).rounded()),
            Float(scale),
            dark,
            Float(insets.top * scale),
            Float(insets.right * scale),
            Float(insets.bottom * scale),
            Float(insets.left * scale),
            0,
            0,
            0,
            Float(min(screen.width, screen.height)),
            Locale.preferredLanguages.first ?? "en",
            NSWorkspace.shared.accessibilityDisplayShouldReduceMotion,
            NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast,
            RustVec<Float>()
        )
        layer?.contentsScale = scale
        if !attached, presentable {
            attached = true
            gpuPageAttach(
                token,
                UInt(bitPattern: Unmanaged.passUnretained(self).toOpaque()),
                UInt32((bounds.width * scale).rounded()),
                UInt32((bounds.height * scale).rounded())
            )
        }
    }

    /// Calls `gpuPageVsync` on the next refresh of this view's display.
    func requestVsync() -> Bool {
        // An occluded window's display link stops; its frames still complete on the posted path.
        guard #available(macOS 14.0, *), let window, window.occlusionState.contains(.visible)
        else { return false }
        let link: CADisplayLink
        if let existing = vsyncLink as? CADisplayLink {
            link = existing
        } else {
            link = displayLink(target: GpuVsyncTarget(self), selector: #selector(GpuVsyncTarget.tick))
            link.add(to: .main, forMode: .common)
            vsyncLink = link
        }
        link.isPaused = false
        return true
    }

    fileprivate func vsync() {
        if #available(macOS 14.0, *) { (vsyncLink as? CADisplayLink)?.isPaused = true }
        gpuPageVsync(token)
    }

    /// The last presented GPU frame, from Rust's readback.
    func lastFrame() -> CGImage? {
        let frame = gpuPageReadback(token)
        let length = Int(frame.len())
        guard length > 8 else { return nil }
        let bytes = UnsafeRawBufferPointer(start: frame.as_ptr(), count: length)
        let width = Int(bytes.loadUnaligned(fromByteOffset: 0, as: UInt32.self).littleEndian)
        let height = Int(bytes.loadUnaligned(fromByteOffset: 4, as: UInt32.self).littleEndian)
        guard width > 0, height > 0, length == 8 + width * height * 4,
              let provider = CGDataProvider(data: Data(bytes[8...]) as CFData)
        else { return nil }
        return CGImage(
            width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
            bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue),
            provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
    }

    // MARK: Mouse

    private var longPress: Timer?
    /// AppKit's drag threshold, in points.
    private static let slopPoints: CGFloat = 3

    @discardableResult
    private func send(_ phase: UInt8, _ event: NSEvent) -> Bool {
        let point = convert(event.locationInWindow, from: nil)
        let scale = window?.backingScaleFactor ?? 2
        return gpuPagePointer(
            token, phase, Float(point.x * scale), Float(point.y * scale),
            Float(Self.slopPoints * scale), 0, 0, .greatestFiniteMagnitude)
    }

    private func pixels(_ event: NSEvent) -> (Float, Float) {
        let point = convert(event.locationInWindow, from: nil)
        let scale = window?.backingScaleFactor ?? 2
        return (Float(point.x * scale), Float(point.y * scale))
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(NSTrackingArea(
            rect: .zero,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self))
    }

    override func mouseMoved(with event: NSEvent) {
        let (x, y) = pixels(event)
        gpuPageHover(token, true, x, y)
    }

    override func mouseExited(with event: NSEvent) {
        gpuPageHover(token, false, 0, 0)
    }

    override func scrollWheel(with event: NSEvent) {
        if gpuPageInputBlocked(token) { return super.scrollWheel(with: event) }
        if let refresh = lxPullToRefreshController {
            let (x, y) = pixels(event)
            refresh.setAtTop(!gpuPageCanScrollVertically(token, x, y, -1))
            refresh.refreshEnabledFlag()
            if refresh.handleScrollWheel(event) { return }
        }
        let (x, y) = pixels(event)
        let scale = Float(window?.backingScaleFactor ?? 2)
        let precise = event.hasPreciseScrollingDeltas
        let unit: Float = precise ? scale : 1
        let phase: UInt8
        if !event.momentumPhase.isEmpty {
            phase = event.momentumPhase.contains(.ended) ? 3 : 4
        } else if event.phase.contains(.began) || event.phase.contains(.mayBegin) {
            phase = 1
        } else if event.phase.contains(.changed) {
            phase = 2
        } else if event.phase.contains(.ended) || event.phase.contains(.cancelled) {
            phase = 3
        } else {
            phase = 0
        }
        if !gpuPageWheel(
            token, x, y, Float(event.scrollingDeltaX) * unit, Float(event.scrollingDeltaY) * unit,
            precise, phase)
        {
            super.scrollWheel(with: event)
        }
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) {
        if gpuPageInputBlocked(token) { return super.mouseDown(with: event) }
        send(0, event)
    }

    // MARK: Keyboard

    override var acceptsFirstResponder: Bool { true }

    /// Focus that arrives by Tab walks into the page; a click only takes it.
    override func becomeFirstResponder() -> Bool {
        if let event = NSApp.currentEvent, event.type == .keyDown, event.keyCode == 48 {
            gpuPageFocus(token, true, event.modifierFlags.contains(.shift))
        }
        return true
    }

    override func resignFirstResponder() -> Bool {
        gpuPageFocus(token, false, false)
        if ime.isActive { ime.clear(blur: true) }
        return true
    }

    override func keyDown(with event: NSEvent) {
        if gpuPageInputBlocked(token) { return super.keyDown(with: event) }
        if editorKeyDown(event) { return }
        let key: UInt8? = switch event.keyCode {
        case 48: 0
        case 126: 1
        case 125: 2
        case 123: 3
        case 124: 4
        case 115: 5
        case 119: 6
        case 36, 76, 49: 7
        case 53: 8
        default: nil
        }
        let plain = event.modifierFlags.isDisjoint(with: [.command, .control, .option])
        guard let key, plain, !gpuPageInputBlocked(token),
              gpuPageKey(token, key, event.modifierFlags.contains(.shift), event.isARepeat)
        else { return super.keyDown(with: event) }
    }
    override func mouseDragged(with event: NSEvent) { send(1, event) }
    override func mouseUp(with event: NSEvent) {
        send(2, event)
        if event.clickCount == 2 {
            let (x, y) = pixels(event)
            gpuPageSelectWordAt(token, x, y)
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        let (x, y) = pixels(event)
        if gpuPageInputBlocked(token) || !gpuPageContextMenu(token, x, y) { super.rightMouseDown(with: event) }
    }

    func armLongPress() {
        longPress?.invalidate()
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

    // MARK: Feedback

    /// Press and focus-ring layers over the frame, made on first use.
    private var highlights: [GpuHighlightLayer] = []

    func feedback(ring: Bool, rects: UnsafeBufferPointer<Float>, argb: UInt32, width: Float) {
        if highlights.isEmpty {
            if rects.isEmpty { return }
            highlights = [GpuHighlightLayer(), GpuHighlightLayer()]
            highlights.forEach { layer!.addSublayer($0) }
        }
        let highlight = highlights[ring ? 1 : 0]
        highlight.frame = bounds
        highlight.contentsScale = window?.backingScaleFactor ?? 2
        highlight.update(rects: rects, argb: argb, ringWidth: width)
    }

    func drawFeedback(in context: CGContext) {
        for highlight in highlights where !highlight.isHidden { highlight.draw(in: context) }
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
        layer?.contentsScale = window?.backingScaleFactor ?? 2
        layer?.contents = image
        return true
    }
}

/// Colours for the host chrome around a GPU page, chosen by the core from its palette.
struct GpuHostChrome {
    /// Posted on the main thread with the page view as object.
    static let changed = Notification.Name("LingXiaGpuHostChromeChanged")
    let canvas: NSColor
    let bar: NSColor
    let barLine: NSColor
    let content: NSColor
    let contentMuted: NSColor
    let accent: NSColor
    let dark: Bool
}

/// Host screenshots reach this through the Objective-C runtime, so the capture code needs
/// no link-time dependency on the SDK.
@MainActor
@objc(LingXiaGpuCapture)
public final class GpuCapture: NSObject {
    @objc(overlayContentView:into:)
    public static func overlay(_ contentView: NSView, into rep: NSBitmapImageRep) {
        GpuPages.overlay(contentView, into: rep)
    }
}

/// A display link retains its target; this keeps it from retaining the view.
@MainActor
private final class GpuVsyncTarget: NSObject {
    weak var view: GpuPageView?
    init(_ view: GpuPageView) { self.view = view }
    @objc func tick() { view?.vsync() }
}

extension GpuPageView: PullRefreshPage {
    private var info: GpuPageInfo { gpuPageInfo(token) }
    var refreshAppId: String? { info.app_id.toString().nilIfEmpty }
    var refreshPath: String? { info.path.toString().nilIfEmpty }
    var refreshWebTag: String? { info.webtag.toString().nilIfEmpty }
    var refreshBackground: NSColor? { chrome?.canvas }
}

private extension String {
    var nilIfEmpty: String? { isEmpty ? nil : self }
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

    /// The GPU view showing the page `webtag` of `appId`.
    @MainActor static func view(appId: String, webtag: String) -> GpuPageView? {
        views.values.first { $0.refreshAppId == appId && $0.refreshWebTag == webtag }
    }

    static func armLongPress(token: Int64) {
        MainActor.assumeIsolated { views[token]?.armLongPress() }
    }

    static func cancelLongPress(token: Int64) {
        MainActor.assumeIsolated { views[token]?.cancelLongPress() }
    }

    /// A mouse release carries no velocity, so the page never asks a desktop view to fling.
    static func fling(token: Int64, region: RustStr, horizontal: Bool, velocity: Float) {}

    static func feedback(token: Int64, ring: Bool, shape: UInt, count: UInt, color: UInt32, width: Float) {
        MainActor.assumeIsolated {
            let rects = UnsafeBufferPointer(start: UnsafePointer<Float>(bitPattern: shape), count: Int(count))
            views[token]?.feedback(ring: ring, rects: rects, argb: color, width: width)
        }
    }

    static func haptic(token: Int64) {
        MainActor.assumeIsolated { NSHapticFeedbackManager.defaultPerformer.perform(.generic, performanceTime: .now) }
    }

    static func copyText(text: RustStr) {
        let text = text.toString()
        MainActor.assumeIsolated {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
    }

    static func stopFling(token: Int64) {}

    static func imePresented(token: Int64, generation: UInt64, node: RustStr) {
        let node = node.toString()
        MainActor.assumeIsolated {
            guard let view = views[token] else { return }
            view.ime.presented(GpuPageView.decodeNode(node), ticket: view.ime.capture())
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

    static func hostChrome(token: Int64, canvas: UInt32, bar: UInt32, bar_line barLine: UInt32, content: UInt32, content_muted contentMuted: UInt32, accent: UInt32, dark: Bool) {
        MainActor.assumeIsolated {
            guard let view = views[token] else { return }
            let color = { (argb: UInt32) in
                NSColor(srgbRed: CGFloat((argb >> 16) & 0xff) / 255, green: CGFloat((argb >> 8) & 0xff) / 255,
                        blue: CGFloat(argb & 0xff) / 255, alpha: CGFloat(argb >> 24) / 255)
            }
            view.chrome = GpuHostChrome(
                canvas: color(canvas), bar: color(bar), barLine: color(barLine), content: color(content),
                contentMuted: color(contentMuted), accent: color(accent), dark: dark)
        }
    }

    static func textMenu(token: Int64, menu: RustStr) {
        let menu = menu.toString()
        MainActor.assumeIsolated { views[token]?.showTextMenu(menu) }
    }

    static func openPicker(token: Int64, generation: UInt64, node: RustStr) {
        let node = node.toString()
        MainActor.assumeIsolated {
            guard let view = views[token], let node = GpuPageView.decodeNode(node) else { return }
            view.openPicker(generation: generation, node: node)
        }
    }

    static func imeActivation(token: Int64) {
        MainActor.assumeIsolated {
            if let ime = views[token]?.ime, ime.isActive { ime.clear(blur: true) }
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

    static func accessibilityEvent(token: Int64, kind: UInt8, id: UInt32, text: RustStr) {
        let text = text.toString()
        MainActor.assumeIsolated { views[token]?.accessibilityEvent(kind: kind, id: id, text: text) }
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

    /// Draw each GPU page of `contentView`'s window over its cached bitmap, at the page's frame.
    @MainActor static func overlay(_ contentView: NSView, into rep: NSBitmapImageRep) {
        guard let context = NSGraphicsContext(bitmapImageRep: rep) else { return }
        NSGraphicsContext.saveGraphicsState()
        defer { NSGraphicsContext.restoreGraphicsState() }
        NSGraphicsContext.current = context
        for view in views.values where view.window === contentView.window && !view.isHiddenOrHasHiddenAncestor {
            guard let image = view.lastFrame() else { continue }
            let frame = view.convert(view.bounds, to: contentView)
            let rect = contentView.isFlipped
                ? CGRect(x: frame.minX, y: contentView.bounds.height - frame.maxY, width: frame.width, height: frame.height)
                : frame
            context.cgContext.draw(image, in: rect)
            // Press and focus feedback sit over the frame on screen, so in the capture too.
            let cg = context.cgContext
            cg.saveGState()
            cg.translateBy(x: rect.minX, y: rect.maxY)
            cg.scaleBy(x: 1, y: -1)
            view.drawFeedback(in: cg)
            cg.restoreGState()
        }
        context.flushGraphics()
    }

    /// The GPU page token of a page of the app, or 0 when it is not a GPU page.
    static func token(appId: String, sessionId: UInt64, pageInstanceId: String?) -> Int64 {
        pageBinding(appId, sessionId, pageInstanceId ?? "").gpu_token
    }
}
#endif
