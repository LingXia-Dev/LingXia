import Foundation

/// Localization helper for LingXia SDK
/// Usage: L10n.string("lx_ui_back") or "lx_ui_back".localized
enum L10n {
    /// Get localized string from LingXia SDK bundle
    static func string(_ key: String) -> String {
        return NSLocalizedString(key, bundle: Bundle.module, comment: "")
    }

    /// Get localized string with format arguments
    static func string(_ key: String, _ arguments: CVarArg...) -> String {
        let format = NSLocalizedString(key, bundle: Bundle.module, comment: "")
        return String(format: format, arguments: arguments)
    }
}

extension String {
    /// Localize string using LingXia SDK bundle
    var localized: String {
        return NSLocalizedString(self, bundle: Bundle.module, comment: "")
    }

    /// Localize string with format arguments
    func localized(_ arguments: CVarArg...) -> String {
        let format = NSLocalizedString(self, bundle: Bundle.module, comment: "")
        return String(format: format, arguments: arguments)
    }
}
