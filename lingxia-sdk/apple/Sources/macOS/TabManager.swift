#if os(macOS)
import SwiftUI
import Foundation

/// Represents a single tab in the tab-style tab bar
public struct LxAppTab: Equatable, Identifiable {
    public let appId: String
    public var id: String { appId }

    public var currentPath: String { "/" }
    public var title: String { appId }
    public var isClosable: Bool { appId != "homeminiapp" }

    public init(appId: String) {
        self.appId = appId
    }
}

/// Manages tab-style tabs for LxApps
@MainActor
public class LxAppTabManager: ObservableObject {
    public static let shared = LxAppTabManager()

    @Published public var tabs: [LxAppTab] = []
    @Published public var activeTab: LxAppTab?
    public var onTabChanged: ((LxAppTab) -> Void)?

    private init() {}

    public var hasTabs: Bool { !tabs.isEmpty }

    public func addTab(appId: String) {
        if tabs.contains(where: { $0.appId == appId }) {
            selectTab(appId: appId)
            return
        }

        let newTab = LxAppTab(appId: appId)
        tabs.append(newTab)
        activeTab = newTab
        onTabChanged?(newTab)
    }

    public func selectTab(appId: String) {
        guard let tab = tabs.first(where: { $0.appId == appId }) else { return }
        activeTab = tab
        onTabChanged?(tab)
    }

    public func closeTab(appId: String) {
        guard let index = tabs.firstIndex(where: { $0.appId == appId }),
              tabs[index].isClosable else { return }

        let wasActive = activeTab?.appId == appId
        tabs.remove(at: index)

        if wasActive {
            activeTab = tabs.first
            if let newActive = activeTab {
                onTabChanged?(newActive)
            }
        }
    }

    public func clearAllTabs() {
        tabs.removeAll()
        activeTab = nil
    }

    public func hasTab(for appId: String) -> Bool {
        tabs.contains { $0.appId == appId }
    }
}

#endif
