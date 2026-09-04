#if os(macOS)
import Foundation

/// Bootstrap-owned projection of the sealed Settings destination into native
/// chrome. It deliberately retains only the static variant kind: resolution
/// always returns to Rust for the current runtime target.
struct LxAppStaticSettingsSource: Equatable, Sendable {
    enum DestinationKind: Equatable, Sendable {
        case controlAppPage
        case browserControlPage
        case nativeAction
    }

    static let sidebarItemID = "lingxia:static-settings"

    let destinationKind: DestinationKind

    init?(_ destination: LxAppGeneratedSettingsDestination?) {
        guard let destination else { return nil }
        switch destination {
        case .controlAppPage:
            destinationKind = .controlAppPage
        case .browserControlPage:
            destinationKind = .browserControlPage
        case .nativeAction:
            destinationKind = .nativeAction
        }
    }

    func activate(itemID: String, using resolver: () -> Bool) -> Bool {
        guard itemID == Self.sidebarItemID else { return false }
        return resolver()
    }

    /// Runtime sidebar declarations are not an alternate Settings provider.
    /// Suppress both identifier- and label-based impersonation even when the
    /// host has no configured Settings destination.
    static func acceptsRuntimeSidebarAction(id: String, label: String) -> Bool {
        id != sidebarItemID && !isSettingsIdentity(id) && !isSettingsIdentity(label)
    }

    private static func isSettingsIdentity(_ value: String) -> Bool {
        value.trimmingCharacters(in: .whitespacesAndNewlines)
            .caseInsensitiveCompare("settings") == .orderedSame
    }
}
#endif
