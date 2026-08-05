import Foundation
import OSLog

#if os(iOS)
import UIKit

private let splashLog = OSLog(subsystem: "LingXia", category: "Splash")

/// Runtime half of the launch screen: covers the screen from the home app's
/// cold start until its page first renders, showing the full-screen splash
/// image (aspect-fill) over the background color the static `UILaunchScreen`
/// used. Dismissed by the runtime's onHomeFirstReady signal, with a timeout
/// fallback so a broken page never leaves it stuck.
///
/// It lives in its own window above the app's: the host swaps the app
/// window's `rootViewController` (and may present the lxapp manager modally)
/// right after startup, either of which would bury a plain subview overlay.
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

    private static var splashWindow: UIWindow?
    private static var shownThisProcess = false
    private static var homeReadySeen = false
    private static var shownAt: CFAbsoluteTime = 0

    /// Cover the screen at host startup, resolving the active scene's window.
    /// Idempotent — safe to call from every plausible cold-start entry point.
    static func attachIfNeeded() {
        guard !shownThisProcess, !homeReadySeen else { return }
        let window = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first?
            .windows
            .first
        guard let window else {
            os_log("splash skipped: no window yet", log: splashLog, type: .info)
            return
        }
        attachIfNeeded(to: window)
    }

    /// Cover the screen on the home app's cold start when splash assets exist.
    static func attachIfNeeded(to window: UIWindow) {
        guard !shownThisProcess, !homeReadySeen else { return }
        guard let scene = window.windowScene else {
            os_log("splash skipped: no window scene", log: splashLog, type: .info)
            return
        }
        guard let background = resolveBackground(for: window.traitCollection) else {
            os_log("splash skipped: no background configured", log: splashLog, type: .info)
            return
        }

        let host = UIViewController()
        host.view.backgroundColor = background
        let image = resolveImage(for: window.traitCollection)
        if let image {
            let imageView = UIImageView(image: image)
            imageView.frame = host.view.bounds
            imageView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            imageView.contentMode = .scaleAspectFill
            imageView.clipsToBounds = true
            host.view.addSubview(imageView)
        }

        let splash = UIWindow(windowScene: scene)
        splash.windowLevel = .normal + 100
        splash.rootViewController = host
        splash.isHidden = false

        splashWindow = splash
        shownThisProcess = true
        shownAt = CFAbsoluteTimeGetCurrent()
        os_log("splash shown (image: %{public}@)", log: splashLog, type: .info,
               image == nil ? "none" : "yes")

        DispatchQueue.main.asyncAfter(deadline: .now() + timeoutSeconds) {
            if splashWindow != nil {
                os_log("splash dismissed by timeout", log: splashLog, type: .error)
            }
            dismiss()
        }
    }

    /// Runtime signal (via `LxApp.onHomeFirstReady`): home page rendered its first frame.
    static func notifyHomeReady() {
        homeReadySeen = true
        dismiss()
    }

    private static func dismiss() {
        guard let splash = splashWindow else { return }
        splashWindow = nil
        os_log("splash dismissed after %{public}.2fs", log: splashLog, type: .info,
               CFAbsoluteTimeGetCurrent() - shownAt)
        UIView.animate(
            withDuration: fadeSeconds,
            animations: { splash.alpha = 0 },
            completion: { _ in splash.isHidden = true }
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
