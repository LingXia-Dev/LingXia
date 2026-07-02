import Foundation

#if os(macOS)
import AppKit
import OSLog
import WebKit
import CLingXiaRustAPI
import CLingXiaSwiftAPI

@MainActor
enum RunnerBridge {
    private static let log = OSLog(subsystem: "LingXiaRunner", category: "RunnerBridge")

    static func setOpenUrlHandler(_ handler: @escaping (String, UInt64, String, Bool) -> Bool) {
        LxApp.openUrlHandler = { ownerAppId, ownerSessionId, url, target in
            switch target {
            case .selfTarget, .newBrowserTab, .asideBrowser:
                // The phone simulator has a single in-app browser surface, so an
                // in-page new-tab request (target="_blank" / window.open) opens
                // another tab in it, just like a `target="self"` navigation.
                // An aside open marks its tab so the chrome hides the address bar.
                return .handled(
                    handler(ownerAppId, ownerSessionId, url, target == .asideBrowser))
            case .external:
                return .useDefault
            }
        }
    }

    static func sessionId(for appId: String) -> UInt64? {
        LxAppCore.sessionId(for: appId)
    }

    static func currentAppId() -> String? {
        LxAppCore.currentAppId
    }

    static func currentPath() -> String {
        LxAppCore.getCurrentPath()
    }

    static func setSessionId(_ sessionId: UInt64, for appId: String) {
        LxAppCore.setSessionId(sessionId, for: appId)
    }

    static func removeSessionId(for appId: String) {
        LxAppCore.removeSessionId(for: appId)
    }

    static func setCurrentApp(appId: String, path: String) {
        LxAppCore.setCurrentApp(appId: appId, path: path)
    }

    static func setCurrentPath(_ path: String) {
        LxAppCore.setCurrentPath(path)
    }

    static func currentWebView() -> WKWebView? {
        LxAppCore.getCurrentWebView()
    }

    static func removeCurrentWebViewFromSuperview() {
        LxAppCore.getCurrentWebView()?.removeFromSuperview()
    }

    static func homeLxAppId() -> String? {
        LxAppCore.getHomeLxAppId()
    }

    static func resolveWebView(appId: String, path: String, sessionId: UInt64) -> WKWebView? {
        WebViewManager.resolveWebView(appId: appId, path: path, sessionId: sessionId)
    }

    static func resolveWebView(appId: String, path: String) -> WKWebView? {
        WebViewManager.resolveWebView(appId: appId, path: path)
    }

    static func attachWebView(_ webView: WKWebView, to container: NSView) {
        WebViewManager.attachWebViewToContainer(webView, container: container)
    }

    static func attachLxAppWebView(_ webView: WKWebView, to container: NSView) {
        WebViewManager.attachWebViewToContainer(webView, container: container)
        MacNativeBridge.attachIfNeeded(to: webView, in: container)
        webView.resumeWebView()
    }

    static func createBrowserTab(
        ownerAppId: String,
        ownerSessionId: UInt64,
        url: String,
        aside: Bool = false
    ) -> String? {
        let openedTab = aside
            ? openAsideBrowserTab(ownerAppId, ownerSessionId, url)
            : openBrowserTab(ownerAppId, ownerSessionId, url)
        guard let openedTab else {
            return nil
        }
        let tabId = openedTab.toString().trimmingCharacters(in: .whitespacesAndNewlines)
        return tabId.isEmpty ? nil : tabId
    }

    static func browserTabIsAside(tabId: String) -> Bool {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        return lingxia.browserTabIsAside(normalized)
    }

    static func builtinBrowserAppId() -> String {
        getBuiltinBrowserAppId().toString()
    }

    static func browserTabWebView(tabId: String) -> WKWebView? {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return nil }

        let appId = getBuiltinBrowserAppId().toString()
        let sessionId = getLxAppSessionId(appId)
        guard sessionId > 0 else { return nil }

