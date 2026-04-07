import AppKit
import WebKit

public typealias RunnerTabBarConfig = TabBar
public typealias RunnerNavigationBarState = NavigationBarState

public enum RunnerSupport {
    @MainActor
    public enum Runtime {
        public static func sessionId(for appId: String) -> UInt64? {
            RunnerBridge.sessionId(for: appId)
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

        public static func homeLxAppId() -> String? {
            RunnerBridge.homeLxAppId()
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

        public static func find(
            appId: String,
            path: String,
            sessionId: UInt64
        ) -> WKWebView? {
            RunnerBridge.findWebView(appId: appId, path: path, sessionId: sessionId)
        }

        public static func find(appId: String, path: String) -> WKWebView? {
            RunnerBridge.findWebView(appId: appId, path: path)
        }

        public static func attach(_ webView: WKWebView, to container: NSView) {
            RunnerBridge.attachWebView(webView, to: container)
        }
    }

    @MainActor
    public enum TabBar {
        public static let stateChangedNotification = Notification.Name("TabBarDataChanged")

        public static func config(for appId: String) -> RunnerTabBarConfig? {
            RunnerBridge.tabBar(appId: appId)
        }

        public static func isTransparent(_ colorValue: UInt32) -> Bool {
            RunnerBridge.isTabBarTransparent(colorValue)
        }

        public static func makeView(
            config: RunnerTabBarConfig,
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
    public enum Navigation {
        public static func state(
            appId: String,
            path: String
        ) -> RunnerNavigationBarState? {
            RunnerBridge.navigationBarState(appId: appId, path: path)
        }

        public static func updateState(appId: String, path: String) {
            RunnerBridge.updateNavigationBarState(appId: appId, path: path)
        }

        public static func currentState() -> RunnerNavigationBarState? {
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
}
