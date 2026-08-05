import Foundation

#if os(iOS)
import UIKit

/// Runtime half of the launch screen: covers the window from the home app's
/// cold start until its page first renders, showing the full-screen splash
/// image (aspect-fill) over the background color the static `UILaunchScreen`
/// used. Dismissed by the runtime's onHomeFirstReady signal, with a timeout
/// fallback so a broken page never leaves it stuck.
///
/// Assets are looked up by the names the CLI generates (`LingXiaSplash` image,
/// `LingXiaSplashBackground` color); when the color is absent the overlay is
/// disabled entirely. An online-updated image dropped under
/// `<Application Support>/lingxia/splash/{light,dark}.png` takes precedence
/// over the bundled image on the next launch.
@MainActor
enum LingXiaSplashOverlay {
    private static let timeoutSeconds: TimeInterval = 6
    private static let fadeSeconds: TimeInterval = 0.25
    private static let cacheSubpath = "lingxia/splash"

    private static var overlay: UIView?
    private static var shownThisProcess = false
    private static var homeReadySeen = false

    /// Attach over `window` on the home app's cold start when splash assets exist.
    static func attachIfNeeded(to window: UIWindow) {
        guard !shownThisProcess, !homeReadySeen else { return }
        guard let background = resolveBackground(for: window.traitCollection) else { return }

        let view = UIView(frame: window.bounds)
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.backgroundColor = background
        // Swallow touches so nothing underneath is tappable while visible.
        view.isUserInteractionEnabled = true

        if let splash = resolveImage(for: window.traitCollection) {
            let imageView = UIImageView(image: splash)
            imageView.frame = view.bounds
            imageView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            imageView.contentMode = .scaleAspectFill
            imageView.clipsToBounds = true
            view.addSubview(imageView)
        }

        window.addSubview(view)
        overlay = view
        shownThisProcess = true
        DispatchQueue.main.asyncAfter(deadline: .now() + timeoutSeconds) {
            dismiss()
        }
    }

    /// Runtime signal (via `LxApp.onHomeFirstReady`): home page rendered its first frame.
    static func notifyHomeReady() {
        homeReadySeen = true
        dismiss()
    }

    private static func dismiss() {
        guard let view = overlay else { return }
        overlay = nil
        UIView.animate(
            withDuration: fadeSeconds,
            animations: { view.alpha = 0 },
            completion: { _ in view.removeFromSuperview() }
        )
    }

    private static func resolveImage(for traits: UITraitCollection) -> UIImage? {
        let dark = traits.userInterfaceStyle == .dark
        if let support = FileManager.default.urls(
            for: .applicationSupportDirectory, in: .userDomainMask
        ).first {
            let cacheDir = support.appendingPathComponent(cacheSubpath)
            for name in dark ? ["dark.png", "light.png"] : ["light.png"] {
                let url = cacheDir.appendingPathComponent(name)
                if let image = UIImage(contentsOfFile: url.path) {
                    return image
                }
            }
        }
        // Plain bundle resources first: `actool` can fail and leave the
        // compiled catalog out of the app entirely.
        let names = dark
            ? ["LingXiaSplash~dark", "LingXiaSplash"]
            : ["LingXiaSplash"]
        for name in names {
            if let url = Bundle.main.url(forResource: name, withExtension: "png"),
               let image = UIImage(contentsOfFile: url.path) {
                return image
            }
        }
        return UIImage(named: "LingXiaSplash")
    }

    /// Background color from Info.plist (always written when `splash:` is
    /// configured), falling back to the asset-catalog color.
    private static func resolveBackground(for traits: UITraitCollection) -> UIColor? {
        let dark = traits.userInterfaceStyle == .dark
        let keys = dark
            ? ["LingXiaSplashBackgroundDark", "LingXiaSplashBackground"]
            : ["LingXiaSplashBackground"]
        for key in keys {
            if let hex = Bundle.main.object(forInfoDictionaryKey: key) as? String,
               let color = UIColor(lingXiaHex: hex) {
                return color
            }
        }
        return UIColor(named: "LingXiaSplashBackground")
    }
}

private extension UIColor {
    /// Parse `#RRGGBB` as written into Info.plist by the CLI.
    convenience init?(lingXiaHex hex: String) {
        var value = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("#") { value.removeFirst() }
        guard value.count == 6, let rgb = UInt32(value, radix: 16) else { return nil }
        self.init(
            red: CGFloat((rgb >> 16) & 0xFF) / 255,
            green: CGFloat((rgb >> 8) & 0xFF) / 255,
            blue: CGFloat(rgb & 0xFF) / 255,
            alpha: 1
        )
    }
}

#else

/// The splash overlay is a mobile concern — desktop shells present native
/// chrome immediately, so the ready signal is a no-op here.
@MainActor
enum LingXiaSplashOverlay {
    static func notifyHomeReady() {}
}

#endif
