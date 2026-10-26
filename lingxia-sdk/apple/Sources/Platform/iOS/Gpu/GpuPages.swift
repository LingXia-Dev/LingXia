import Foundation
#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// A GPU page's view. Rust owns the page; this view reports its measurements
/// and shows the frames Rust presents.
@MainActor
final class GpuPageView: UIView {
    let token: Int64

    init(token: Int64) {
        self.token = token
        super.init(frame: .zero)
        isOpaque = true
        layer.contentsGravity = .topLeft
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

    static func dispose(token: Int64) {
        MainActor.assumeIsolated {
            views.removeValue(forKey: token)?.removeFromSuperview()
        }
    }
}
#else
/// GPU pages render only on iOS.
public enum GpuPages {
    static func presentPixels(token: Int64, pixels: UInt, length: UInt, width: UInt32, height: UInt32) -> Bool {
        false
    }

    static func dispose(token: Int64) {}
}
#endif
