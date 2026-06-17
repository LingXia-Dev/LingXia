import Foundation
import OSLog

#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Adaptive Surface Layout (§11.2 Phase 4) — macOS aside-dock reconciler.
///
/// The shared Rust core owns the surface graph and, after every mutation,
/// re-derives a `DerivedLayout` and hands it here. This reconciler is the SOLE
/// authority for aside PLACEMENT — edge, position, and visibility:
///
///   * each aside leaf in the tree is placed at the tree's `edge` and shown;
///   * an aside already shown at the correct edge is left untouched (idempotent
///     — no hide/show/re-place, so no flicker, no empty re-paint);
///   * an aside the reconciler previously placed that is no longer in the tree
///     is hidden (undocked) — the core dropped it.
///
/// The per-surface present path (`LxAppSurface.presentDockedAside`) and the
/// AppUI host-aside path (`LxAppMacAppUIRuntime`) only CREATE + REGISTER aside
/// content (so the reconciler can find a registered, hidden panel by id); they
/// no longer place or show it. Placement is owned exclusively here.
///
/// Scope is strictly the aside dock. The reconciler only ever shows/hides/places
/// ids that are (or were) asides in the core tree, so it never disturbs
/// non-aside panels — main content, browser tabs, settings, downloads, float
/// popups, the sidebar, the update callout, or terminal fullscreen state.
@MainActor
enum LxAppLayoutReconciler {
    private static let log = OSLog(subsystem: "LingXia", category: "LayoutReconciler")

    /// Ids this reconciler has placed as asides (across both the dynamic and the
    /// host-aside paths). Bounds undock to surfaces we actually placed, so a
    /// panel owned by some other path is never hidden even if it briefly shares
    /// the workspace dock.
    private static var placedAsideIds: Set<String> = []

    /// Decoded view of the parts of `DerivedLayout` the dock needs.
    private struct Layout: Decodable {
        let asides: [Aside]?
    }

    private struct Aside: Decodable {
        let id: String
        let edge: String?
    }

    /// Re-derive the core layout for `appId` and reconcile. Called by the content
    /// paths AFTER they register aside content, so the reconciler runs once the
    /// panel exists and can be placed (the core's own `present_layout` may have
    /// fired before the content was registered).
    static func reconcileNow(appId: String) {
        let json = surfaceDerivedLayout(appId).toString()
        guard !json.isEmpty else { return }
        _ = reconcile(appId: appId, json: json)
    }

    static func reconcile(appId: String, json: String) -> Bool {
        guard let shell = LxAppActiveHost.activeShell else {
            // Headless / no desktop shell: nothing to dock. Drop the placed set
            // so a later shell starts from a clean slate.
            placedAsideIds.removeAll()
            return false
        }
        guard let data = json.data(using: .utf8),
              let layout = try? JSONDecoder().decode(Layout.self, from: data) else {
            os_log("presentLayout: failed to parse layout json app=%{public}@", log: log, type: .error, appId)
            return false
        }

        let workspace = shell.workspaceManager

        // Desired aside set from the core (id -> edge).
        var desired: [String: PanelPosition] = [:]
        for aside in layout.asides ?? [] {
            desired[aside.id] = panelPosition(for: aside.edge)
        }
        let desiredIds = Set(desired.keys)

        // Undock asides the core removed. Only ones we ourselves placed, so we
        // never disturb panels owned by other paths.
        for id in placedAsideIds.subtracting(desiredIds)
        where workspace.isPanelRegistered(id: id) {
            if workspace.isPanelVisible(id: id) {
                shell.hidePanel(id: id)
            }
            placedAsideIds.remove(id)
        }

        // Place each desired aside at the tree's edge and show it. The content
        // path created + registered the panel (hidden) for ids the core knows;
        // until that registration lands there is nothing to place yet (the
        // content path calls reconcileNow once it has registered, so this runs
        // again with the panel present).
        for (id, edge) in desired {
            guard workspace.isPanelRegistered(id: id) else { continue }

            let atCorrectEdge = workspace.panelPosition(id: id) == edge
            let visible = workspace.isPanelVisible(id: id)

            // Idempotent fast path: already shown at the right edge — leave it
            // exactly as is (no hide/show/re-place → no flicker, no empty paint).
            if atCorrectEdge && visible {
                placedAsideIds.insert(id)
                continue
            }

            // Move to the tree's edge if it differs from where it was registered.
            // repositionPanel preserves the attached content and leaves it
            // hidden, so the show below brings it in at the authoritative edge.
            if !atCorrectEdge {
                workspace.repositionPanel(id: id, to: edge)
            }
            if !workspace.isPanelVisible(id: id) {
                shell.showPanel(id: id)
            }
            placedAsideIds.insert(id)
        }

        return true
    }

    /// Map a serde `Edge` ("left"/"right"/"top"/"bottom") to a dock edge.
    /// A missing/unknown edge defaults to the trailing edge, matching the core's
    /// host-aside default (`registerHostAside` falls back to `Right`).
    private static func panelPosition(for edge: String?) -> PanelPosition {
        switch edge {
        case "left": return .left
        case "top": return .top
        case "bottom": return .bottom
        default: return .right
        }
    }
}
#endif