        let path = browserTabPathForId(normalized).toString()
        guard !path.isEmpty else { return nil }
        return WebViewManager.resolveWebView(appId: appId, path: path, sessionId: sessionId)
    }

    static func navigateBrowserTab(tabId: String, url: String) -> Bool {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        return browserTabNavigate(normalized, url)
    }

    static func closeBrowserTab(tabId: String) -> Bool {
        let normalized = tabId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else { return false }
        return browserTabClose(normalized)
    }

    static func handleBrowserAddressSubmission(
        rawInput: String,
        currentURL: String?,
        tabId: String
    ) -> (url: String, displayText: String)? {
        guard let result = lingxia.handleBrowserAddressSubmission(
            rawInput: rawInput,
            currentURL: currentURL,
            tabId: tabId
        ) else {
            return nil
        }
        return (url: result.url, displayText: result.displayText)
    }

    static func tabBar(appId: String) -> TabBar? {
        getTabBar(appId)
    }

    static func isTabBarTransparent(_ colorValue: UInt32) -> Bool {
        TabBarHelper.isTransparent(colorValue)
    }

    static func makeTabBarView(
        config: TabBar,
        appId: String,
        onSelect: @escaping (Int, String) -> Void
    ) -> NSView {
        let tabBar = LingXiaTabBar()
        tabBar.initialize(config: config, appId: appId)
        tabBar.setOnTabSelectedListener(onSelect)
        tabBar.setSelectedIndex(Int(config.selected_index), notifyListener: false)
        tabBar.translatesAutoresizingMaskIntoConstraints = false
        return tabBar
    }

    static func refreshTabBarView(_ view: NSView?) {
        (view as? LingXiaTabBar)?.refreshLayout()
    }

    static func setTabBarSelectedIndex(
        _ view: NSView?,
        index: Int,
        notifyListener: Bool
    ) {
        (view as? LingXiaTabBar)?.setSelectedIndex(index, notifyListener: notifyListener)
    }

    static func navigationBarState(appId: String, path: String) -> NavigationBarState? {
        getNavigationBarState(appId, path)
    }

    static func updateNavigationBarState(appId: String, path: String) {
        NavigationBarStateManager.shared.updateState(appId: appId, path: path)
    }

    static func navigationBarCurrentState() -> NavigationBarState? {
        NavigationBarStateManager.shared.currentState
    }

    static func icon(named name: String, size: CGSize? = nil) -> NSImage? {
        LxIcon.image(named: name, size: size)
    }

    static func makeSurfaceShell(controller: LxAppController) -> LxAppShell {
        let configuration = Lingxia.resolvedShellConfiguration(
            from: LxAppShellConfiguration(),
            capabilities: LxAppCapabilities(rawValue: LxAppCore.capabilities),
            homeAppId: LxAppCore.getHomeLxAppId()
        )
        let shell = LxAppShell(
            controller: controller,
            configuration: configuration,
            startupBehavior: .manual
        )
        shell.reconcileSidebarAutoHide()
        return shell
    }

    static func activateSurfaceShell(_ shell: LxAppShell) {
        LxAppActiveHost.activate(shell: shell)
    }

    static func setSurfaceShellTrafficLightsVisible(_ shell: LxAppShell, visible: Bool) {
        shell.setTrafficLightsVisible(visible)
    }

    static func setSurfaceShellTopAccessory(_ shell: LxAppShell, view: NSView?, height: CGFloat) {
        shell.setTopAccessory(view, height: height)
    }

    static func openInSurfaceShell(
        _ shell: LxAppShell,
        appId: String,
        path: String,
        sessionId: UInt64
    ) {
        shell.openLxApp(appId: appId, path: path, sessionId: sessionId)
        shell.reconcileSidebarAutoHide()
    }

    static func navigateSurfaceShell(
        _ shell: LxAppShell,
        appId: String,
        path: String,
        animationType: LxAppAnimation
    ) {
        shell.browserCoordinator.deactivate()
        shell.ensureViewController(for: appId, path: path)?
            .navigate(appId: appId, to: path, with: animationType)
        shell.reconcileSidebarAutoHide()
    }

    static func presentBrowserTabInSurfaceShell(_ shell: LxAppShell, tabId: String) {
        shell.presentInternalBrowserTab(id: tabId)
        shell.reconcileSidebarAutoHide()
        shell.window?.makeKeyAndOrderFront(nil)
    }
}
#endif
