import Foundation
import QuartzCore
import CLingXiaRustAPI

/// A page that leaves the screen and is kept shows its last frame when it comes back: the view
/// holds a snapshot over its layer until the page presents a frame at its current size.
extension GpuPageView {
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

    /// A layer over `surface` showing the last frame; nil when no frame is held.
    func makeCover(over surface: CALayer) -> CALayer? {
        guard let image = lastFrame() else { return nil }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        let cover = CALayer()
        cover.contents = image
        cover.contentsGravity = .resize
        cover.frame = surface.bounds
        surface.addSublayer(cover)
        CATransaction.commit()
        return cover
    }

    static func dropCover(_ cover: CALayer?) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        cover?.removeFromSuperlayer()
        CATransaction.commit()
    }
}
