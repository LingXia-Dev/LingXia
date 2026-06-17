import Foundation
import OSLog

#if os(macOS)
import AppKit

/// Adaptive Surface Layout (§11.2 Phase 3) — macOS aside-dock reconciler.
///
/// The shared Rust core owns the surface graph and, after every mutation,
/// re-derives a `DerivedLayout` and hands it here. This reconciler makes the
/// shell's aside dock match the core's authoritative aside set:
///
///   * tree asides that already have a docked panel are left untouched
///     (idempotent — no flicker, no re-create of a correctly-docked panel);
///   * tree asides whose panel exists but is hidden are re-shown;
///   * panels we previously docked as asides that are no longer in the tree are
///     hidden (undocked) — the core dropped them.
///
/// Scope is strictly the aside dock. It never creates panel *content* (the
/// per-surface present path and the AppUI host-aside path still build and dock
/// the terminal/AI-chat/page/web content; this only reconciles dock
/// visibility) and never touches main content, browser tabs, float popups, the
/// sidebar, the update callout, or terminal fullscreen state.
@MainActor
enum LxAppLayoutReconciler {
    private static let log = OSLog(subsystem: "LingXia", category: "LayoutReconciler")

    /// Ids this reconciler has docked as asides. Bounds undock to surfaces we
    /// actually manage, so panels owned by other paths (main, browser) are never
    /// hidden even if they briefly share the workspace dock.
    private static var managedAsideIds: Set<String> = []

    /// Decoded view of the parts of `DerivedLayout` the dock needs.
    private struct Layout: Decodable {
        let asides: [Aside]?
    }

    private struct Aside: Decodable {
        let id: String
        let edge: String?
    }

    static func reconcile(appId: String, json: String) -> Bool {
        guard let shell = LxAppActiveHost.activeShell else {
            // Headless / no desktop shell: nothing to dock. Drop the managed set
            // so a later shell starts from a clean slate.
            managedAsideIds.removeAll()
            return false
        }
        guard let data = json.data(using: .utf8),
              let layout = try? JSONDecoder().decode(Layout.self, from: data) else {
            os_log("presentLayout: failed to parse layout json app=%{public}@", log: log, type: .error, appId)
            return false
        }

        // Desired aside set from the core (id -> edge).
        var desired: [String: PanelPosition] = [:]
        for aside in layout.asides ?? [] {
            desired[aside.id] = panelPosition(for: aside.edge)
        }
        let desiredIds = Set(desired.keys)

        // Undock asides the core removed. Only ones we ourselves docked, so we
        // never disturb panels owned by other paths.
        for id in managedAsideIds.subtracting(desiredIds)
        where shell.workspaceManager.isPanelRegistered(id: id) {
            if shell.workspaceManager.isPanelVisible(id: id) {
                shell.hidePanel(id: id)
            }
            managedAsideIds.remove(id)
        }

        // Ensure each desired aside is docked. Already-visible panels are left
        // exactly as they are (idempotent — no re-create, no flicker). Registered
        // but hidden panels are re-shown; the per-surface / host-aside path will
        // have created & registered the panel content for ids the core knows.
        for id in desiredIds {
            guard shell.workspaceManager.isPanelRegistered(id: id) else {
                // Content not yet docked by its owning present path; presentLayout
                // does not create content, so there is nothing to reconcile yet.
                // TODO(Phase 4): move aside content creation + initial dock here so
                // the dock is owned solely by presentLayout. Until then the
                // per-surface (LxAppSurface.presentDockedAside) and host-aside
                // (LxAppMacAppUIRuntime) paths perform the initial dock and this
                // reconciler keeps it in sync — making presentLayout idempotent
                // rather than authoritative for creation. See §11.2 Phase 4.
                continue
            }
            if !shell.workspaceManager.isPanelVisible(id: id) {
                shell.showPanel(id: id)
            }
            managedAsideIds.insert(id)
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
