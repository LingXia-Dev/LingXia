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

    /// Source type, not presentation strings, grants the static resolver.
    /// The reserved id is the sole merge collision a runtime item cannot own.
    static func acceptsRuntimeSidebarAction(id: String) -> Bool {
        id != sidebarItemID
    }

    @MainActor
    static func mergeFooter(
        runtimeItems: [LxAppUIActionItem],
        source: LxAppStaticSettingsSource?
    ) -> [LxAppUIActionItem] {
        var items = runtimeItems.filter { acceptsRuntimeSidebarAction(id: $0.id) }
        guard source != nil else { return items }
        items.append(LxAppUIActionItem(
            id: sidebarItemID,
            label: "Settings",
            iconURL: nil,
            builtInIcon: "gearshape",
            closable: false,
            sidebarActionSource: .staticSettings
        ))
        return items
    }
}
#endif
