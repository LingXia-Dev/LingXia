import Foundation
import OSLog

#if os(macOS)
import AppKit

/// Adaptive Surface Layout — macOS aside-dock reconciler.
///
/// The shared Rust core owns the surface graph and, after every mutation,
/// re-derives a `LayoutPresentationPlan` and hands it here. This reconciler is the SOLE
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
/// ## Single source of truth
///
/// The reconciler keeps NO private mirror of which asides it placed. The
/// WorkspaceManager view-registry IS that state: every registered panel is an
/// aside under this reconciler's authority (main content lives in the content
/// container, not the panel registry), and the reconciler is the sole code path
/// that shows/hides asides — so the set of "currently-placed asides" is exactly
/// the registered panels that are visible. Reconcile = make the registry match
/// `plan.asides`: show + position registered-and-desired panels, hide visible
/// panels no longer desired. A desired aside whose panel is not yet registered is
/// skipped as a defensive no-op; expected content paths register the panel before
/// the Rust graph commit pushes this plan.
///
/// Scope is strictly the aside dock. Because the reconciler only ever touches
/// the panel registry (all asides), it never disturbs non-aside surfaces — main
/// content, browser tabs, settings, downloads, float popups, the sidebar, the
/// update callout, or terminal fullscreen state.
@MainActor
enum LxAppLayoutReconciler {
    private static let log = OSLog(subsystem: "LingXia", category: "LayoutReconciler")

    /// The stable, fully-typed render contract the shared core emits (the same
    /// `LayoutPresentationPlan` JSON returned by `surfaceDerivedLayout`). The
    /// reconciler acts on `asides` for now, but decodes the complete contract so
    /// any future skin binding has the full typed view.
    private struct LayoutPresentationPlan: Decodable {
        let sizeClass: String
        let bottomOwner: String
        let switcherForm: String
        let splitForm: String
        let mains: [String]
        let activeMainId: String?
        let asides: [PlanAside]
        let floats: [PlanFloat]
        let tree: LxAppJSONValue?
    }

    private struct PlanAside: Decodable {
        let id: String
        let edge: String?
        let preferredSize: Double?
    }

    private struct PlanFloat: Decodable {
        let id: String
    }

    static func reconcile(windowId: String, json: String) -> Bool {
        return reconcile(label: "window=\(windowId)", json: json)
    }

    /// Reconcile float popups (modal sheets above the layout). Shell-independent:
    /// each float is its own popup window registered hidden by LxAppSurface.present,
    /// so this works with or without an active desktop shell. Single authority for
    /// float visibility — dismiss what the core dropped, show what it wants.
    private static func presentFloats(_ ids: [String]) {
        let desiredFloatIds = Set(ids)
        for id in LxAppSurface.visibleFloatIds().subtracting(desiredFloatIds) {
            _ = LxAppSurface.dismissFloat(id: id)
        }
        for id in desiredFloatIds {
            _ = LxAppSurface.showFloat(id: id)
        }
    }

    private static func reconcile(label: String, json: String) -> Bool {
        guard let shell = LxAppActiveHost.activeShell else {
            // No desktop shell (e.g. the Runner's phone simulator hosts the lxapp
            // via a controller, not a shell). Floats are shell-independent popup
            // windows, so still present them; asides/main are owned by the
            // controller-hosted layout, so there is nothing to dock here.
            if let data = json.data(using: .utf8),
               let plan = try? JSONDecoder().decode(LayoutPresentationPlan.self, from: data) {
                presentFloats(plan.floats.map { $0.id })
            }
            return true
        }
        guard let data = json.data(using: .utf8),
              let plan = try? JSONDecoder().decode(LayoutPresentationPlan.self, from: data) else {
            os_log("presentLayout: failed to parse layout json %{public}@", log: log, type: .error, label)
            return false
        }

        let workspace = shell.workspaceManager

        // Desired docked aside set from the core (id -> edge). `plan.asides`
        // also lists compact full-screen asides so mobile skins can keep them
        // in their desired set; macOS only docks them when the split form says
        // there is a side-by-side container.
        var desired: [String: PanelPosition] = [:]
        if plan.splitForm != "fullScreen" {
            for aside in plan.asides {
                desired[aside.id] = panelPosition(for: aside.edge)
            }
        }
        let desiredIds = Set(desired.keys)

        // Undock asides the core removed. The placed-aside set is derived from
        // the view-registry itself: a visible registered panel is, by
        // construction, one the reconciler placed (every registered panel is an
        // aside, and the reconciler is the sole code that shows asides). Hide
        // any such panel that the core no longer wants.
        for id in workspace.visiblePanelIds().subtracting(desiredIds) {
            shell.hidePanel(id: id)
        }

        // Place each desired aside at the tree's edge and show it. Content paths
        // should have already registered the panel (hidden) before the graph
        // commit. If not, skip defensively; a later plan push can converge it.
        for (id, edge) in desired {
            guard workspace.isPanelRegistered(id: id) else { continue }

            let atCorrectEdge = workspace.panelPosition(id: id) == edge
            let visible = workspace.isPanelVisible(id: id)

            // Idempotent fast path: already shown at the right edge — leave it
            // exactly as is (no hide/show/re-place → no flicker, no empty paint).
            if atCorrectEdge && visible {
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
        }

        // Float pass — popups above the layout. The reconciler is the single
        // authority for float visibility, mirroring the aside pass above: the
        // content path (LxAppSurface.present) created + registered each float
        // hidden, and this is the only code that shows/dismisses them.
        //
        //   * dismiss any float currently on-screen that the core no longer
        //     lists in plan.floats (the float teardown pops the modal-focus
        //     stack via the close observer);
        //   * show/order-front any desired float not yet visible (idempotent —
        //     a float already visible is left untouched, no flicker). A desired
        //     float whose popup is not yet registered is skipped defensively.
        presentFloats(plan.floats.map { $0.id })

        // Main pass — the active-main switch. The core's activeMainId is the
        // single source of truth for which lxapp occupies the primary content
        // area; when it differs from what the shell currently has attached, drive
        // the switch through the shell. reconcileActiveMain reuses the existing
        // attach machinery and is itself idempotent, and the browser is not a
        // graph main (attachedMainAppId is nil while the browser is active), so a
        // plan whose activeMainId already matches the on-screen main is a no-op.
        if let activeMainId = plan.activeMainId, activeMainId != shell.attachedMainAppId {
            shell.reconcileActiveMain(appId: activeMainId)
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
