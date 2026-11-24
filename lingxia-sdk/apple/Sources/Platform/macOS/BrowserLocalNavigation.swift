#if os(macOS)
import Foundation

/// Browser system pages opened from browser chrome. Rust opens them as the
/// `app.lingxia.browser` control surface; they are not host-wide Settings.
enum BrowserLocalNavigation: Equatable {
    case settings
    case clearSiteData(tabID: String)
    case downloads
    case bookmarks
    case history

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

    /// The page query as a JSON object, or empty for none.
    var queryJSON: String {
        guard case .clearSiteData(let tabID) = self,
              let data = try? JSONSerialization.data(
                  withJSONObject: ["section": "clear-site-data", "tabId": tabID],
                  options: [.sortedKeys]
              ),
              let json = String(data: data, encoding: .utf8)
        else { return "" }
        return json
    }
}
#endif
