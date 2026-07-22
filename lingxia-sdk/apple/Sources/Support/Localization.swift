import Foundation

/// Localization helper for LingXia SDK. `string(_:)`/`.localized` are
/// SDK-internal — the full internal key space is not a public contract.
/// Host apps building native chrome get a curated surface via `L10n.AppMenu`
/// instead. Usage: L10n.string("lx_ui_back") or "lx_ui_back".localized
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
    static func string(_ key: String) -> String {
        return NSLocalizedString(key, bundle: localizedBundle, comment: "")
    }

    /// Get localized string with format arguments
    static func string(_ key: String, _ arguments: CVarArg...) -> String {
        return string(key, arguments: arguments)
    }

    static func string(_ key: String, arguments: [CVarArg]) -> String {
        let format = NSLocalizedString(key, bundle: localizedBundle, comment: "")
        return String(format: format, arguments: arguments)
    }

    /// Curated native-chrome strings for host apps that build their own
    /// macOS menu bar instead of relying on Cocoa's automatic one (see
    /// `examples/lingxia-showcase/macos/Sources/main.swift`). This is the
    /// supported public surface — it does not expose the SDK's full internal
    /// key space, so internal key renames don't become host-app breakage.
    public enum AppMenu {
        private static var hostAppName: String {
            Bundle.main.object(forInfoDictionaryKey: "CFBundleName") as? String
                ?? ProcessInfo.processInfo.processName
        }

        public static var about: String { L10n.string("lx_app_about", hostAppName) }
        public static var quit: String { L10n.string("lx_app_quit", hostAppName) }
        public static var edit: String { L10n.string("lx_app_edit") }
        public static var window: String { L10n.string("lx_app_window") }
        public static var minimize: String { L10n.string("lx_app_minimize") }
        public static var zoom: String { L10n.string("lx_app_zoom") }
        public static var bringAllToFront: String { L10n.string("lx_app_bring_all_to_front") }
        public static var cut: String { L10n.string("lx_menu_cut") }
        public static var copy: String { L10n.string("lx_menu_copy") }
        public static var paste: String { L10n.string("lx_menu_paste") }
        public static var selectAll: String { L10n.string("lx_menu_select_all") }
    }

    /// Curated strings for host-owned browser chrome.
    public enum Browser {
        public static var addressPlaceholder: String {
            L10n.string("lx_browser_address_placeholder")
        }
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
