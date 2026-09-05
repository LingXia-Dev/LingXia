#if os(macOS)
import Foundation

enum BrowserTabOpenAuthority: Equatable {
    case appSession
    case nativeControl
}

/// Browser-owned internal navigation. These routes belong to browser chrome;
/// they are not host-wide Settings destination resolution.
enum BrowserLocalNavigation: Equatable {
    case settings
    case clearSiteData(tabID: String)
    case downloads
    case bookmarks
    case history

    static let settingsTabID = "settings"

    var url: String {
        switch self {
        case .settings:
            return "lingxia://settings"
        case .clearSiteData(let tabID):
            return "lingxia://settings#clear-site-data?tabId=\(tabID)"
        case .downloads:
            return "lingxia://downloads"
        case .bookmarks:
            return "lingxia://bookmarks"
        case .history:
            return "lingxia://history"
        }
    }

    var stableTabID: String {
        switch self {
        case .settings, .clearSiteData:
            return Self.settingsTabID
        case .downloads:
            return "downloads"
        case .bookmarks:
            return "bookmarks"
        case .history:
            return "history"
        }
    }

    var openAuthority: BrowserTabOpenAuthority { .nativeControl }
}
#endif
