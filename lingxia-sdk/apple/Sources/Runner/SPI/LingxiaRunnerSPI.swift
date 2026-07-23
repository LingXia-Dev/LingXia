import Foundation

#if os(macOS)
import AppKit
import WebKit

/// Tooling-only SPI consumed by `tools/lingxia-runner`.
@_spi(Runner) public enum LingxiaRunnerSPI {
    @MainActor
    public enum Runtime {
        /// Handler receives `(ownerAppId, ownerSessionId, url, aside)`; `aside`
        /// marks an `{ url, as: 'aside' }` open for compact one-row chrome.
        public static func setOpenUrlHandler(
            _ handler: @escaping (String, UInt64, String, Bool) -> Bool
        ) {
            RunnerBridge.setOpenUrlHandler(handler)
        }

        /// Handler receives `(appId, path, refreshing)` when the Runner owns
        /// the lxapp surface instead of the SDK's standard host controller.
        public static func setPullDownRefreshHandler(
            _ handler: @escaping (String, String, Bool) -> Bool
        ) {
            RunnerBridge.setPullDownRefreshHandler(handler)
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

        @MainActor
        @discardableResult
        public static func removeShellTab(for appId: String) -> String? {
            RunnerBridge.removeShellTab(for: appId)
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
            // Bound floating surfaces (e.g. the cloud login sheet) to the lxapp's
            // render container so they stay within the device frame.
            LxAppSurface.hostAnchorView = container
            RunnerBridge.attachLxAppWebView(webView, to: container)
        }

        public static func configureUserAgentOverride(
            _ userAgent: String?,
            reloadExisting: Bool
        ) -> Bool {
            RunnerBridge.configureUserAgentOverride(
                userAgent,
                reloadExisting: reloadExisting
            )
        }

        /// App id of the built-in browser, which owns in-page new-tab requests
        /// (`target="_blank"` / `window.open`).
        public static var builtinBrowserAppId: String {
            RunnerBridge.builtinBrowserAppId()
        }

        public static func openBrowserTab(
            ownerAppId: String,
            ownerSessionId: UInt64,
            url: String,
            aside: Bool = false
        ) -> String? {
            RunnerBridge.createBrowserTab(
                ownerAppId: ownerAppId,
                ownerSessionId: ownerSessionId,
                url: url,
                aside: aside
            )
        }

        /// Opens a tab in the managed browser group without an lxapp owner.
        public static func openBrowserTab(url: String) -> String? {
            RunnerBridge.createUnownedBrowserTab(url: url)
        }

        /// Whether the tab belongs to the API-managed aside browser group.
        public static func browserTabIsAside(tabId: String) -> Bool {
            RunnerBridge.browserTabIsAside(tabId: tabId)
        }

        public static func browserTabIds() -> [String] {
            RunnerBridge.browserTabIds()
        }

        public static func browserCurrentTabId() -> String? {
            RunnerBridge.browserCurrentTabId()
        }

        public static func browserTabWebView(tabId: String) -> WKWebView? {
            RunnerBridge.browserTabWebView(tabId: tabId)
        }

        /// Navigate a browser tab via the managed browser runtime (tracks the
        /// tab URL and applies navigation policy) — unlike a raw `WKWebView.load`.
        public static func navigateBrowserTab(tabId: String, url: String) -> Bool {
            RunnerBridge.navigateBrowserTab(tabId: tabId, url: url)
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

        /// Signal that the runner chrome finished applying a tabbar state
        /// change; resolves any awaited lx.showTabBar/hideTabBar.
        @MainActor
        public static func updateApplied(appId: String) {
            TabBarUpdateWaiters.complete(appId)
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
        public static let stateChangedNotification = Notification.Name("NavBarDataChanged")

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
    public enum SurfaceShell {
        public static func make(controller: LxAppController) -> LxAppShell {
            RunnerBridge.makeSurfaceShell(controller: controller)
        }

        public static func activate(_ shell: LxAppShell) {
            RunnerBridge.activateSurfaceShell(shell)
        }

        public static func setTrafficLightsVisible(_ shell: LxAppShell, visible: Bool) {
            RunnerBridge.setSurfaceShellTrafficLightsVisible(shell, visible: visible)
        }

        public static func setTopAccessory(_ shell: LxAppShell, view: NSView?, height: CGFloat) {
            RunnerBridge.setSurfaceShellTopAccessory(shell, view: view, height: height)
        }

        public static func setBrowserPageActionsVisible(
            _ shell: LxAppShell,
            visible: Bool
        ) {
            RunnerBridge.setSurfaceShellBrowserPageActionsVisible(shell, visible: visible)
        }

        public static func setBrowserRootVisible(_ shell: LxAppShell, visible: Bool) {
            RunnerBridge.setSurfaceShellBrowserRootVisible(shell, visible: visible)
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
