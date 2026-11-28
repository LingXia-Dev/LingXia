#if os(iOS)
import UIKit
import CLingXiaRustAPI

/// The App Link a cold launch carries. When there is one, its open is the cold open: the shell
/// does not open home first, so the link's page is not stacked over a home nobody asked for.
@MainActor
enum LaunchLink {
    /// The launch announced a link that has not reached the runtime yet.
    private static var announced = false
    /// A link that arrived before the runtime could take it.
    private static var held: URL?
    /// Opens home; set by the shell once it would have opened home itself.
    private static var openHome: (() -> Void)?
    private static var activation: NSObjectProtocol?
    /// Set when the host installs `LingxiaAppDelegate`.
    static var delegateInstalled = false
    private static var warned = false

    static func announce(_ options: UIScene.ConnectionOptions) {
        let web = options.userActivities.first { $0.activityType == NSUserActivityTypeBrowsingWeb }
        guard let url = options.urlContexts.first?.url ?? web?.webpageURL,
              url.scheme == "https" else { return }
        announced = true
    }

    static func receive(_ url: URL) {
        guard url.scheme == "https" else { return }
        guard openHome != nil else {
            held = url
            return
        }
        deliver(url)
    }

    /// The shell is ready to open home: it does, unless a launch link opens instead.
    static func shellReady(openHome: @escaping () -> Void) {
        self.openHome = openHome
        warnIfUnwired()
        if let url = held {
            held = nil
            deliver(url)
        } else if announced {
            awaitLink()
        } else {
            openHome()
        }
    }

    /// Without the delegate a cold link is only seen after home opened, so it stacks over home.
    private static func warnIfUnwired() {
        guard !delegateInstalled, !warned, appLinksConfigured() else { return }
        warned = true
        LXLog.warn(
            "appLinks are configured but the App does not install LingxiaAppDelegate, so a cold "
                + "link opens over the home page. Add to the App struct: "
                + "@UIApplicationDelegateAdaptor(LingxiaAppDelegate.self) private var appDelegate",
            category: "AppLink"
        )
    }

    private static func deliver(_ url: URL) {
        let opened = onApplinkReceived(url.absoluteString) > 0
        guard announced else { return }
        announced = false
        if !opened { openHome?() }
    }

    /// SwiftUI hands a launch URL to `onOpenURL` before the scene is active; a host that never
    /// passes it on still gets home once it is.
    private static func awaitLink() {
        let active = UIApplication.shared.connectedScenes.contains { $0.activationState == .foregroundActive }
        if active {
            DispatchQueue.main.async { giveUp() }
            return
        }
        activation = NotificationCenter.default.addObserver(
            forName: UIScene.didActivateNotification, object: nil, queue: .main
        ) { _ in
            MainActor.assumeIsolated { giveUp() }
        }
    }

    private static func giveUp() {
        if let activation { NotificationCenter.default.removeObserver(activation) }
        activation = nil
        guard announced else { return }
        announced = false
        openHome?()
    }
}

/// The app delegate a SwiftUI host adopts with `@UIApplicationDelegateAdaptor`, so a link that
/// launched the app is known before the first view appears.
open class LingxiaAppDelegate: NSObject, UIApplicationDelegate {
    override public init() {
        super.init()
        MainActor.assumeIsolated { LaunchLink.delegateInstalled = true }
    }

    open func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        MainActor.assumeIsolated { LaunchLink.announce(options) }
        return UISceneConfiguration(name: nil, sessionRole: connectingSceneSession.role)
    }
}
#endif
