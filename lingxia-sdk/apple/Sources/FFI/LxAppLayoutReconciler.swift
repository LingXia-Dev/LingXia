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
    private static var lastRevisionByWindow: [String: UInt64] = [:]
    /// The last committed plan sequence applied per window.
    private static var lastSequenceByWindow: [String: UInt64] = [:]
    /// One-shot explicit lxapp activation intent (`lx` API / switcher). A
    /// presented browser tab covers the lxapp main across unrelated layout
    /// commits (resize, aside changes); only this intent may replace it.
    private static var requestedLxappMainActivation: String?

    /// `raiseHost` is true only for the runtime's explicit `lx.shell.openApp`;
    /// a sidebar click already runs in the foreground, and an lxapp restart or
    /// self-close must never pull this host over another app.
    static func requestLxappMainActivation(appId: String, raiseHost: Bool) {
        requestedLxappMainActivation = appId
        guard let shell = LxAppActiveHost.activeShell else { return }
        if raiseHost {
            // Not a click in this process's foreground window: order the host
            // front before the layout commit.
            shell.orderShellFront()
        }
        // The sidebar highlight is not in the surface graph. Selecting here
        // covers a reopen whose active main did not change, so no later plan
        // would move the row. A browser cover is replaced first; otherwise
        // its deactivation clears the selection we just set.
        if shell.browserIsCoveringMain {
            requestedLxappMainActivation = nil
            shell.reconcileActiveMain(appId: appId)
        }
        shell.selectSidebarLxApp(appId: appId, surfaceId: appId)
    }

    /// A newly-presented browser/native cover invalidates any older intent —
    /// it must not be consumed later by an unrelated commit.
    static func clearLxappMainActivation() {
        requestedLxappMainActivation = nil
    }

    /// The stable, fully-typed render contract the shared core emits (the same
    /// `LayoutPresentationPlan` JSON returned by `surfaceDerivedLayout`). The
    /// reconciler acts on `asides` for now, but decodes the complete contract so
    /// any future skin binding has the full typed view.
    private struct LayoutPresentationPlan: Decodable {
        let sequence: UInt64?
        let sizeClass: String
        let bottomOwner: String
        let switcherForm: String
        let splitForm: String
        let mains: [String]
        let activeMainId: String?
        let asides: [PlanAside]
        let asideSlots: [PlanAsideSlot]?
        let floats: [PlanFloat]
        let tree: LxAppJSONValue?
        let mainSwitcher: MainSwitcher?
    }

    private struct MainSwitcher: Decodable {
        let revision: UInt64
        let items: [MainSwitcherItem]
    }

    private struct MainSwitcherItem: Decodable {
        struct Content: Decodable {
            let kind: String
            let appId: String?
        }

        let surfaceId: String
        let content: Content
    }

    private struct PlanAside: Decodable {
        let id: String
        let edge: String?
        let preferredSize: Double?
    }

    private struct PlanAsideSlot: Decodable {
        let kind: String
        let edge: String?
        let children: [String]
        let activeChild: String?
        let visible: Bool
        let overlay: Bool
        /// The user maximized the slot over the main. The core also sets
        /// `overlay`, so placement below needs nothing else; the state lives
        /// in the core and survives every later plan.
        let maximized: Bool?
        let collapsed: Bool?
    }

    private struct PlanFloat: Decodable {
        let id: String
    }

    /// Last aside-slot plan applied while a desktop shell was up. Registry
    /// artwork lands after the paint that asked for it; without this the
    /// visible tab strip would keep the titles/icons from that first pass.
    private static var lastAsideSlots: [PlanAsideSlot] = []

    static func reconcile(windowId: String, json: String) -> Bool {
        return reconcile(windowId: windowId, label: "window=\(windowId)", json: json)
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

    private static func reconcile(windowId: String, label: String, json: String) -> Bool {
        guard let data = json.data(using: .utf8),
              let plan = try? JSONDecoder().decode(LayoutPresentationPlan.self, from: data) else {
            LXLog.error("presentLayout: failed to parse layout json \(label)", category: "LayoutReconciler")
            return false
        }
        // Plans hop to the main queue, so one committed earlier from another
        // thread can land after a newer one committed on the main thread (a
        // maximize right after an aside opened). Applying it would put the
        // slot back in its docked state. The core stamps every commit; drop
        // a plan older than the last one applied.
        if let sequence = plan.sequence, sequence > 0 {
            let last = lastSequenceByWindow[windowId] ?? 0
            if sequence < last { return true }
            lastSequenceByWindow[windowId] = sequence
        }
        if let revision = plan.mainSwitcher?.revision {
            let last = lastRevisionByWindow[windowId] ?? 0
            if revision < last { return true }
            lastRevisionByWindow[windowId] = revision
        }
        guard let shell = LxAppActiveHost.activeShell else {
            // No desktop shell (the Runner's iPhone shape hosts the lxapp via a
            // controller, not a shell). Asides/windows drill in full-screen over
            // the device screen like a real phone, so — mirroring the iOS
            // reconciler — dismiss any full-screen surface the core dropped.
            // Floats are shell-independent popup windows, so still present them.
            let desiredFullScreen = Set(plan.asides.map { $0.id }).union(plan.mains)
            for id in LxAppSurface.presentedFullScreenIds().subtracting(desiredFullScreen) {
                LxAppSurface.dismissFullScreen(id: id)
            }
            presentFloats(plan.floats.map { $0.id })
            return true
        }

        let slots = plan.asideSlots ?? []

        // The core already resolved breakpoints and hysteresis. Bind shell
        // chrome to that same plan so macOS cannot drift from Windows.
        shell.applySurfaceLayoutProjection(plan.sizeClass)
        let workspace = shell.workspaceManager

        // Desired docked panel set from the core (panel id -> edge).
        // `plan.asides` also lists compact full-screen asides so mobile skins
        // can keep them in their desired set; macOS only docks them when the
        // split form says there is a side-by-side container.
        //
        // With slots the aside area holds ONE region per slot kind: only each
        // visible slot's active child docks; sibling children stay
        // registered-but-hidden and switch via the slot's header tab strip.
        // In the side panel an lxapp child is its own registered panel while
        // every browser child lives inside the ONE docked browser, whose
        // panel is keyed by its anchor tab (`panelHostId`).
        var desired: [String: PanelPosition] = [:]
        var overlayIds = Set<String>()
        // Overlays the user maximized: they keep their docked room so the main
        // keeps its frame underneath (see `setPanelFullscreen`).
        var maximizedIds = Set<String>()
        // A compact window has no dock to keep: there the main fills the area.
        let keepsDockRoom = { (slot: PlanAsideSlot) in
            slot.maximized == true && plan.splitForm != "fullScreen"
        }
        if slots.isEmpty {
            if plan.splitForm != "fullScreen" {
                for aside in plan.asides {
                    desired[aside.id] = panelPosition(for: aside.edge)
                }
            }
        } else {
            for slot in slots where slot.visible {
                let projectedOverlay = slot.overlay || plan.splitForm == "fullScreen"
                guard let active = slot.activeChild ?? slot.children.last else { continue }
                let host = panelHostId(for: active, shell: shell)
                desired[host] = panelPosition(for: slot.edge)
                if projectedOverlay { overlayIds.insert(host) }
                if projectedOverlay && keepsDockRoom(slot) { maximizedIds.insert(host) }
                // A browser child: the docked browser shows that tab.
                if host != active {
                    shell.browserCoordinator.activeDockedBrowser?.activateTab(surfaceId: active)
                }
            }
        }
        let desiredIds = Set(desired.keys)

        // Panels taking part in an INTRA-SLOT tab switch: some sibling of the
        // slot's active child is currently on screen, so the region itself
        // stays put — hide/show must be instant, not a slide (the slide is the
        // "a new aside docks" affordance, wrong for switching tabs in place).
        let visibleNow = workspace.visiblePanelIds()
        var instantIds = Set<String>()
        for slot in slots where slot.visible {
            guard let active = slot.activeChild ?? slot.children.last else { continue }
            let activeHost = panelHostId(for: active, shell: shell)
            let hosts = Set(slot.children.map { panelHostId(for: $0, shell: shell) })
            if let shown = hosts.first(where: { $0 != activeHost && visibleNow.contains($0) }) {
                instantIds.formUnion(hosts)
                // One region, one width: the incoming host takes the size the
                // user gave the slot through the host it is replacing.
                workspace.syncPanelSize(from: shown, to: activeHost)
            }
        }

        // Undock asides the core removed. The placed-aside set is derived from
        // the view-registry itself: a visible registered panel is, by
        // construction, one the reconciler placed (every registered panel is an
        // aside, and the reconciler is the sole code that shows asides). Hide
        // any such panel that the core no longer wants.
        for id in visibleNow.subtracting(desiredIds) {
            let intraSlotSwitch = instantIds.contains(id)
            // On a switch the sibling re-occupies the region in this same
            // pass: skip the card-edge update so the main area's webview is
            // not resized out and back (a visible flash).
            shell.hidePanel(id: id, animated: !intraSlotSwitch, updateCardEdges: !intraSlotSwitch)
            workspace.setPanelFullscreen(id: id, enabled: false)
        }

        // Place each desired aside at the tree's edge and show it. Content paths
        // should have already registered the panel (hidden) before the graph
        // commit. The one content path that cannot is a main demoted to an
        // aside: its page still lives in a main controller, so re-host it in
        // a panel now. Otherwise skip defensively; a later plan push can
        // converge it.
        for (id, edge) in desired {
            if !workspace.isPanelRegistered(id: id)
                && !LxAppMacAppUIRuntime.adoptDemotedMainLxApp(surfaceId: id, position: edge) {
                continue
            }

            let atCorrectEdge = workspace.panelPosition(id: id) == edge
            let visible = workspace.isPanelVisible(id: id)
            workspace.setPanelFullscreen(
                id: id,
                enabled: overlayIds.contains(id),
                keepsDockRoom: maximizedIds.contains(id))

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
                // Show immediately: the panel is the click's feedback. Its
                // container spins a loading indicator on light paper until
                // the content view lands, so a cold lxapp boot reads as
                // progress, not as a dead click or a dark flash.
                shell.showPanel(id: id, animated: !instantIds.contains(id))
            }
        }

        // Slot tab pass — bind each visible slot's header strip onto the panel
        // presenting it. Selecting a tab focuses that child in the core graph
        // (the next plan swaps the visible child); closing destroys that child;
        // collapsing puts the region away with every child still open. The strip
        // remains visible for a single child as its close affordance.
        let previousSlots = lastAsideSlots
        lastAsideSlots = slots
        applyPanelSlotTabs(slots, shell: shell, workspace: workspace)
        applyCollapsedHandles(slots, shell: shell, workspace: workspace)
        applyMaximizeFocus(from: previousSlots, to: slots, shell: shell)
        for slot in slots where slot.visible && slot.kind == "native" {
            guard let active = slot.activeChild ?? slot.children.last else { continue }
            LxAppMacAppUIRuntime.reflectAsideMaximized(surfaceId: active, maximized: slot.maximized == true)
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

        // Main provider pass. `activeMainId` is a Surface id, never a provider
        // app id. Only lxapp content is attached here; browser/native providers
        // already rendered their content before committing this plan.
        if let activeMainId = plan.activeMainId,
           let item = plan.mainSwitcher?.items.first(where: { $0.surfaceId == activeMainId }),
           item.content.kind == "lxapp",
           let appId = item.content.appId {
            let explicitlyRequested = requestedLxappMainActivation == appId
            if explicitlyRequested { requestedLxappMainActivation = nil }
            // An undeclared browser tab covers the lxapp main until its own
            // close/back flow restores it. Explicit activation is the one
            // exception; unrelated commits must not evict the cover (mirrors
            // the Windows reconciler). An already-attached main still needs
            // the sidebar selection: that highlight is not in the graph, and
            // `lx.shell.openApp` never went through a sidebar click.
            let show = !shell.browserIsCoveringMain || explicitlyRequested
            let alreadyAttached = shell.attachedMainAppId == appId
            if show && !alreadyAttached {
                shell.reconcileActiveMain(appId: appId)
            }
            if explicitlyRequested || (show && !alreadyAttached) {
                shell.selectSidebarLxApp(appId: appId, surfaceId: activeMainId)
            }
        }
        LxAppMacAppUIRuntime.refreshSurfaceSwitcherProjection()

        return true
    }

    /// The registered panel that shows `child`: a browser tab's is the docked
    /// browser's (keyed by its anchor tab); an lxapp's is its own.
    private static func panelHostId(for child: String, shell: LxAppShell) -> String {
        if let docked = shell.browserCoordinator.activeDockedBrowser,
           docked.contains(surfaceId: child),
           let anchor = docked.anchorSurfaceId {
            return anchor
        }
        return child
    }

    private static func isBrowserChild(_ child: String, shell: LxAppShell) -> Bool {
        shell.browserCoordinator.activeDockedBrowser?.contains(surfaceId: child) == true
    }

    /// The lxapp the side panel shows, from the last plan; nil when the slot
    /// is absent, collapsed, not admitted, or showing a browser tab.
    static func activeLxappAsideId() -> String? {
        guard let shell = LxAppActiveHost.activeShell,
              let slot = lastAsideSlots.first(where: { $0.kind == "panel" && $0.visible }),
              let active = slot.activeChild ?? slot.children.last,
              !isBrowserChild(active, shell: shell) else {
            return nil
        }
        return active
    }

    /// Any lxapp the side panel holds, shown or not. A live lxapp reaches the
    /// window's graph, which is what a collapsed slot's keyboard restore needs
    /// when no lxapp main is attached.
    static func lxappAsideChildId() -> String? {
        guard let shell = LxAppActiveHost.activeShell,
              let slot = lastAsideSlots.first(where: { $0.kind == "panel" }) else { return nil }
        let preferred = slot.activeChild ?? slot.children.last
        return ([preferred].compactMap { $0 } + slot.children.reversed())
            .first { !isBrowserChild($0, shell: shell) }
    }

    /// Any live lxapp that reaches the window's graph, for slot-level calls
    /// (focus, collapse, maximize) made while a browser tab may be active.
    private static func graphAppId(shell: LxAppShell, fallback: String) -> String {
        shell.attachedMainAppId
            ?? LxAppTabManager.shared.activeTab?.appId
            ?? lxappAsideChildId()
            ?? fallback
    }

    /// Re-bind the visible side panel's tabs from the last plan, resolving
    /// lxapp names and icons against the current registry cache and browser
    /// titles and favicons against the docked browser. The initial lookups
    /// are asynchronous; without this the strip stays on the first pass's
    /// titles/icons until an unrelated layout commit.
    static func refreshVisibleSlotTabs() {
        guard let shell = LxAppActiveHost.activeShell else { return }
        applyPanelSlotTabs(
            lastAsideSlots,
            shell: shell,
            workspace: shell.workspaceManager
        )
    }

    private static func applyPanelSlotTabs(
        _ slots: [PlanAsideSlot],
        shell: LxAppShell,
        workspace: WorkspaceManager
    ) {
        let docked = shell.browserCoordinator.activeDockedBrowser
        docked?.onTabsChanged = { refreshVisibleSlotTabs() }
        for slot in slots where slot.visible && slot.kind == "panel" {
            guard let active = slot.activeChild ?? slot.children.last else { continue }
            let host = panelHostId(for: active, shell: shell)
            guard workspace.isPanelRegistered(id: host) else { continue }
            let tabs = slot.children.map { child -> AsideSlotTab in
                if let docked, docked.contains(surfaceId: child) {
                    let info = docked.tabInfo(surfaceId: child)
                    return AsideSlotTab(
                        id: child,
                        title: info?.title ?? child,
                        isBrowser: true,
                        image: info?.icon
                    )
                }
                // An lxapp aside's surface id IS its app id — resolve the
                // human-facing app name + icon; fall back to the raw id.
                let info = getLxAppInfo(child)
                let name = info.app_name.toString()
                let icon = getLxAppDisplayIconPath(child).toString()
                return AsideSlotTab(
                    id: child,
                    title: name.isEmpty ? child : name,
                    iconPath: icon.isEmpty ? nil : icon
                )
            }
            let appId = graphAppId(shell: shell, fallback: active)
            workspace.setSlotTabs(
                panelId: host,
                tabs: tabs,
                activeId: active,
                onSelect: { childId in
                    guard childId != active else { return }
                    _ = focusSurface(appId, childId)
                },
                onClose: { childId in
                    if LxAppSurface.isBrowserAside(surfaceId: childId) {
                        _ = LxAppSurface.closeAsideTab(surfaceId: childId)
                    } else {
                        _ = LxAppMacAppUIRuntime.handleAsideSlotClose(surfaceId: childId)
                    }
                },
                onCollapse: {
                    _ = collapseAsideSlot(appId, "panel")
                },
                maximized: slot.maximized == true,
                // A compact overlay already covers the main: nothing to toggle.
                onToggleMaximize: slot.overlay && slot.maximized != true ? nil : {
                    _ = setAsideSlotMaximized(appId, "panel", slot.maximized != true)
                },
                onMenu: { childId in
                    if let docked, docked.contains(surfaceId: childId) {
                        return docked.menu(forSurfaceId: childId)
                    }
                    return lxappAsideTabMenu(appId: childId, graphAppId: appId)
                }
            )
        }
    }

    /// The menu behind an lxapp aside tab's "···" and right-click: the
    /// app's identity, the move to the main area (what dropping the tab on
    /// the sidebar does), the app's own MoreActions, and Close. Collapse and
    /// Maximize stay out: they are slot controls, already on the strip.
    private static func lxappAsideTabMenu(appId: String, graphAppId: String) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false

        let info = getLxAppInfo(appId)
        let name = info.app_name.toString().trimmingCharacters(in: .whitespacesAndNewlines)
        var header = name.isEmpty ? appId : name
        let version = info.version.toString().trimmingCharacters(in: .whitespacesAndNewlines)
        if !version.isEmpty { header += " · \(version)" }
        if info.release_type.toString().lowercased() == "draft" { header += " [DRAFT]" }
        let headerItem = NSMenuItem(title: header, action: nil, keyEquivalent: "")
        headerItem.isEnabled = false
        menu.addItem(headerItem)
        menu.addItem(.separator())

        let openInMain = MenuActionItem(title: L10n.string("lx_aside_open_in_main")) {
            _ = promoteAsideLxappToMain(appId)
        }
        openInMain.image = NSImage(
            systemSymbolName: "macwindow", accessibilityDescription: nil)?
            .withSymbolConfiguration(.init(pointSize: 13, weight: .regular))
        menu.addItem(openInMain)

        let snapshot = LxAppMoreActionSnapshot.load(appId: appId)
        if !snapshot.items.isEmpty {
            menu.addItem(.separator())
            for (index, item) in snapshot.items.enumerated() {
                let token = snapshot.token(at: index)
                let actionItem = MenuActionItem(title: item.label) {
                    DispatchQueue.main.async {
                        _ = onLxappEvent(appId, LxAppEvent.capsuleClick, token)
                    }
                }
                actionItem.image = LxIcon.menuImage(fromPath: item.iconPath)
                menu.addItem(actionItem)
            }
        }

        menu.addItem(.separator())
        let close = MenuActionItem(title: L10n.string("lx_common_close")) {
            _ = LxAppMacAppUIRuntime.handleAsideSlotClose(surfaceId: appId)
        }
        close.image = LxIcon.menuSymbol("icon_close_x")
        menu.addItem(close)
        return menu
    }

    /// Focus follows a maximize, whichever way it was asked for (the control,
    /// Escape, a sidebar intent, Logic): into the covering page, back to the
    /// main page on restore. The terminal slot moves focus with its own zoom.
    private static func applyMaximizeFocus(
        from previous: [PlanAsideSlot],
        to slots: [PlanAsideSlot],
        shell: LxAppShell
    ) {
        let shown = { (slot: PlanAsideSlot) in slot.visible && slot.maximized == true }
        let was = previous.first(where: { $0.kind == "panel" }).map(shown) ?? false
        let now = slots.first(where: { $0.kind == "panel" })
        let isNow = now.map(shown) ?? false
        guard was != isNow else { return }
        let panelId = now
            .flatMap { $0.activeChild ?? $0.children.last }
            .map { panelHostId(for: $0, shell: shell) }
        shell.asideSlotMaximizeDidChange(maximized: isNow, panelId: panelId)
    }

    /// A collapsed slot that still holds content, or one hidden for lack of
    /// room, keeps a handle at its edge, so the region can come back without
    /// the app opening it again.
    private static func applyCollapsedHandles(
        _ slots: [PlanAsideSlot],
        shell: LxAppShell,
        workspace: WorkspaceManager
    ) {
        let handles = slots.compactMap { slot -> CollapsedAsideHandle? in
            // A collapsed slot, or one hidden because it has no room to dock
            // beside the main: either way its content stays one click away.
            // A slot whose children its app hid has no active child and
            // nothing to bring back.
            guard slot.collapsed == true || !slot.visible, slot.activeChild != nil,
                  slot.kind == "panel" else { return nil }
            let position = panelPosition(for: slot.edge)
            guard position == .left || position == .right else { return nil }
            // Named after the child the slot was showing: a browser tab's
            // title, or the lxapp's name. Any live lxapp reaches the graph.
            var title = "Browser"
            let child = slot.activeChild ?? slot.children.last
            if let child, let docked = shell.browserCoordinator.activeDockedBrowser,
               let info = docked.tabInfo(surfaceId: child) {
                title = info.title
            } else if let child {
                let name = getLxAppInfo(child).app_name.toString()
                title = name.isEmpty ? child : name
            }
            let graphAppId = self.graphAppId(shell: shell, fallback: child ?? "")
            return CollapsedAsideHandle(
                kind: slot.kind, position: position, title: title, graphAppId: graphAppId)
        }
        workspace.setCollapsedAsideHandles(handles) { handle in
            _ = restoreAsideSlot(handle.graphAppId, handle.kind)
        }
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
