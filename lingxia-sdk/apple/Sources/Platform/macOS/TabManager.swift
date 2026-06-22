#if os(macOS)
import SwiftUI
import Foundation

/// Represents a single tab in the tab-style tab bar
struct LxAppTab: Equatable, Identifiable {
    let appId: String
    /// When set, this lxapp is a companion (aside) opened via an activator/shell,
    /// not a main. Its sidebar entry toggles this surface instead of switching
    /// the main; `nil` means a normal main lxapp.
    var asideSurfaceId: String?
    var id: String { appId }

    var currentPath: String { "/" }
    var title: String { appId }
    var isMain: Bool { asideSurfaceId == nil }
    // Rust decides if tab is closable, Swift doesn't need to judge
    var isClosable: Bool { true }

    init(appId: String, asideSurfaceId: String? = nil) {
        self.appId = appId
        self.asideSurfaceId = asideSurfaceId
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

    func addTab(appId: String, asideSurfaceId: String? = nil, activate: Bool = true) {
        if tabs.contains(where: { $0.appId == appId }) {
            if activate { selectTab(appId: appId) }
            return
        }

        let newTab = LxAppTab(appId: appId, asideSurfaceId: asideSurfaceId)
        tabs.append(newTab)
        // Aside companions (activate == false) appear in the sidebar without
        // becoming the active main.
        if activate {
            activeTab = newTab
            onTabChanged?(newTab)
        }
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
