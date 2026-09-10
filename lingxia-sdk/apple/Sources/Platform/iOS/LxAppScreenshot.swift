#if os(iOS)
import AVFoundation
import UIKit

/// Host-surface PNG for `lxdev app screenshot`.
///
/// `CALayer.render(in:)` skips `AVPlayerLayer` (hardware compositor). Draw
/// every window in the scene with `drawHierarchy(afterScreenUpdates:)`, then
/// stamp the current video frame from `AVAssetImageGenerator` over each
/// player layer so a playing preview is visible in the capture.
enum LxAppScreenshot {
    static func pngData() -> Data? {
        let windows = captureWindows()
        guard let first = windows.first else { return nil }
        let bounds = first.windowScene?.coordinateSpace.bounds ?? first.screen.bounds
        guard bounds.width > 1, bounds.height > 1 else { return nil }

        let format = UIGraphicsImageRendererFormat()
        format.opaque = true
        format.scale = first.screen.scale
        let renderer = UIGraphicsImageRenderer(bounds: bounds, format: format)
        let image = renderer.image { ctx in
            UIColor.black.setFill()
            ctx.fill(bounds)
            for window in windows {
                let frame = window.convert(window.bounds, to: nil)
                _ = window.drawHierarchy(in: frame, afterScreenUpdates: true)
                overlayPlayerFrames(
                    from: window.layer,
                    into: ctx.cgContext,
                    root: window.layer,
                    windowFrame: frame
                )
            }
        }
        return image.pngData()
    }

    private static func captureWindows() -> [UIWindow] {
        var windows: [UIWindow] = []
        for scene in UIApplication.shared.connectedScenes {
            guard let windowScene = scene as? UIWindowScene else { continue }
            if scene.activationState != .foregroundActive,
               scene.activationState != .foregroundInactive {
                continue
            }
            windows.append(contentsOf: windowScene.windows)
        }
        if windows.isEmpty {
            windows = UIApplication.shared.windows
        }
        return windows
            .filter { !$0.isHidden && $0.alpha > 0.01 }
            .sorted { $0.windowLevel.rawValue < $1.windowLevel.rawValue }
    }

    private static func overlayPlayerFrames(
        from layer: CALayer,
        into context: CGContext,
        root: CALayer,
        windowFrame: CGRect
    ) {
        if let playerLayer = layer as? AVPlayerLayer {
            drawCurrentFrame(playerLayer, into: context, root: root, windowFrame: windowFrame)
        }
        for child in layer.sublayers ?? [] {
            overlayPlayerFrames(from: child, into: context, root: root, windowFrame: windowFrame)
        }
    }

    private static func drawCurrentFrame(
        _ playerLayer: AVPlayerLayer,
        into context: CGContext,
        root: CALayer,
        windowFrame: CGRect
    ) {
        guard let player = playerLayer.player, let item = player.currentItem else { return }
        guard item.status == .readyToPlay else { return }
        let time = player.currentTime()
        guard time.isValid, !time.isIndefinite else { return }

        let generator = AVAssetImageGenerator(asset: item.asset)
        generator.appliesPreferredTrackTransform = true
        generator.requestedTimeToleranceBefore = .positiveInfinity
        generator.requestedTimeToleranceAfter = .positiveInfinity
        guard let frame = try? generator.copyCGImage(at: time, actualTime: nil) else { return }

        let videoRect = playerLayer.convert(playerLayer.videoRect, to: root)
        let dest = videoRect.offsetBy(dx: windowFrame.minX, dy: windowFrame.minY)
        guard dest.width > 1, dest.height > 1 else { return }
        context.draw(frame, in: dest)
    }
}

@_cdecl("lingxia_ios_capture_app_png")
public func lingxia_ios_capture_app_png(
    outLen: UnsafeMutablePointer<Int>
) -> UnsafeMutablePointer<UInt8>? {
    let capture: () -> Data? = {
        LxAppScreenshot.pngData()
    }
    let data: Data?
    if Thread.isMainThread {
        data = capture()
    } else {
        data = DispatchQueue.main.sync(execute: capture)
    }
    guard let data, !data.isEmpty else {
        outLen.pointee = 0
        return nil
    }
    let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: data.count)
    data.copyBytes(to: pointer, count: data.count)
    outLen.pointee = data.count
    return pointer
}

@_cdecl("lingxia_ios_capture_app_png_free")
public func lingxia_ios_capture_app_png_free(
    ptr: UnsafeMutablePointer<UInt8>?,
    len: Int
) {
    guard let ptr, len > 0 else { return }
    ptr.deallocate()
}
#endif
