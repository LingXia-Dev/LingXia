import Foundation

/// Localization helper for LingXia SDK. Public so host apps can reuse the
/// SDK's shared string vocabulary (e.g. common menu/action labels) when
/// building their own native chrome instead of duplicating hardcoded text.
/// Usage: L10n.string("lx_ui_back") or "lx_ui_back".localized
public enum L10n {
    /// Get localized string from LingXia SDK bundle
    public static func string(_ key: String) -> String {
        return NSLocalizedString(key, bundle: Bundle.lingxiaResources, comment: "")
    }

    /// Get localized string with format arguments
    public static func string(_ key: String, _ arguments: CVarArg...) -> String {
        let format = NSLocalizedString(key, bundle: Bundle.lingxiaResources, comment: "")
        return String(format: format, arguments: arguments)
    }
}

extension String {
    /// Localize string using LingXia SDK bundle
    var localized: String {
        return NSLocalizedString(self, bundle: Bundle.lingxiaResources, comment: "")
    }

    /// Localize string with format arguments
    func localized(_ arguments: CVarArg...) -> String {
        let format = NSLocalizedString(self, bundle: Bundle.lingxiaResources, comment: "")
        return String(format: format, arguments: arguments)
    }
}
