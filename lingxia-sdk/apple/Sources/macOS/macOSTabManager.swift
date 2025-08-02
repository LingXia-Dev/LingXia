#if os(macOS)
import Cocoa
import Foundation

/// Represents a single tab in the tab-style tab bar
/// Uses minimal data - only stores appId, everything else is computed
public struct LxAppTab: Equatable {
    public let appId: String

    /// Current path from LxApp instance
    public var currentPath: String {
        // Simple fallback for now - avoid async calls
        return "/"
    }

    /// Display title from app configuration
    public var title: String {
        return LxAppTabManager.getAppDisplayName(for: appId)
    }

    /// Home app is not closable
    public var isClosable: Bool {
        return !LxAppTabManager.isHomeApp(appId)
    }

    public init(appId: String) {
        self.appId = appId
    }
}

/// Manages tab-style tabs for LxApps with minimal interface
@MainActor
public class LxAppTabManager: ObservableObject {
    @Published public private(set) var tabs: [LxAppTab] = []
    @Published public private(set) var activeTabIndex: Int = 0

    /// Simple callback for tab changes - replaces complex delegate pattern
    public var onTabChanged: ((LxAppTab) -> Void)?

    /// Cached information to avoid async calls in computed properties
    nonisolated(unsafe) private static var homeAppId: String?
    nonisolated(unsafe) private static var appInfoCache: [String: (name: String, isHome: Bool)] = [:]

    /// Current active tab
    public var activeTab: LxAppTab? {
        tabs.indices.contains(activeTabIndex) ? tabs[activeTabIndex] : nil
    }

    /// Check if there are any tabs
    public var hasTabs: Bool {
        !tabs.isEmpty
    }

    public init() {
        // Initialize home app id cache
        Self.initializeHomeAppId()
    }

    /// Initialize home app id cache
    private static func initializeHomeAppId() {
        if homeAppId == nil {
            homeAppId = LxAppCore.getHomeLxAppId()
        }
    }

    /// Cache app info when adding tabs
    private static func cacheAppInfo(for appId: String) {
        guard appInfoCache[appId] == nil else { return }

        // Get app info from FFI
        let lxappInfo = getLxAppInfo(appId)
        let appName = lxappInfo.app_name.toString()
        let displayName = appName.isEmpty ? appId : appName
        let isHome = (appId == homeAppId)

        appInfoCache[appId] = (name: displayName, isHome: isHome)
    }

    /// Get cached app display name
    nonisolated static func getAppDisplayName(for appId: String) -> String {
        if let cached = appInfoCache[appId] {
            return cached.name
        }
        // Fallback to appId if not cached
        return appId
    }

    /// Check if app is home app
    nonisolated static func isHomeApp(_ appId: String) -> Bool {
        if let cached = appInfoCache[appId] {
            return cached.isHome
        }
        // Fallback check
        return appId == homeAppId
    }

    /// Add a new tab or switch to existing one
    public func addTab(appId: String) {
        // Cache app info first
        Self.cacheAppInfo(for: appId)

        // Check if tab already exists
        if let index = tabs.firstIndex(where: { $0.appId == appId }) {
            selectTab(at: index)
            return
        }

        // Create new tab
        let newTab = LxAppTab(appId: appId)
        tabs.append(newTab)

        // Always activate newly added tabs (Chrome behavior)
        activeTabIndex = tabs.count - 1
        onTabChanged?(newTab)
    }

    /// Select a tab by index
    public func selectTab(at index: Int) {
        guard tabs.indices.contains(index) else { return }
        activeTabIndex = index
        onTabChanged?(tabs[index])
    }

    /// Select a tab by appId
    public func selectTab(appId: String) {
        if let index = tabs.firstIndex(where: { $0.appId == appId }) {
            selectTab(at: index)
        }
    }

    /// Close a tab by appId
    public func closeTab(appId: String) {
        guard let index = tabs.firstIndex(where: { $0.appId == appId }) else { return }
        let tab = tabs[index]
        guard tab.isClosable else { return }

        let wasActive = (index == activeTabIndex)
        tabs.remove(at: index)

        // Adjust active index if needed
        if wasActive && !tabs.isEmpty {
            activeTabIndex = min(index, tabs.count - 1)
            onTabChanged?(tabs[activeTabIndex])
        } else if tabs.isEmpty {
            activeTabIndex = 0
        } else if index <= activeTabIndex {
            activeTabIndex = max(0, activeTabIndex - 1)
        }
    }

    /// Check if tabs should be visible (more than just home tab)
    public var shouldShowTabs: Bool {
        // Simple check without async call - assume we want to show tabs if we have multiple
        return tabs.count > 1
    }
}

#endif
