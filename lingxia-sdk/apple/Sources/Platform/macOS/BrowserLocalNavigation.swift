#if os(macOS)
import Foundation

/// Browser-owned internal navigation. These routes belong to browser chrome;
/// they are not host-wide Settings destination resolution.
enum BrowserLocalNavigation: Equatable {
    case settings
    case clearSiteData(tabID: String)

    static let settingsTabID = "settings"

    var url: String {
        switch self {
        case .settings:
            return "lingxia://settings"
        case .clearSiteData(let tabID):
            return "lingxia://settings#clear-site-data?tabId=\(tabID)"
        }
    }

    var stableTabID: String { Self.settingsTabID }
}
#endif
