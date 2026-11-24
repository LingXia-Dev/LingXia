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

    static let appID = "app.lingxia.browser"
    static let settingsTabID = "settings"

    var page: String {
        switch self {
        case .settings, .clearSiteData:
            return "settings"
        case .downloads:
            return "downloads"
        case .bookmarks:
            return "bookmarks"
        case .history:
            return "history"
        }
    }

    var query: [String: String]? {
        switch self {
        case .clearSiteData(let tabID):
            return ["section": "clear-site-data", "tabId": tabID]
        case .settings, .downloads, .bookmarks, .history:
            return nil
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
