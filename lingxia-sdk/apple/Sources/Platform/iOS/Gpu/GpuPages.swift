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
    }

    @objc private func suspend() {
        guard attached else { return }
        attached = false
        gpuPageSuspend(token)
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
            0,
            0,
            0,
            Float(shorter),
            Locale.preferredLanguages.first ?? "en",
            UIAccessibility.isReduceMotionEnabled
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

    fileprivate func vsync() {
        vsyncLink?.isPaused = true
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
        layer.contentsScale = window?.screen.scale ?? UIScreen.main.scale
        layer.contents = image
        return true
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

    /// Draw each GPU page of `window` at its frame; layer rendering cannot see Metal drawables.
    @MainActor static func overlay(_ window: UIWindow, into context: CGContext) {
        for view in views.values where view.window === window && !view.isHidden {
            guard let image = view.lastFrame() else { continue }
            let rect = view.convert(view.bounds, to: window)
            context.saveGState()
            // UIKit contexts are flipped; CGImage draws bottom-up.
            context.translateBy(x: rect.minX, y: rect.maxY)
            context.scaleBy(x: 1, y: -1)
            context.draw(image, in: CGRect(origin: .zero, size: rect.size))
            context.restoreGState()
        }
    }

    /// The GPU page token of a page of the app, or 0 when it is not a GPU page.
    static func token(appId: String, sessionId: UInt64, pageInstanceId: String?) -> Int64 {
        pageBinding(appId, sessionId, pageInstanceId ?? "").gpu_token
    }
}
#endif
