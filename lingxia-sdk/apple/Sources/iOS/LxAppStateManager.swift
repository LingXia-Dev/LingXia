import Foundation
import WebKit
import os.log

@MainActor
public class LxAppStateManager {

    public static let shared = LxAppStateManager()
    private init() {}

    private static let log = OSLog(subsystem: "LingXia", category: "StateManager")

    public struct LxAppState {
        let appId: String
        var currentPath: String
        var initialPath: String
        var isDisplayingHomeLxApp: Bool
        var hasInitializedTransparency: Bool
        var isWebViewLoading: Bool
        var navigationHistory: [String]
        var currentHistoryIndex: Int
        var webView: WKWebView?
        var webViewTopConstraint: NSLayoutConstraint?
        var navigationTitle: String = ""
        var showBackButton: Bool = false

        init(appId: String, path: String) {
            self.appId = appId
            self.currentPath = path
            self.initialPath = path
            self.isDisplayingHomeLxApp = false
            self.hasInitializedTransparency = false
            self.isWebViewLoading = false
            self.navigationHistory = [path]
            self.currentHistoryIndex = 0
        }

        mutating func pushToHistory(_ path: String) {
            // Remove any forward history if we're not at the end
            if currentHistoryIndex < navigationHistory.count - 1 {
                navigationHistory = Array(navigationHistory[0...currentHistoryIndex])
            }
            navigationHistory.append(path)
            currentHistoryIndex = navigationHistory.count - 1
            currentPath = path
        }

        mutating func popFromHistory() -> String? {
            guard currentHistoryIndex > 0 else { return nil }
            currentHistoryIndex -= 1
            currentPath = navigationHistory[currentHistoryIndex]
            return currentPath
        }
    }

    private var lxappStates: [String: LxAppState] = [:]
    public private(set) var currentAppId: String?

    public func createOrUpdateState(appId: String, path: String) -> LxAppState {
        if var existingState = lxappStates[appId] {
            existingState.currentPath = path
            lxappStates[appId] = existingState
            return existingState
        } else {
            var newState = LxAppState(appId: appId, path: path)
            // Set home app flag in main actor context
            newState.isDisplayingHomeLxApp = LxAppCore.isHomeLxApp(appId)
            lxappStates[appId] = newState
            os_log("Created new LxApp state for %@", log: Self.log, type: .info, appId)
            return newState
        }
    }

    /// Updates app state for navigation
    public func updateStateForNavigation(appId: String, path: String, navigationType: NavigationType) {
        guard var appState = lxappStates[appId] else {
            os_log("No state found for appId: %@", log: Self.log, type: .error, appId)
            return
        }

        switch navigationType {
        case .launch, .replace, .switchTab:
            appState.currentPath = path
        case .forward:
            appState.pushToHistory(path)
        case .backward:
            _ = appState.popFromHistory()
        }

        lxappStates[appId] = appState
        os_log("Updated state for %@ with navigation type: %@", log: Self.log, type: .debug, appId, String(describing: navigationType))
    }

    /// Gets current state for app
    public func getState(for appId: String) -> LxAppState? {
        return lxappStates[appId]
    }

    /// Sets current active app
    public func setCurrentApp(_ appId: String) {
        currentAppId = appId
        os_log("Set current app to: %@", log: Self.log, type: .info, appId)
    }

    /// Removes state for app
    public func removeState(for appId: String) {
        lxappStates.removeValue(forKey: appId)
        if currentAppId == appId {
            currentAppId = nil
        }
        os_log("Removed state for: %@", log: Self.log, type: .info, appId)
    }

    /// Gets all active app IDs
    public var activeAppIds: [String] {
        return Array(lxappStates.keys)
    }

    /// Checks if app should use transparent mode
    public func shouldUseTransparentMode(for appId: String, with tabBar: LingXiaTabBar?) -> Bool {
        guard let tabBar = tabBar,
              let config = tabBar.config else {
            return false
        }
        return TabBar.isTransparent(config.background_color)
    }

    /// Gets current path for app
    public func getCurrentPath(for appId: String) -> String? {
        return lxappStates[appId]?.currentPath
    }

    /// Updates current path for app
    public func updateCurrentPath(_ path: String, for appId: String) {
        lxappStates[appId]?.currentPath = path
    }

    /// Updates WebView reference
    public func updateWebView(_ webView: WKWebView?, for appId: String) {
        lxappStates[appId]?.webView = webView
    }

    /// Updates WebView constraint
    public func updateWebViewConstraint(_ constraint: NSLayoutConstraint?, for appId: String) {
        lxappStates[appId]?.webViewTopConstraint = constraint
    }
}
