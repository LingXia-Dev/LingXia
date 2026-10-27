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
    /// Paused between requested frames; follows the display the view is on.
    private var vsyncLink: AnyObject?

    init(token: Int64) {
        self.token = token
        super.init(frame: .zero)
        wantsLayer = true
        layerContentsRedrawPolicy = .never
        layer?.contentsGravity = .topLeft
        layer?.isOpaque = true
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
        gpuPageWindowVisible(token, shown && window.occlusionState.contains(.visible))
        if !shown, attached {
            attached = false
            gpuPageSuspend(token)
        } else if shown, !attached {
            report()
        }
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
        let dark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
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
            NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
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
        guard #available(macOS 14.0, *), window != nil else { return false }
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
        }
        context.flushGraphics()
    }

    /// The GPU page token of a page of the app, or 0 when it is not a GPU page.
    static func token(appId: String, sessionId: UInt64, pageInstanceId: String?) -> Int64 {
        pageBinding(appId, sessionId, pageInstanceId ?? "").gpu_token
    }
}
#endif
