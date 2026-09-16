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

    static func fromBootstrapJSON(_ json: String) -> Self? {
        guard let destination = try? JSONDecoder().decode(
            LxAppGeneratedSettingsDestination.self,
            from: Data(json.utf8)
        ) else { return nil }
        return Self(destination)
    }

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

    /// Settings is bootstrap-owned header chrome (icon-only, with Downloads),
    /// not a Logic `sidebarActions` entry and not a footer row.
    @MainActor
    static func mergeHeader(
        runtimeItems: [LxAppUIActionItem],
        source: LxAppStaticSettingsSource?
    ) -> [LxAppUIActionItem] {
        var items = runtimeItems.filter { acceptsRuntimeSidebarAction(id: $0.id) }
        guard source != nil else { return items }
        let settings = LxAppUIActionItem(
            id: sidebarItemID,
            label: "Settings",
            iconURL: nil,
            builtInIcon: "gearshape",
            closable: false,
            sidebarActionSource: .staticSettings
        )
        items.insert(settings, at: 0)
        // Settings takes one of the two header slots (spec §4.5).
        if items.count > 2 {
            let dropped = items.dropFirst(2).map(\.id).joined(separator: ", ")
            LXLog.warn(
                "header sidebar actions exceed the slots left beside Settings; dropping \(dropped)",
                category: "Sidebar"
            )
            items = Array(items.prefix(2))
        }
        return items
    }
}
#endif
