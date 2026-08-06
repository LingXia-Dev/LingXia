import Foundation
import OSLog

#if os(iOS)
import UIKit

private let splashLog = OSLog(subsystem: "LingXia", category: "Splash")

/// Runtime half of the launch screen: covers the screen from the home app's
/// cold start until its page first renders, fading the cover — bundled, or
/// the hook's pick for this launch — in over the `UILaunchScreen`
/// placeholder it reproduces (background color + centered mark), so the
/// launch reads "tap the icon, see the cover". Dismissed by the runtime's
/// onHomeFirstReady signal, with a timeout fallback so a broken page never
/// leaves it stuck.
///
/// It lives in its own window above the app's: the host swaps the app
/// window's `rootViewController` (and may present the lxapp manager modally)
/// right after startup, either of which would bury a plain subview overlay.
///
/// Assets are looked up by the names the CLI generates
/// (`LingXiaSplashBackground` color, `LingXiaSplash` cover,
/// `LingXiaSplashMark` mark); when the color is absent the overlay is
/// disabled entirely.
@MainActor
enum LingXiaSplashOverlay {
    private static let timeoutSeconds: TimeInterval = 6
    private static let fadeSeconds: TimeInterval = 0.25

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
        guard let background = resolveBackground() else {
            os_log("splash skipped: no background configured", log: splashLog, type: .info)
            return
        }

        // Let the host's Rust hook pick this launch's cover before anything
        // shows, so the bundled art never flashes first. Registration is
        // cheap and has no runtime dependency; selection is budgeted in core
        // and runs before initialization, so the data dir comes from here.
        LxAppCore.registerNativeHostAddonOnce()
        let directories = LxAppDirectoryFactory.createDirectoryConfig()
        let dark = window.traitCollection.userInterfaceStyle == .dark
        let picked = splashSelectCover(directories.dataPath, dark).toString()

        let host = UIViewController()
        host.view.backgroundColor = background
        let coverImage = (!picked.isEmpty ? UIImage(contentsOfFile: picked) : nil)
            ?? resolveCover()
        if let coverImage {
            // The cover: full bleed, fully opaque from the very first frame —
            // the launch frame's handoff is the only transition onto it. Any
            // fade here would cross-blend the cover's art with what sits
            // beneath, a beat the other platforms never show.
            let cover = UIImageView(image: coverImage)
            cover.frame = host.view.bounds
            cover.autoresizingMask = [.flexibleWidth, .flexibleHeight]
            cover.contentMode = .scaleAspectFill
            cover.clipsToBounds = true
            host.view.addSubview(cover)
        } else if let mark = resolveMark() {
            // No cover configured: reproduce the launch frame's own
            // composition — brand color plus the centered mark — so the only
            // transition the user ever sees is into the home page. The
            // catalog ships the mark as a 3x entry, so the frame draws it at
            // pixels/3 points; match that, not the screen scale.
            let size = CGSize(width: mark.size.width / 3, height: mark.size.height / 3)
            let view = UIImageView(image: mark)
            view.frame = CGRect(
                x: (host.view.bounds.width - size.width) / 2,
                y: (host.view.bounds.height - size.height) / 2,
                width: size.width,
                height: size.height
            )
            view.autoresizingMask = [
                .flexibleLeftMargin, .flexibleRightMargin,
                .flexibleTopMargin, .flexibleBottomMargin,
            ]
            host.view.addSubview(view)
        }

        // Paint the app's own window too. Anything that shows through before
        // or behind the cover — the host's first SwiftUI frame, the gap while
        // the lxapp manager is being presented — is then the brand color
        // instead of the system default white.
        window.backgroundColor = background

        let splash = UIWindow(windowScene: scene)
        splash.windowLevel = .normal + 100
        splash.rootViewController = host
        splash.isHidden = false

        splashWindow = splash
        shownThisProcess = true
        shownAt = CFAbsoluteTimeGetCurrent()
        os_log("splash shown (cover: %{public}@)", log: splashLog, type: .info,
               coverImage == nil ? "none" : (picked.isEmpty ? "bundled" : "picked"))

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

    /// Plain bundle resources, deliberately not the catalog: `actool` can
    /// fail and leave the compiled catalog out of the app, and the overlay
    /// must not lose its images when it does.
    private static func resolveCover() -> UIImage? {
        guard let url = Bundle.main.url(forResource: "LingXiaSplash", withExtension: "png")
        else { return nil }
        return UIImage(contentsOfFile: url.path)
    }

    private static func resolveMark() -> UIImage? {
        guard let url = Bundle.main.url(forResource: "LingXiaSplashMark", withExtension: "png")
        else { return nil }
        return UIImage(contentsOfFile: url.path)
    }

    /// Background color from Info.plist (always written when `splash:` is
    /// configured), falling back to the asset-catalog color.
    private static func resolveBackground() -> UIColor? {
        if let hex = Bundle.main.object(forInfoDictionaryKey: "LingXiaSplashBackground") as? String,
           let color = UIColor(lingXiaHex: hex) {
            return color
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
