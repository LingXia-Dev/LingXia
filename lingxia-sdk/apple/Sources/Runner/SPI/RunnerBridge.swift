import Foundation

#if os(macOS)
import AppKit
import WebKit
import CLingXiaRustAPI
import CLingXiaSwiftAPI

@MainActor
enum RunnerBridge {
    static func sessionId(for appId: String) -> UInt64? {
        LxAppCore.sessionId(for: appId)
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

    static func findWebView(appId: String, path: String, sessionId: UInt64) -> WKWebView? {
        WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId)
    }

    static func findWebView(appId: String, path: String) -> WKWebView? {
        WebViewManager.findWebView(appId: appId, path: path)
    }

    static func attachWebView(_ webView: WKWebView, to container: NSView) {
        WebViewManager.attachWebViewToContainer(webView, container: container)
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

    static func showCapsuleMenu(appId: String) {
        LxAppCapsuleMenu.show(appId: appId)
    }
}
#endif
