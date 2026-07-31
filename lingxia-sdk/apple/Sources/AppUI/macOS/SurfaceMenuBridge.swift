#if os(macOS)
import AppKit
import Foundation

struct SurfaceMenuSnapshot: Decodable {
    struct Section: Decodable {
        let kind: String
        let items: [Item]
    }

    struct Item: Decodable {
        struct Action: Codable {
            let owner: String
            let action: String?
            let namespace: String?
            let generation: UInt64?
            let actionId: String?
        }

        let action: Action
        let label: String?
        let enabled: Bool
        let role: String
    }

    let revision: UInt64
    let surfaceId: String
    let sections: [Section]
}

struct SurfaceMenuExecution: Decodable {
    let accepted: Bool
    let removedSurfaceIds: [String]
    let snapshot: SurfaceSwitcherSnapshot
}

@MainActor
enum SurfaceMenuBridge {
    private struct Intent: Encodable {
        let revision: UInt64
        let surfaceId: String
        let action: SurfaceMenuSnapshot.Item.Action
        let value: String?
    }

    static func snapshot(ownerAppId: String, surfaceId: String) -> SurfaceMenuSnapshot? {
        let json = surfaceMenu(ownerAppId, surfaceId).toString()
        guard let data = json.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(SurfaceMenuSnapshot.self, from: data)
    }

    static func perform(
        ownerAppId: String,
        revision: UInt64,
        surfaceId: String,
        action: SurfaceMenuSnapshot.Item.Action,
        value: String? = nil
    ) -> SurfaceMenuExecution? {
        let intent = Intent(
            revision: revision,
            surfaceId: surfaceId,
            action: action,
            value: value
        )
        guard let data = try? JSONEncoder().encode(intent),
              let json = String(data: data, encoding: .utf8)
        else { return nil }
        let result = performSurfaceMenuIntent(ownerAppId, json).toString()
        guard let resultData = result.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(SurfaceMenuExecution.self, from: resultData)
    }

    static func builtInAction(_ name: String) -> SurfaceMenuSnapshot.Item.Action {
        SurfaceMenuSnapshot.Item.Action(
            owner: "switcher",
            action: name,
            namespace: nil,
            generation: nil,
            actionId: nil
        )
    }
}

@MainActor
final class SurfaceMenuPresenter: NSObject {
    var onAction: ((UInt64, String, SurfaceMenuSnapshot.Item.Action, String?) -> Void)?

    func present(_ snapshot: SurfaceMenuSnapshot, event: NSEvent, from view: NSView) {
        let menu = NSMenu()
        for (sectionIndex, section) in snapshot.sections.enumerated() {
            if sectionIndex > 0 { menu.addItem(.separator()) }
            for item in section.items {
                let menuItem = NSMenuItem(
                    title: item.label ?? Self.title(for: item.action.action),
                    action: #selector(menuItemSelected(_:)),
                    keyEquivalent: ""
                )
                menuItem.target = self
                menuItem.isEnabled = item.enabled
                menuItem.representedObject = Selection(
                    revision: snapshot.revision,
                    surfaceId: snapshot.surfaceId,
                    action: item.action
                )
                menu.addItem(menuItem)
            }
        }
        guard !menu.items.isEmpty else { return }
        NSMenu.popUpContextMenu(menu, with: event, for: view)
    }

    private final class Selection: NSObject {
        let revision: UInt64
        let surfaceId: String
        let action: SurfaceMenuSnapshot.Item.Action

        init(revision: UInt64, surfaceId: String, action: SurfaceMenuSnapshot.Item.Action) {
            self.revision = revision
            self.surfaceId = surfaceId
            self.action = action
        }
    }

    @objc private func menuItemSelected(_ sender: NSMenuItem) {
        guard let selection = sender.representedObject as? Selection else { return }
        onAction?(selection.revision, selection.surfaceId, selection.action, nil)
    }

    private static func title(for action: String?) -> String {
        switch action {
        case "rename": L10n.string("lx_surface_rename")
        case "resetTitle": L10n.string("lx_surface_reset_title")
        case "close": L10n.string("lx_surface_close")
        case "closeOthers": L10n.string("lx_surface_close_others")
        case "closeAfter": L10n.string("lx_surface_close_after")
        case "restart": L10n.string("lx_capsule_restart")
        case "cleanCacheRestart": L10n.string("lx_capsule_clean_cache")
        default: ""
        }
    }
}
#endif
