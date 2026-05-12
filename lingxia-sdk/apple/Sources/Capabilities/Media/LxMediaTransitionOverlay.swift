#if os(iOS)
import AVFoundation
import Foundation
import UIKit

/// Thin `UIImageView` placed above the player layer to bridge visual gaps
/// during media-item transitions (cold start, manual jumps, error recovery).
///
/// Mirrors the Android `VideoTransitionOverlay`:
/// - Hidden by default; shown only during transitions.
/// - Doesn't own the `UIImage` reference — the cache does. `hide` clears the
///   image-view reference; the cache retains the image for future hits.
/// - `setObjectFit` keeps the overlay's pixels aligned with the live video so
///   transitions don't show a size jump.
@MainActor
final class LxMediaTransitionOverlay {

    private weak var host: UIView?
    private let imageView = UIImageView()
    private var lastShown: UIImage?
    private var attached = false

    init(host: UIView) {
        self.host = host
        imageView.contentMode = .scaleAspectFill
        imageView.clipsToBounds = true
        imageView.isHidden = true
        imageView.isUserInteractionEnabled = false
        imageView.backgroundColor = .clear
        // Pin the imageView to the host on attach.
    }

    /// Mount the overlay above the host's existing sublayers/views. Idempotent.
    /// `belowSubview`: optional sibling to insert below (e.g. controls overlay).
    func attach(belowSubview ceiling: UIView? = nil) {
        guard !attached, let host = host else { return }
        imageView.frame = host.bounds
        imageView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        if let ceiling = ceiling, ceiling.superview === host {
            host.insertSubview(imageView, belowSubview: ceiling)
        } else {
            host.addSubview(imageView)
        }
        attached = true
    }

    /// Show `image`. No-op if the same image is already visible (avoids a
    /// needless `image=` set which forces a layer redraw).
    func show(_ image: UIImage) {
        if lastShown === image && !imageView.isHidden { return }
        imageView.image = image
        imageView.isHidden = false
        lastShown = image
    }

    /// Hide and release the image-view reference. The cache still owns the image.
    func hide() {
        if imageView.isHidden && lastShown == nil { return }
        imageView.isHidden = true
        imageView.image = nil
        lastShown = nil
    }

    /// Mirror the player's video gravity so the overlay's contents align.
    func setVideoGravity(_ gravity: AVLayerVideoGravity) {
        switch gravity {
        case .resizeAspectFill: imageView.contentMode = .scaleAspectFill
        case .resizeAspect: imageView.contentMode = .scaleAspectFit
        case .resize: imageView.contentMode = .scaleToFill
        default: imageView.contentMode = .scaleAspectFill
        }
    }

    /// Mirror the player's corner radius for in-line clipping.
    func setCornerRadius(_ radius: CGFloat) {
        imageView.layer.cornerRadius = radius
        imageView.layer.masksToBounds = radius > 0
    }

    /// Mirror the player's display rotation (degrees, 0/90/180/270).
    func setRotation(degrees: Int) {
        let radians = CGFloat(degrees) * .pi / 180
        imageView.transform = CGAffineTransform(rotationAngle: radians)
    }

    func detach() {
        imageView.removeFromSuperview()
        imageView.image = nil
        lastShown = nil
        attached = false
    }
}
#endif
