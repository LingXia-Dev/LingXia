#if os(macOS)
import SwiftUI
import Foundation

/// Represents a single tab in the tab-style tab bar
struct LxAppTab: Equatable, Identifiable {
    let appId: String
    var id: String { appId }

    var currentPath: String { "/" }
    var title: String { appId }
    // Rust decides if tab is closable, Swift doesn't need to judge
    var isClosable: Bool { true }

    init(appId: String) {
        self.appId = appId
    }
}

/// Manages tab UI state for LxApps
@MainActor
class LxAppTabManager: ObservableObject {
    static let shared = LxAppTabManager()

    @Published public var tabs: [LxAppTab] = []
    @Published public var activeTab: LxAppTab?
    var onTabChanged: ((LxAppTab) -> Void)?
    var onTabsChanged: (([LxAppTab]) -> Void)?

    private init() {}

    var hasTabs: Bool { !tabs.isEmpty }

    func addTab(appId: String) {
        if tabs.contains(where: { $0.appId == appId }) {
            selectTab(appId: appId)
            return
        }

        let newTab = LxAppTab(appId: appId)
        tabs.append(newTab)
        activeTab = newTab
        onTabChanged?(newTab)
        onTabsChanged?(tabs)
    }

    func selectTab(appId: String) {
        guard let tab = tabs.first(where: { $0.appId == appId }) else { return }
        activeTab = tab
        onTabChanged?(tab)
    }

    func closeTab(appId: String) {
        guard let index = tabs.firstIndex(where: { $0.appId == appId }) else { return }

        // Rust decides if tab is closable, Swift just removes it
        let wasActive = activeTab?.appId == appId
        tabs.remove(at: index)

        if wasActive {
            activeTab = tabs.first
            if let newActive = activeTab {
                onTabChanged?(newActive)
            }
        }
        onTabsChanged?(tabs)
    }

    func hasTab(for appId: String) -> Bool {
        tabs.contains { $0.appId == appId }
    }
}

#endif
