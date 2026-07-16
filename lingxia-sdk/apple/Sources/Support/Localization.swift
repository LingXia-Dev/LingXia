import Foundation

/// Localization helper for LingXia SDK. Public so host apps can reuse the
/// SDK's shared string vocabulary (e.g. common menu/action labels) when
/// building their own native chrome instead of duplicating hardcoded text.
/// Usage: L10n.string("lx_ui_back") or "lx_ui_back".localized
public enum L10n {
    private static var localizedBundle: Bundle {
        let locale = Lingxia.displayLanguage.replacingOccurrences(of: "_", with: "-")
        let localization = locale.lowercased().hasPrefix("zh") ? "zh-Hans" : "en"
        guard let path = Bundle.lingxiaResources.path(
            forResource: localization,
            ofType: "lproj"
        ), let bundle = Bundle(path: path) else {
            return Bundle.lingxiaResources
        }
        return bundle
    }

    /// Get localized string from LingXia SDK bundle
    public static func string(_ key: String) -> String {
        return NSLocalizedString(key, bundle: localizedBundle, comment: "")
    }

    /// Get localized string with format arguments
    public static func string(_ key: String, _ arguments: CVarArg...) -> String {
        return string(key, arguments: arguments)
    }

    static func string(_ key: String, arguments: [CVarArg]) -> String {
        let format = NSLocalizedString(key, bundle: localizedBundle, comment: "")
        return String(format: format, arguments: arguments)
    }
}

extension String {
    /// Localize string using LingXia SDK bundle
    var localized: String {
        return L10n.string(self)
    }

    /// Localize string with format arguments
    func localized(_ arguments: CVarArg...) -> String {
        return L10n.string(self, arguments: arguments)
    }
}
