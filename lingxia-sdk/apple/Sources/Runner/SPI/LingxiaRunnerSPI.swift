import Foundation

#if os(macOS)
import AppKit
import WebKit

/// Tooling-only SPI consumed by `tools/lingxia-runner`.
@_spi(Runner) public enum LingxiaRunnerSPI {
    @MainActor
    public enum Runtime {
        public static func setOpenUrlHandler(_ handler: @escaping (String, UInt64, String) -> Bool) {
            RunnerBridge.setOpenUrlHandler(handler)
        }

        public static func sessionId(for appId: String) -> UInt64? {
            RunnerBridge.sessionId(for: appId)
        }

        public static func currentAppId() -> String? {
            RunnerBridge.currentAppId()
        }

        public static func currentPath() -> String {
            RunnerBridge.currentPath()
        }

        public static func setSessionId(_ sessionId: UInt64, for appId: String) {
            RunnerBridge.setSessionId(sessionId, for: appId)
        }

        public static func removeSessionId(for appId: String) {
            RunnerBridge.removeSessionId(for: appId)
        }

        public static func setCurrentApp(appId: String, path: String) {
            RunnerBridge.setCurrentApp(appId: appId, path: path)
        }

        public static func setCurrentPath(_ path: String) {
            RunnerBridge.setCurrentPath(path)
        }
    }

    @MainActor
    public enum WebView {
        public static func current() -> WKWebView? {
            RunnerBridge.currentWebView()
        }

        public static func removeCurrentFromSuperview() {
            RunnerBridge.removeCurrentWebViewFromSuperview()
        }

        public static func resolve(
            appId: String,
            path: String,
            sessionId: UInt64
        ) -> WKWebView? {
            RunnerBridge.resolveWebView(appId: appId, path: path, sessionId: sessionId)
        }

        public static func resolve(appId: String, path: String) -> WKWebView? {
            RunnerBridge.resolveWebView(appId: appId, path: path)
        }

        public static func attach(_ webView: WKWebView, to container: NSView) {
            RunnerBridge.attachWebView(webView, to: container)
        }

        public static func attachLxApp(_ webView: WKWebView, to container: NSView) {
            RunnerBridge.attachLxAppWebView(webView, to: container)
        }

        public static func openBrowserTab(
            ownerAppId: String,
            ownerSessionId: UInt64,
            url: String
        ) -> String? {
            RunnerBridge.createBrowserTab(
                ownerAppId: ownerAppId,
                ownerSessionId: ownerSessionId,
                url: url
            )
        }

        public static func browserTabWebView(tabId: String) -> WKWebView? {
            RunnerBridge.browserTabWebView(tabId: tabId)
        }

        public static func closeBrowserTab(tabId: String) -> Bool {
            RunnerBridge.closeBrowserTab(tabId: tabId)
        }

        public static func handleAddressSubmission(
            rawInput: String,
            currentURL: String?,
            tabId: String
        ) -> (url: String, displayText: String)? {
            RunnerBridge.handleBrowserAddressSubmission(
                rawInput: rawInput,
                currentURL: currentURL,
                tabId: tabId
            )
        }
    }

    @MainActor
    public enum Tabs {
        public typealias Config = TabBar
        public static let stateChangedNotification = Notification.Name("TabBarDataChanged")

        public static func config(for appId: String) -> Config? {
            RunnerBridge.tabBar(appId: appId)
        }

        public static func isTransparent(_ colorValue: UInt32) -> Bool {
            RunnerBridge.isTabBarTransparent(colorValue)
        }

        public static func makeView(
            config: Config,
            appId: String,
            onSelect: @escaping (Int, String) -> Void
        ) -> NSView {
            RunnerBridge.makeTabBarView(config: config, appId: appId, onSelect: onSelect)
        }

        public static func refresh(_ view: NSView?) {
            RunnerBridge.refreshTabBarView(view)
        }

        public static func setSelectedIndex(
            _ view: NSView?,
            index: Int,
            notifyListener: Bool
        ) {
            RunnerBridge.setTabBarSelectedIndex(view, index: index, notifyListener: notifyListener)
        }
    }

    @MainActor
    public enum NavigationBars {
        public typealias State = NavigationBarState

        public static func state(
            appId: String,
            path: String
        ) -> State? {
            RunnerBridge.navigationBarState(appId: appId, path: path)
        }

        public static func updateState(appId: String, path: String) {
            RunnerBridge.updateNavigationBarState(appId: appId, path: path)
        }

        public static func currentState() -> State? {
            RunnerBridge.navigationBarCurrentState()
        }
    }

    @MainActor
    public enum Assets {
        public static func image(named name: String, size: CGSize? = nil) -> NSImage? {
            RunnerBridge.icon(named: name, size: size)
        }
    }

    @MainActor
    public enum CapsuleMenu {
        public static func show(appId: String) {
            RunnerBridge.showCapsuleMenu(appId: appId)
        }
    }

    @MainActor
    public enum SurfaceShell {
        public static func make(controller: LxAppController) -> LxAppShell {
            RunnerBridge.makeSurfaceShell(controller: controller)
        }

        public static func activate(_ shell: LxAppShell) {
            RunnerBridge.activateSurfaceShell(shell)
        }

        public static func open(
            _ shell: LxAppShell,
            appId: String,
            path: String,
            sessionId: UInt64
        ) {
            RunnerBridge.openInSurfaceShell(
                shell,
                appId: appId,
                path: path,
                sessionId: sessionId
            )
        }

        public static func navigate(
            _ shell: LxAppShell,
            appId: String,
            path: String,
            animationType: LxAppAnimation
        ) {
            RunnerBridge.navigateSurfaceShell(
                shell,
                appId: appId,
                path: path,
                animationType: animationType
            )
        }

        public static func presentBrowserTab(_ shell: LxAppShell, tabId: String) {
            RunnerBridge.presentBrowserTabInSurfaceShell(shell, tabId: tabId)
        }
    }
}

#else

/// Tooling-only SPI is macOS-only. The symbol still exists on non-macOS
/// targets so the package can compile for iOS without exposing any runner APIs.
@_spi(Runner) public enum LingxiaRunnerSPI {}

#endif
