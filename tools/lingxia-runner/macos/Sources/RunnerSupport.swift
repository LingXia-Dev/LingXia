import AppKit
import WebKit
@_spi(Runner) import lingxia

typealias RunnerTabBarConfig = TabBar
typealias RunnerNavigationBarState = NavigationBarState

enum RunnerSupport {
    @MainActor
    enum Runtime {
        static func setOpenUrlHandler(_ handler: @escaping (String, UInt64, String) -> Bool) {
            LingxiaRunnerSPI.Runtime.setOpenUrlHandler(handler)
        }

        static func sessionId(for appId: String) -> UInt64? {
            LingxiaRunnerSPI.Runtime.sessionId(for: appId)
        }

        static func currentAppId() -> String? {
            LingxiaRunnerSPI.Runtime.currentAppId()
        }

        static func currentPath() -> String {
            LingxiaRunnerSPI.Runtime.currentPath()
        }

        static func setSessionId(_ sessionId: UInt64, for appId: String) {
            LingxiaRunnerSPI.Runtime.setSessionId(sessionId, for: appId)
        }

        static func removeSessionId(for appId: String) {
            LingxiaRunnerSPI.Runtime.removeSessionId(for: appId)
        }

        static func setCurrentApp(appId: String, path: String) {
            LingxiaRunnerSPI.Runtime.setCurrentApp(appId: appId, path: path)
        }

        static func setCurrentPath(_ path: String) {
            LingxiaRunnerSPI.Runtime.setCurrentPath(path)
        }
    }

    @MainActor
    enum WebView {
        static func current() -> WKWebView? {
            LingxiaRunnerSPI.WebView.current()
        }

        static func removeCurrentFromSuperview() {
            LingxiaRunnerSPI.WebView.removeCurrentFromSuperview()
        }

        static func resolve(appId: String, path: String, sessionId: UInt64) -> WKWebView? {
            LingxiaRunnerSPI.WebView.resolve(appId: appId, path: path, sessionId: sessionId)
        }

        static func resolve(appId: String, path: String) -> WKWebView? {
            LingxiaRunnerSPI.WebView.resolve(appId: appId, path: path)
        }

        static func attach(_ webView: WKWebView, to container: NSView) {
            LingxiaRunnerSPI.WebView.attach(webView, to: container)
        }

        static func attachLxApp(_ webView: WKWebView, to container: NSView) {
            LingxiaRunnerSPI.WebView.attachLxApp(webView, to: container)
        }

    }

    @MainActor
    enum Browser {
        static func openTab(ownerAppId: String, ownerSessionId: UInt64, url: String) -> String? {
            LingxiaRunnerSPI.WebView.openBrowserTab(
                ownerAppId: ownerAppId,
                ownerSessionId: ownerSessionId,
                url: url
            )
        }

        static func webView(tabId: String) -> WKWebView? {
            LingxiaRunnerSPI.WebView.browserTabWebView(tabId: tabId)
        }

        static func closeTab(tabId: String) -> Bool {
            LingxiaRunnerSPI.WebView.closeBrowserTab(tabId: tabId)
        }

        static func handleAddressSubmission(
            rawInput: String,
            currentURL: String?,
            tabId: String
        ) -> (url: String, displayText: String)? {
            LingxiaRunnerSPI.WebView.handleAddressSubmission(
                rawInput: rawInput,
                currentURL: currentURL,
                tabId: tabId
            )
        }
    }

    @MainActor
    enum TabBar {
        static let stateChangedNotification = LingxiaRunnerSPI.Tabs.stateChangedNotification

        static func config(for appId: String) -> RunnerTabBarConfig? {
            LingxiaRunnerSPI.Tabs.config(for: appId)
        }

        static func isTransparent(_ colorValue: UInt32) -> Bool {
            LingxiaRunnerSPI.Tabs.isTransparent(colorValue)
        }

        static func makeView(
            config: RunnerTabBarConfig,
            appId: String,
            onSelect: @escaping (Int, String) -> Void
        ) -> NSView {
            LingxiaRunnerSPI.Tabs.makeView(config: config, appId: appId, onSelect: onSelect)
        }

        static func refresh(_ view: NSView?) {
            LingxiaRunnerSPI.Tabs.refresh(view)
        }

        static func setSelectedIndex(_ view: NSView?, index: Int, notifyListener: Bool) {
            LingxiaRunnerSPI.Tabs.setSelectedIndex(view, index: index, notifyListener: notifyListener)
        }
    }

    @MainActor
    enum Navigation {
        static func state(appId: String, path: String) -> RunnerNavigationBarState? {
            LingxiaRunnerSPI.NavigationBars.state(appId: appId, path: path)
        }

        static func updateState(appId: String, path: String) {
            LingxiaRunnerSPI.NavigationBars.updateState(appId: appId, path: path)
        }

        static func currentState() -> RunnerNavigationBarState? {
            LingxiaRunnerSPI.NavigationBars.currentState()
        }
    }

    @MainActor
    enum Assets {
        static func image(named name: String, size: CGSize? = nil) -> NSImage? {
            LingxiaRunnerSPI.Assets.image(named: name, size: size)
        }
    }

    @MainActor
    enum SurfaceShell {
        static func make(controller: LxAppController) -> LxAppShell {
            LingxiaRunnerSPI.SurfaceShell.make(controller: controller)
        }

        static func activate(_ shell: LxAppShell) {
            LingxiaRunnerSPI.SurfaceShell.activate(shell)
        }

        static func open(
            _ shell: LxAppShell,
            appId: String,
            path: String,
            sessionId: UInt64
        ) {
            LingxiaRunnerSPI.SurfaceShell.open(
                shell,
                appId: appId,
                path: path,
                sessionId: sessionId
            )
        }

        static func navigate(
            _ shell: LxAppShell,
            appId: String,
            path: String,
            animationType: LxAppAnimation
        ) {
            LingxiaRunnerSPI.SurfaceShell.navigate(
                shell,
                appId: appId,
                path: path,
                animationType: animationType
            )
        }

        static func presentBrowserTab(_ shell: LxAppShell, tabId: String) {
            LingxiaRunnerSPI.SurfaceShell.presentBrowserTab(shell, tabId: tabId)
        }
    }
}
