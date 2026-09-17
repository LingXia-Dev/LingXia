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

            private enum CodingKeys: String, CodingKey {
                case owner, action, namespace, generation, actionId
            }

            func encode(to encoder: Encoder) throws {
                var values = encoder.container(keyedBy: CodingKeys.self)
                try values.encode(owner, forKey: .owner)
                try values.encodeIfPresent(action, forKey: .action)
                try values.encodeIfPresent(namespace, forKey: .namespace)
                try values.encodeIfPresent(generation, forKey: .generation)
                try values.encodeIfPresent(actionId, forKey: .actionId)
            }
        }

        let action: Action
        let label: String?
        let icon: String?
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
        guard let data = json.data(using: .utf8), !json.isEmpty, json != "null" else {
            return nil
        }
        do {
            return try JSONDecoder().decode(SurfaceMenuSnapshot.self, from: data)
        } catch {
            LXLog.error(
                "surface menu decode failed surface=\(surfaceId): \(error)",
                category: "SurfaceMenu"
            )
            return nil
        }
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

    @discardableResult
    func present(_ snapshot: SurfaceMenuSnapshot, event: NSEvent, from view: NSView) -> Bool {
        let menu = NSMenu()
        menu.autoenablesItems = false
        for (sectionIndex, section) in snapshot.sections.enumerated() {
            if sectionIndex > 0 { menu.addItem(.separator()) }
            for item in section.items {
                let title = Self.title(for: item)
                guard !title.isEmpty || item.action.owner == "information" else { continue }
                let menuItem = NSMenuItem(
                    title: title,
                    action: item.enabled ? #selector(menuItemSelected(_:)) : nil,
                    keyEquivalent: ""
                )
                menuItem.target = item.enabled ? self : nil
                menuItem.isEnabled = item.enabled
                menuItem.image = Self.icon(for: item)
                menuItem.representedObject = Selection(
                    revision: snapshot.revision,
                    surfaceId: snapshot.surfaceId,
                    action: item.action
                )
                menu.addItem(menuItem)
            }
        }
        guard !menu.items.isEmpty else { return false }
        NSMenu.popUpContextMenu(menu, with: event, for: view)
        return true
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

    private static func title(for item: SurfaceMenuSnapshot.Item) -> String {
        switch item.action.owner {
        case "information", "external":
            return item.label ?? ""
        default:
            return localizedTitle(for: item.action.action) ?? item.label ?? ""
        }
    }

    private static func localizedTitle(for action: String?) -> String? {
        switch action {
        case "rename": L10n.string("lx_surface_rename")
        case "resetTitle": L10n.string("lx_surface_reset_title")
        case "close": L10n.string("lx_surface_close")
        case "closeOthers": L10n.string("lx_surface_close_others")
        case "closeAfter": L10n.string("lx_surface_close_after")
        case "restart": L10n.string("lx_capsule_restart")
        case "cleanCacheRestart": L10n.string("lx_capsule_clean_cache")
        default: nil
        }
    }

    /// Same `design/icons/svg` names as the iOS / Android / Harmony capsule
    /// (`icon_restart`, `icon_clean_cache`, …). Close-tab rows reuse the
    /// sidebar bookmark glyphs from that set.
    private static func icon(for item: SurfaceMenuSnapshot.Item) -> NSImage? {
        if let path = item.icon, !path.isEmpty {
            return LxIcon.menuImage(fromPath: path)
        }
        switch (item.action.owner, item.action.action) {
        case ("lxapp", "restart"):
            return LxIcon.menuSymbol("icon_restart")
        case ("lxapp", "cleanCacheRestart"):
            return LxIcon.menuSymbol("icon_clean_cache")
        case ("switcher", "close"):
            return LxIcon.menuSymbol("icon_close_x")
        case ("switcher", "closeOthers"):
            return LxIcon.menuSymbol("icon_close_other_tabs")
        case ("switcher", "closeAfter"):
            return LxIcon.menuSymbol("icon_close_tabs_below")
        default:
            return nil
        }
    }
}
#endif
