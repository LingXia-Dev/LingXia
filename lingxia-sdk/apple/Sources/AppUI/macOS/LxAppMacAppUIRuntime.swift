#if os(macOS)
import AppKit
import CLingXiaRustAPI
import OSLog

private func lxTerminalRuntimeStdoutLog(_ message: String) {
    guard ProcessInfo.processInfo.environment["LX_TERMINAL_STDOUT_LOGS"] == "1" else {
        return
    }
    let line = "[LingXia][TerminalRuntime] \(message)\n"
    FileHandle.standardOutput.write(Data(line.utf8))
    NSLog("%@", line.trimmingCharacters(in: .newlines))
}

private func lxTerminalRuntimeFormatRect(_ rect: NSRect) -> String {
    String(
        format: "%.0f,%.0f %.0fx%.0f",
        rect.minX,
        rect.minY,
        rect.width,
        rect.height
    )
}

@MainActor
struct LxAppUIActionItem: Sendable {
    var generation: UInt64 = 0
    let id: String
    let label: String
    let iconURL: URL?
    var contentAppId: String? = nil
    var builtInIcon: String? = nil
    var showsLxappTabBar: Bool = false
    var active: Bool = false
    var closable: Bool = true
    var renameable: Bool = false
    var titleOverridden: Bool = false
    var disabled: Bool = false
}

@MainActor
final class LxAppMacAppUIRuntime: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "MacAppUI")
    private static let panelFirstPaintPollNs: UInt64 = 50_000_000
    private static let panelFirstPaintMaxPolls = 24
    private static let panelFirstPaintSettleNs: UInt64 = 16_000_000
    private static let shellTerminalSurfaceID = "shell:terminal"

    nonisolated(unsafe) static weak var active: LxAppMacAppUIRuntime?

    let appConfig: LxAppGeneratedAppConfig
    let uiConfig: LxAppUIConfig
    let controller: LxAppController
    let shell: LxAppShell
    let uiConfigURL: URL

    private let rootSurface: LxAppUIConfig.Surface
    private var surfaceById: [String: LxAppUIConfig.Surface]
    private let declaredSurfaceIDs: Set<String>
    private let childrenByParentId: [String: [String]]
    private let menuBarActivators: [LxAppUIConfig.Activator]
    private let appActivationActivators: [LxAppUIConfig.Activator]
    private let sidebarActivators: [LxAppUIConfig.Activator]
    private let toolbarActivators: [LxAppUIConfig.Activator]
    private let titlebarActivators: [LxAppUIConfig.Activator]

    private var visibleSurfaceIDs = Set<String>()
    private var openedSurfaceIDs = Set<String>()
    /// Runtime edge overrides from `lx.openSurface({surface, edge})`; the
    /// declared `lingxia.yaml` edge applies when absent.
    private var managedEdgeOverrides: [String: LxAppUIConfig.Edge] = [:]
    /// Native providers keep one workspace while the core moves their stable
    /// surface id between the main switcher and an aside slot.
    private var managedRoleOverrides: [String: LxAppUIConfig.Role] = [:]
    private var nativeInstanceSurfaceIDs: [String: String] = [:]
    private struct RuntimeLxAppPanel {
        let appId: String
        let path: String
    }
    /// Undeclared or role-overridden lxapps opened as asides. They participate
    /// in the same managed visibility API as declared panels, but remain live
    /// while hidden until the Rust handle explicitly closes the lxapp.
    private var runtimeLxAppPanels: [String: RuntimeLxAppPanel] = [:]
    private lazy var trayController = LxAppMacTrayController(
        appConfig: appConfig,
        uiConfigURL: uiConfigURL
    ) { [weak self] actionID in
        // A status-item click does not activate the app, and the target window may
        // be hidden to the tray or sitting behind another app — pull the app to the
        // foreground so the click reliably brings it forward.
        NSApp.activate(ignoringOtherApps: true)
        self?.performActivator(id: actionID)
    }
    private var independentPanelWindows: [String: NSPanel] = [:]
    private var independentPanelHostViews: [String: LxAppHostView] = [:]
    private var independentPanelOpenTasks: [String: Task<Void, Never>] = [:]
    private var independentPanelDisplayTasks: [String: Task<Void, Never>] = [:]
    private var independentPanelSourceActivatorIDs: [String: String] = [:]
    private var surfacePageInstanceIDs: [String: String] = [:]
    private var terminalWorkspaces: [String: LingXiaTerminalWorkspaceView] = [:]
    private lazy var surfaceMenuPresenter: SurfaceMenuPresenter = {
        let presenter = SurfaceMenuPresenter()
        presenter.onAction = { [weak self] revision, surfaceId, action, value in
            self?.performSurfaceMenuAction(
                revision: revision,
                surfaceId: surfaceId,
                action: action,
                value: value
            )
        }
        return presenter
    }()
    nonisolated(unsafe) private var independentPanelOutsideClickGlobalMonitor: Any?
    nonisolated(unsafe) private var independentPanelOutsideClickLocalMonitor: Any?
    nonisolated(unsafe) private var appActivationObserver: NSObjectProtocol?
    private var handlingAppActivation = false

    private let graphOwnerAppId: String?

    init(
        bundleConfig: LxAppGeneratedBundleConfig,
        controller: LxAppController,
        shell: LxAppShell
    ) throws {
        self.appConfig = bundleConfig.app
        self.uiConfig = bundleConfig.ui
        self.controller = controller
        self.shell = shell
        self.uiConfigURL = bundleConfig.uiURL

        let validation = try Self.validate(bundleConfig: bundleConfig)
        self.rootSurface = validation.rootSurface
        self.surfaceById = validation.surfaceById
        self.declaredSurfaceIDs = Set(validation.surfaceById.keys)
        self.childrenByParentId = validation.childrenByParentId
        self.menuBarActivators = validation.menuBarActivators
        self.appActivationActivators = validation.appActivationActivators
        self.sidebarActivators = validation.sidebarActivators
        self.toolbarActivators = validation.toolbarActivators
        self.titlebarActivators = validation.titlebarActivators
        if let appId = validation.rootSurface.content.appId ?? bundleConfig.app.homeAppId,
           !appId.isEmpty {
            self.graphOwnerAppId = appId
        } else {
            let ownerAppId = ensureHostSurfaceOwner().toString()
            self.graphOwnerAppId = ownerAppId.isEmpty ? nil : ownerAppId
        }

        super.init()

        shell.onManagedWindowCloseRequested = { [weak self] in
            self?.handleRootWindowCloseRequest()
        }
        // A companion lxapp's sidebar entry TOGGLES its aside surface (never
        // switches the main): hidden → show + focus; already showing → close. A
        // single entry with one obvious behavior, so clicking it again closes
        // the aside the user opened.
        shell.onAsideActivateRequested = { [weak self] surfaceId in
            guard let self else { return }
            if self.visibleSurfaceIDs.contains(surfaceId) {
                self.closeManagedSurface(id: surfaceId)
            } else {
                self.openManagedSurface(id: surfaceId)
                self.bringSurfaceToFront(id: surfaceId)
            }
        }
        shell.onMainWillSwitch = { [weak self] in
            self?.collapseExpandedAsides()
        }
        shell.setSidebarHostActionHandler { [weak self] generation, actionID in
            self?.performRuntimeSidebarAction(generation: generation, id: actionID)
        }
        shell.setToolbarHostActionHandler { [weak self] actionID in
            self?.performActivator(id: actionID)
        }
        shell.setTitlebarHostActionHandler { [weak self] actionID in
            self?.performActivator(id: actionID)
        }
        shell.configureDeclaredBrowser(
            ownerAppId: graphOwnerAppId,
            onSurfaceActivate: { [weak self] surfaceID in
                self?.didActivateBrowserMainSurface(id: surfaceID)
            },
            onSurfaceClose: { [weak self] surfaceID in
                self?.closeMainSurface(id: surfaceID)
            },
            onRestoreActiveMain: { [weak self] in
                self?.restoreActiveMainProvider() ?? false
            }
        )
        // A float root never shows the sidebar; for other roots, content drives
        // visibility via the shell's auto-hide recompute.
        shell.setSidebarSuppressed(rootSurface.role == .float)

        Self.active = self
    }

    func start() throws {
        // A tray-exclusive app (hideDockIcon) is a menu-bar agent with no dock icon;
        // everything else keeps the dock icon. Info.plist's LSUIElement already made
        // the exclusive case an accessory before launch, so this only confirms it.
        if uiConfig.launch.hideDockIcon == true {
            NSApp.setActivationPolicy(.accessory)
        } else {
            NSApp.setActivationPolicy(.regular)
        }
        if appConfig.capabilities?.terminal == true {
            _ = LingXiaTerminalSettings.load()
        }
        trayController.installMenuBarActivators(menuBarActivators)
        installAppActivationActivators()
        guard let ownerAppId = graphOwnerAppId,
              SurfaceSwitcherBridge.replaceDeclaredMains(
                  ownerAppId: ownerAppId,
                  surfaces: uiConfig.surfaces,
                  initialSurfaceID: uiConfig.launch.initialSurface
              ),
              SurfaceSwitcherBridge.registerDeclaredNativeAsides(
                  ownerAppId: ownerAppId,
                  surfaces: uiConfig.surfaces
              )
        else {
            throw LxAppUIError.invalidConfig("failed to register declared surfaces")
        }
        refreshChromeActions()
        let opensLxAppOnLaunch = (uiConfig.launch.openOnLaunch ?? true)
            && rootSurface.content.kind == .lxapp
        if uiConfig.launch.openOnLaunch ?? true {
            try openSurface(id: uiConfig.launch.initialSurface)
        }
        // Opening an lxapp already starts its worker and dispatches App.onLaunch.
        // Starting it again here races the asynchronous page-stack setup and can
        // expose a Logic-ready app with no current page to automation clients.
        if !opensLxAppOnLaunch,
           appConfig.homeAppId?.isEmpty == false,
           !launchHomeControlLogic() {
            LXLog.error("Home control Logic failed to launch", category: "MacAppUI")
        }
    }

    deinit {
        for (_, task) in independentPanelOpenTasks {
            task.cancel()
        }
        for (_, task) in independentPanelDisplayTasks {
            task.cancel()
        }
        if let monitor = independentPanelOutsideClickGlobalMonitor {
            NSEvent.removeMonitor(monitor)
        }
        if let monitor = independentPanelOutsideClickLocalMonitor {
            NSEvent.removeMonitor(monitor)
        }
        if let appActivationObserver {
            NotificationCenter.default.removeObserver(appActivationObserver)
        }
    }

    static func handlePanelLxAppOpened(
        appId: String,
        path: String,
        sessionId: UInt64,
        panelId: String
    ) -> Bool {
        guard let active else { return false }
        return active.handleOpenedPanel(appId: appId, path: path, sessionId: sessionId, panelId: panelId)
    }

    static func handleAppActivation() -> Bool {
        guard let active else { return false }
        return active.performAppActivation()
    }

    /// Native aside-slot close affordance. Unlike managed hide, this destroys
    /// the lxapp session and releases its one-region claim.
    static func handleAsideSlotClose(surfaceId: String) -> Bool {
        guard let active else { return false }
        return active.closeAsideSlotChild(surfaceId: surfaceId)
    }

    static func refreshSurfaceSwitcherProjection() {
        active?.refreshChromeActions()
    }

    /// A top-level surface window supersedes transient tray UI. This is also
    /// called when an existing window is shown, because a nonactivating panel
    /// can otherwise remain above a window that was already key.
    static func dismissIndependentPanelsForSurfaceWindow() {
        active?.dismissVisibleIndependentPanels()
    }

    // MARK: - Tray runtime updates (lx.tray.*)

    func setTrayBadge(_ text: String?) { trayController.setBadge(text) }
    func setTrayIcon(_ icon: String) { trayController.setIcon(icon) }
    func setTrayTitle(_ text: String?) { trayController.setTitle(text) }
    func setTrayMenu(_ json: String) { trayController.setMenu(json) }
    func setTrayVisible(_ visible: Bool) { trayController.setVisible(visible) }
    func setTrayClickIntercept(_ intercept: Bool) { trayController.clickIntercepted = intercept }

    private func handleOpenedPanel(
        appId: String,
        path: String,
        sessionId: UInt64,
        panelId: String
    ) -> Bool {
        guard let surface = surfaceById[panelId],
              effectiveRole(for: surface) == .aside || isIndependentPanelSurface(surface) else {
            // An UNDECLARED lxapp opened with panel presentation (privileged
            // openSurface / activator): dock it as a runtime aside keyed by
            // its appId. Returning false here would fall through to the main
            // tab path — an aside must never enter the sidebar.
            guard let primaryAppId = graphOwnerAppId else { return false }
            shell.storeSession(sessionId, for: appId)
            shell.registerPanelWithContent(id: panelId, position: .right, appId: appId, path: path)
            runtimeLxAppPanels[panelId] = RuntimeLxAppPanel(appId: appId, path: path)
            _ = registerHostAside(primaryAppId, panelId, managedEdgeOverrides[panelId]?.rawValue ?? "right")
            openedSurfaceIDs.insert(panelId)
            visibleSurfaceIDs.insert(panelId)
            refreshChromeActions()
            return true
        }

        if isIndependentPanelSurface(surface),
           let hostView = independentPanelHostViews[panelId],
           let panel = independentPanelWindows[panelId] {
            let hasPendingOpenTask = independentPanelOpenTasks[panelId] != nil
            if !hasPendingOpenTask && !openedSurfaceIDs.contains(panelId) {
                if let pageInstanceId = resolveSurfacePageInstanceId(
                    surface,
                    appIdHint: appId,
                    pathHint: path,
                    sessionIdHint: sessionId
                ) {
                    _ = notifyPageInstanceHidden(pageInstanceId, "programmatic")
                }
                os_log(
                    "ignore stale panel-open callback panel=%{public}@ appId=%{public}@ path=%{public}@",
                    log: Self.log,
                    type: .info,
                    panelId,
                    appId,
                    path
                )
                return true
            }
            guard let pageInstanceId = WebViewManager.resolvePageInstanceId(
                appId: appId,
                path: path,
                sessionId: sessionId
            ) else {
                LXLog.error("independent panel missing page instance id panel=\(panelId) appId=\(appId) path=\(path)", category: "MacAppUI")
                return false
            }
            surfacePageInstanceIDs[panelId] = pageInstanceId
            shell.storeSession(sessionId, for: appId)
            let displayActivatorID = independentPanelSourceActivatorIDs[panelId]
            let session = LxAppSession(
                id: LxAppSessionID(rawValue: sessionId),
                appId: appId,
                path: path,
                presentation: .panel,
                userInfo: [
                    "appUISurfaceId": .string(panelId),
                    "pageInstanceId": .string(pageInstanceId),
                ],
                openedAt: Date()
            )
            independentPanelDisplayTasks[panelId]?.cancel()
            independentPanelDisplayTasks[panelId] = Task { @MainActor [weak hostView] in
                defer {
                    independentPanelDisplayTasks[panelId] = nil
                    if independentPanelSourceActivatorIDs[panelId] == displayActivatorID {
                        independentPanelSourceActivatorIDs.removeValue(forKey: panelId)
                    }
                }
                do {
                    try await hostView?.mount(session, notifyVisibleOnMount: false)
                    if let hostView {
                        for _ in 0..<Self.panelFirstPaintMaxPolls {
                            if let webView = hostView.webView,
                               !webView.isLoading,
                               webView.url != nil {
                                try await Task.sleep(nanoseconds: Self.panelFirstPaintSettleNs)
                                break
                            }
                            try await Task.sleep(nanoseconds: Self.panelFirstPaintPollNs)
                        }
                    }
                    try Task.checkCancellation()
                    positionIndependentPanel(panel, for: displayActivatorID)
                    panel.orderFrontRegardless()
                    _ = notifyPageInstanceVisible(pageInstanceId)
                    openedSurfaceIDs.insert(panelId)
                    visibleSurfaceIDs.insert(panelId)
                    installIndependentPanelOutsideClickMonitorsIfNeeded()
                    refreshChromeActions()
                } catch is CancellationError {
                    return
                } catch {
                    surfacePageInstanceIDs.removeValue(forKey: panelId)
                    openedSurfaceIDs.remove(panelId)
                    visibleSurfaceIDs.remove(panelId)
                    updateIndependentPanelOutsideClickMonitors()
                    LXLog.error("independent panel webview mount failed panel=\(panelId) appId=\(appId) path=\(path)", category: "MacAppUI", error: error)
                }
            }
            return true
        }

        guard effectiveRole(for: surface) == .aside,
              let position = panelPosition(for: surface) else {
            return false
        }

        shell.storeSession(sessionId, for: appId)
        // Register the aside lxapp panel slot/content (hidden) before mutating
        // the Rust surface graph. The registerHostAside commit below is the only
        // layout-plan delivery that places and shows it.
        shell.registerPanelWithContent(id: panelId, position: position, appId: appId, path: path)
        registerHostAsideForSurface(surface)
        openedSurfaceIDs.insert(panelId)
        visibleSurfaceIDs.insert(panelId)
        refreshChromeActions()
        return true
    }

    private func handleRootWindowCloseRequest() {
        if menuBarActivators.isEmpty {
            NSApp.terminate(nil)
            return
        }
        // Closing a tray-backed window hides the window; it does not close the
        // stable root Surface, which is intentionally non-closable.
        shell.hide()
        visibleSurfaceIDs.remove(rootSurface.id)
        refreshChromeActions()
    }

    private struct ResolvedRuntimeSidebarAction: Codable {
        let generation: UInt64
        let id: String
        let placement: String
        let label: String
        let iconPath: String?
        let disabled: Bool
    }

    private var runtimeSidebarActions: [ResolvedRuntimeSidebarAction] = []

    func setRuntimeSidebarActions(_ json: String) {
        guard let data = json.data(using: .utf8),
              let items = try? JSONDecoder().decode([ResolvedRuntimeSidebarAction].self, from: data)
        else {
            LXLog.error("setSidebarActions: bad payload", category: "MacAppUI")
            return
        }
        runtimeSidebarActions = items
        refreshChromeActions()
    }

    func setShellPins(_ json: String) {
        shell.updateShellPins(json)
    }

    func openBuiltinBrowserPage(id: String) -> Bool {
        shell.openBuiltinShellSurface(id: id)
    }

    private func runtimeSidebarActionItems(placement: String) -> [LxAppUIActionItem] {
        return runtimeSidebarActions.filter { $0.placement == placement }.map { item in
            return LxAppUIActionItem(
                generation: item.generation,
                id: item.id,
                label: item.label,
                iconURL: runtimeItemIconURL(item),
                disabled: item.disabled
            )
        }
    }

    private func runtimeItemIconURL(_ item: ResolvedRuntimeSidebarAction) -> URL? {
        guard let icon = item.iconPath, !icon.isEmpty else { return nil }
        if let url = URL(string: icon), url.isFileURL { return url }
        if icon.hasPrefix("/") { return URL(fileURLWithPath: icon) }
        return LxAppAppUIBundleLoader.resolveRelativeResource(icon, baseURL: uiConfigURL)
    }

    private func performRuntimeSidebarAction(generation: UInt64, id: String) {
        _ = shellActivate(generation, id)
    }

    private func performActivator(id: String) {
        guard let activator = uiConfig.activators.first(where: { $0.id == id }) else { return }

        switch activator.action.kind {
        case .toggleSurface:
            if let surface = surfaceById[activator.action.surface],
               isIndependentPanelSurface(surface),
               independentPanelWindows[surface.id]?.isVisible == true {
                _ = hideManagedSurface(id: surface.id, updateGraph: true)
                return
            }
            toggleSurface(id: activator.action.surface, sourceActivatorID: activator.id)
        case .openSurface:
            openSurfaceHandlingError(id: activator.action.surface, sourceActivatorID: activator.id)
        }
    }

    private func toggleSurface(id: String, sourceActivatorID: String? = nil) {
        if visibleSurfaceIDs.contains(id) {
            _ = hideManagedSurface(id: id, updateGraph: true)
        } else {
            openSurfaceHandlingError(id: id, sourceActivatorID: sourceActivatorID)
        }
    }

    /// Toggle a host-declared surface's visibility. Returns `false` if `id` is
    /// not a declared surface, so the caller can report the failure.
    @discardableResult
    func toggleManagedSurface(id: String) -> Bool {
        if runtimeLxAppPanels[id] != nil {
            if visibleSurfaceIDs.contains(id) {
                _ = hideManagedSurface(id: id, updateGraph: true)
            } else {
                _ = openManagedSurface(id: id)
            }
            return true
        }
        guard surfaceById[id] != nil else { return false }
        if visibleSurfaceIDs.contains(id) {
            _ = hideManagedSurface(id: id, updateGraph: true)
        } else {
            _ = openManagedSurface(id: id)
        }
        return true
    }

    /// Ensure a declared provider is visible with the core-resolved
    /// presentation. Native terminals may move between main and aside without
    /// replacing their workspace.
    @discardableResult
    func openManagedSurface(id: String, role: String? = nil, edge: String? = nil) -> Bool {
        if runtimeLxAppPanels[id] != nil {
            guard role == nil || role == LxAppUIConfig.Role.aside.rawValue else { return false }
            let previousEdge = managedEdgeOverrides[id]
            if let edge {
                guard let parsed = LxAppUIConfig.Edge(rawValue: edge) else { return false }
                managedEdgeOverrides[id] = parsed
            }
            guard let primaryAppId = graphOwnerAppId else {
                managedEdgeOverrides[id] = previousEdge
                return false
            }
            shell.show()
            guard registerHostAside(
                primaryAppId,
                id,
                managedEdgeOverrides[id]?.rawValue ?? "right"
            ) else {
                managedEdgeOverrides[id] = previousEdge
                return false
            }
            openedSurfaceIDs.insert(id)
            visibleSurfaceIDs.insert(id)
            refreshChromeActions()
            return true
        }
        guard let surface = surfaceById[id] else { return false }

        let previousRoleOverride = managedRoleOverrides[id]
        let previousEdgeOverride = managedEdgeOverrides[id]
        let previousRole = effectiveRole(for: surface)
        let requestedRole: LxAppUIConfig.Role?
        if let role {
            guard let parsed = LxAppUIConfig.Role(rawValue: role) else { return false }
            requestedRole = parsed
        } else {
            requestedRole = nil
        }
        let nextRole = requestedRole ?? previousRole
        if nextRole != previousRole {
            guard id != rootSurface.id,
                  surface.content.isNativeTerminal,
                  nextRole == .main || nextRole == .aside
            else { return false }
        }

        let requestedEdge: LxAppUIConfig.Edge?
        if let edge {
            guard nextRole == .aside,
                  let parsed = LxAppUIConfig.Edge(rawValue: edge)
            else { return false }
            requestedEdge = parsed
        } else {
            requestedEdge = nil
        }

        if let requestedRole {
            if requestedRole == surface.role {
                managedRoleOverrides.removeValue(forKey: id)
            } else {
                managedRoleOverrides[id] = requestedRole
            }
        }
        if let requestedEdge {
            managedEdgeOverrides[id] = requestedEdge
        }

        let nextEdge = managedEdgeOverrides[id] ?? surface.edge
        let previousEdge = previousEdgeOverride ?? surface.edge
        let presentationChanged = nextRole != previousRole
            || (nextRole == .aside && nextEdge != previousEdge)
        let wasVisible = visibleSurfaceIDs.contains(id)
        if wasVisible && !presentationChanged && nextRole != .main {
            return true
        }

        if openManagedSurfaceNow(id: id) {
            return true
        }

        managedRoleOverrides[id] = previousRoleOverride
        managedEdgeOverrides[id] = previousEdgeOverride
        if wasVisible && presentationChanged {
            _ = openManagedSurfaceNow(id: id)
        }
        return false
    }

    @discardableResult
    func openManagedNativeSurface(
        id: String,
        capability: String,
        instanceKey: String?,
        role: String,
        edge: String?
    ) -> Bool {
        guard capability == LxAppUIConfig.Content.NativeName.terminal.rawValue else { return false }
        let template: LxAppUIConfig.Surface
        if instanceKey == nil {
            guard let declared = surfaceById[id],
                  declaredSurfaceIDs.contains(id),
                  declared.content.name?.rawValue == capability
            else { return false }
            template = declared
        } else {
            guard let declared = surfaceById.values.first(where: {
                declaredSurfaceIDs.contains($0.id)
                    && $0.content.name?.rawValue == capability
            }) else { return false }
            template = declared
        }

        if let instanceKey {
            guard !instanceKey.isEmpty,
                  let requestedRole = LxAppUIConfig.Role(rawValue: role),
                  requestedRole == .main || requestedRole == .aside
            else { return false }
            if edge != nil && requestedRole != .aside { return false }

            let identity = "\(capability)\u{0}\(instanceKey)"
            if let existingID = nativeInstanceSurfaceIDs[identity], existingID != id {
                return false
            }
            let registeredIdentity = nativeInstanceSurfaceIDs[identity] == nil
            let createdSurface = surfaceById[id] == nil
            if let existing = surfaceById[id] {
                guard existing.content.name?.rawValue == capability,
                      existing.content.instanceKey == instanceKey
                else { return false }
            } else {
                surfaceById[id] = LxAppUIConfig.Surface(
                    id: id,
                    role: .main,
                    edge: nil,
                    attachTo: nil,
                    size: template.size,
                    anchor: template.anchor,
                    resizable: template.resizable,
                    showTrafficLights: template.showTrafficLights,
                    content: LxAppUIConfig.Content(
                        kind: template.content.kind,
                        appId: template.content.appId,
                        page: template.content.page,
                        query: template.content.query,
                        path: template.content.path,
                        url: template.content.url,
                        name: template.content.name,
                        instanceKey: instanceKey
                    ),
                    platforms: template.platforms
                )
            }
            nativeInstanceSurfaceIDs[identity] = id
            if openManagedSurface(id: id, role: role, edge: edge) {
                return true
            }
            if createdSurface {
                discardTerminalSurfaceContent(id: id)
            } else if registeredIdentity {
                nativeInstanceSurfaceIDs.removeValue(forKey: identity)
            }
            return false
        }

        return openManagedSurface(id: id, role: role, edge: edge)
    }

    private func effectiveRole(for surface: LxAppUIConfig.Surface) -> LxAppUIConfig.Role {
        managedRoleOverrides[surface.id] ?? surface.role
    }

    /// Hide a host-declared surface (no-op if already hidden). Returns `false`
    /// for an unknown surface `id`.
    @discardableResult
    func closeManagedSurface(id: String) -> Bool {
        hideManagedSurface(id: id, updateGraph: false)
    }

    @discardableResult
    private func hideManagedSurface(id: String, updateGraph: Bool) -> Bool {
        if runtimeLxAppPanels[id] != nil {
            if visibleSurfaceIDs.contains(id) {
                shell.hidePanel(id: id)
                visibleSurfaceIDs.remove(id)
                if updateGraph, let primaryAppId = graphOwnerAppId {
                    _ = markHostSurfaceHidden(primaryAppId, id)
                }
                refreshChromeActions()
            }
            return true
        }
        guard let surface = surfaceById[id] else { return false }
        if visibleSurfaceIDs.contains(id) {
            if effectiveRole(for: surface) == .main {
                return closeMainSurface(id: id)
            }
            for childID in childrenByParentId[id] ?? [] {
                _ = hideManagedSurface(id: childID, updateGraph: updateGraph)
            }
            switch effectiveRole(for: surface) {
            case .main:
                break
            case .float:
                independentPanelOpenTasks[id]?.cancel()
                independentPanelOpenTasks[id] = nil
                independentPanelDisplayTasks[id]?.cancel()
                independentPanelDisplayTasks[id] = nil
                independentPanelSourceActivatorIDs.removeValue(forKey: id)
                if let pageInstanceId = surfacePageInstanceIDs[id]
                    ?? resolveSurfacePageInstanceId(surface)
                {
                    _ = notifyPageInstanceHidden(pageInstanceId, "programmatic")
                }
                if isIndependentPanelSurface(surface) {
                    independentPanelWindows[id]?.orderOut(nil)
                } else {
                    shell.hide()
                }
            case .aside:
                shell.setPanelFullscreen(id: id, enabled: false)
                terminalWorkspaces[id]?.setSurfaceZoomEnabled(false, notifyRuntime: false)
                terminalWorkspaces[id]?.disarmInput()
                shell.hidePanel(id: id)
            }
            visibleSurfaceIDs.remove(id)
            if updateGraph, let primaryAppId = graphOwnerAppId {
                _ = markHostSurfaceHidden(primaryAppId, id)
            }
            updateIndependentPanelOutsideClickMonitors()
            refreshChromeActions()
        }
        return true
    }

    @discardableResult
    func destroyManagedSurface(id: String, role: String?) -> Bool {
        let requestedRole: LxAppUIConfig.Role?
        if let role {
            guard let parsed = LxAppUIConfig.Role(rawValue: role) else { return false }
            requestedRole = parsed
        } else {
            requestedRole = nil
        }

        for childID in childrenByParentId[id] ?? [] {
            let childRole = surfaceById[childID].map { effectiveRole(for: $0).rawValue }
            _ = destroyManagedSurface(id: childID, role: childRole)
            if let primaryAppId = graphOwnerAppId {
                _ = unregisterHostAside(primaryAppId, childID)
            }
        }

        if let panel = runtimeLxAppPanels[id] {
            _ = hideManagedSurface(id: id, updateGraph: false)
            shell.closeAsideLxApp(appId: panel.appId)
            runtimeLxAppPanels.removeValue(forKey: id)
            openedSurfaceIDs.remove(id)
            visibleSurfaceIDs.remove(id)
            shell.unregisterPanel(id: id)
            managedRoleOverrides.removeValue(forKey: id)
            managedEdgeOverrides.removeValue(forKey: id)
            refreshChromeActions()
            return true
        }

        guard id != rootSurface.id,
              let surface = surfaceById[id]
        else { return false }
        let providerRole = requestedRole ?? effectiveRole(for: surface)
        guard providerRole == effectiveRole(for: surface) else { return false }

        if providerRole == .main {
            let wasVisible = visibleSurfaceIDs.contains(id)
            discardMainSurfaceContent(id: id)
            if wasVisible,
               let ownerAppId = graphOwnerAppId,
               let activeSurfaceID = SurfaceSwitcherBridge.snapshot(ownerAppId: ownerAppId)?.activeSurfaceId,
               openManagedSurfaceNow(id: activeSurfaceID) {
                _ = setActiveMainSurface(ownerAppId, activeSurfaceID)
            } else {
                refreshChromeActions()
            }
            return true
        }

        _ = hideManagedSurface(id: id, updateGraph: false)
        if surface.content.isNativeTerminal {
            discardTerminalSurfaceContent(id: id)
        }
        if surface.content.kind == .lxapp, let appId = surface.content.appId {
            shell.closeAsideLxApp(appId: appId)
        }
        independentPanelWindows[id]?.close()
        independentPanelWindows.removeValue(forKey: id)
        runtimeLxAppPanels.removeValue(forKey: id)
        visibleSurfaceIDs.remove(id)
        discardOpenedSubtree(rootID: id)
        shell.unregisterPanel(id: id)
        managedRoleOverrides.removeValue(forKey: id)
        managedEdgeOverrides.removeValue(forKey: id)
        refreshChromeActions()
        return true
    }

    private func closeAsideSlotChild(surfaceId: String) -> Bool {
        if surfaceId == Self.shellTerminalSurfaceID {
            closeTerminalWorkspaceSurface(id: surfaceId)
            return true
        }
        let appId = surfaceById[surfaceId]?.content.appId
            ?? runtimeLxAppPanels[surfaceId]?.appId
            ?? surfaceId
        guard !appId.isEmpty else { return false }
        _ = closeManagedSurface(id: surfaceId)
        if let primaryAppId = graphOwnerAppId {
            _ = unregisterHostAside(primaryAppId, surfaceId)
        }
        runtimeLxAppPanels.removeValue(forKey: surfaceId)
        openedSurfaceIDs.remove(surfaceId)
        visibleSurfaceIDs.remove(surfaceId)
        shell.closeAsideLxApp(appId: appId)
        shell.unregisterPanel(id: surfaceId)
        managedRoleOverrides.removeValue(forKey: surfaceId)
        managedEdgeOverrides.removeValue(forKey: surfaceId)
        refreshChromeActions()
        return true
    }

    /// Collapse any fullscreen-expanded aside back to its docked edge (keeps it
    /// visible). Called on a main switch — an expanded aside is a temporary
    /// maximize, not a new main, so it un-maximizes rather than floating over the
    /// newly-shown main. Mirrors the expand teardown in close/hide so the
    /// terminal's own zoom state stays in sync.
    private func collapseExpandedAsides() {
        for id in visibleSurfaceIDs where shell.isPanelFullscreen(id: id) {
            terminalWorkspaces[id]?.setSurfaceZoomEnabled(false, notifyRuntime: false)
            shell.setPanelFullscreen(id: id, enabled: false)
        }
    }

    private func openSurfaceHandlingError(id: String, sourceActivatorID: String? = nil) {
        do {
            try openSurface(id: id, sourceActivatorID: sourceActivatorID)
        } catch {
            LXLog.error("AppUI failed to open surface=\(id) activator=\(sourceActivatorID ?? "nil")", category: "MacAppUI", error: error)
        }
    }

    private func openManagedSurfaceNow(id: String) -> Bool {
        do {
            try openSurface(id: id)
            return true
        } catch {
            LXLog.error("AppUI failed to open managed surface=\(id)", category: "MacAppUI", error: error)
            return false
        }
    }

    /// Browser automation can remove the core tab before native chrome observes
    /// it. Re-query the graph here instead of guessing that the legacy lxapp tab
    /// is the successor; terminal and browser mains use the same arbitration.
    private func restoreActiveMainProvider() -> Bool {
        guard let ownerAppId = graphOwnerAppId,
              let snapshot = SurfaceSwitcherBridge.snapshot(ownerAppId: ownerAppId),
              let activeID = snapshot.activeSurfaceId
        else { return false }

        if surfaceById[activeID] != nil {
            guard openManagedSurfaceNow(id: activeID) else { return false }
        } else if let active = snapshot.items.first(where: { $0.surfaceId == activeID }),
                  active.content.kind == "lxapp",
                  let appId = active.content.appId {
            shell.activateMainLxAppProvider(appId: appId)
        } else {
            return false
        }
        return setActiveMainSurface(ownerAppId, activeID)
    }

    private func openSurface(id: String, sourceActivatorID: String? = nil) throws {
        guard let surface = surfaceById[id] else {
            throw LxAppUIError.invalidConfig("unknown surface id \(id)")
        }

        switch effectiveRole(for: surface) {
        case .main, .float:
            if isIndependentPanelSurface(surface) {
                try openIndependentPanelSurface(surface, sourceActivatorID: sourceActivatorID)
            } else {
                try openWindowSurface(surface, sourceActivatorID: sourceActivatorID)
            }
        case .aside:
            try openAttachPanelSurface(surface)
        }
    }

    private func openWindowSurface(
        _ surface: LxAppUIConfig.Surface,
        sourceActivatorID: String? = nil
    ) throws {
        guard let ownerAppId = graphOwnerAppId else {
            throw LxAppUIError.invalidConfig("main surfaces require a graph owner")
        }
        let role = effectiveRole(for: surface)
        let switcher = SurfaceSwitcherBridge.snapshot(ownerAppId: ownerAppId)
        if switcher?.items.contains(where: { $0.surfaceId == surface.id }) != true,
           !SurfaceSwitcherBridge.openDeclaredMain(ownerAppId: ownerAppId, surface: surface) {
            throw LxAppUIError.invalidConfig("failed to open main surface \(surface.id)")
        }
        applyWindowPresentation(for: surface, role: role)
        if role == .float {
            positionPanelWindow(for: sourceActivatorID)
        }

        if switcher?.activeSurfaceId == surface.id, openedSurfaceIDs.contains(surface.id) {
            shell.show()
            visibleSurfaceIDs.insert(surface.id)
            if surface.content.isNativeTerminal {
                openTerminalMainSurface(surface)
            } else if isBrowserMainSurface(surface) {
                try openBrowserMainSurface(surface)
            } else if surface.content.kind == .lxapp,
                      shell.attachedMainAppId != surface.content.appId,
                      let appId = surface.content.appId {
                shell.mountLxAppMainProvider(appId: appId)
            }
            refreshChromeActions()
            return
        }

        shell.show()
        switch surface.content.kind {
        case .lxapp:
            if !openedSurfaceIDs.contains(surface.id) {
                try openLxAppSurface(surface, presentation: .normal)
            }
        case .native:
            if surface.content.isNativeTerminal {
                openTerminalMainSurface(surface)
            } else if surface.content.isNativeBrowser {
                try openBrowserMainSurface(surface)
            } else {
                throw LxAppUIError.unsupported("surface \(surface.id) uses unsupported native main content")
            }
        case .url:
            try openBrowserMainSurface(surface)
        case .page:
            throw LxAppUIError.unsupported("surface \(surface.id) uses unsupported main content")
        }
        for main in surfaceById.values where effectiveRole(for: main) == .main {
            visibleSurfaceIDs.remove(main.id)
        }
        openedSurfaceIDs.insert(surface.id)
        visibleSurfaceIDs.insert(surface.id)
        guard setActiveMainSurface(ownerAppId, surface.id) else {
            throw LxAppUIError.invalidConfig("failed to activate main surface \(surface.id)")
        }
        if surface.content.kind == .lxapp, let appId = surface.content.appId {
            shell.mountLxAppMainProvider(appId: appId)
        }
        refreshChromeActions()
    }

    @discardableResult
    private func closeMainSurface(id: String) -> Bool {
        guard let ownerAppId = graphOwnerAppId,
              let previousSwitcher = SurfaceSwitcherBridge.snapshot(ownerAppId: ownerAppId),
              let menu = SurfaceMenuBridge.snapshot(ownerAppId: ownerAppId, surfaceId: id),
              menu.sections.flatMap(\.items).contains(where: {
                  $0.action.owner == "switcher" && $0.action.action == "close"
              })
        else { return false }
        let action = SurfaceMenuBridge.builtInAction("close")
        guard let result = SurfaceMenuBridge.perform(
            ownerAppId: ownerAppId,
            revision: menu.revision,
            surfaceId: id,
            action: action
        ) else { return false }
        applySurfaceMenuExecution(result, previousSwitcher: previousSwitcher)
        return result.accepted && result.removedSurfaceIds.contains(id)
    }

    private func openTerminalMainSurface(_ surface: LxAppUIConfig.Surface) {
        let workspace = terminalWorkspaces[surface.id]
            ?? LingXiaTerminalWorkspaceView(surfaceID: surface.id, presentation: .main)
        terminalWorkspaces[surface.id] = workspace
        shell.setPanelFullscreen(id: surface.id, enabled: false)
        shell.hidePanel(id: surface.id)
        workspace.setPresentation(.main)
        workspace.onRequestClosePanel = nil
        workspace.onToggleSurfaceZoom = nil
        workspace.onActiveTitleChanged = { [weak self] title in
            guard let self, let ownerAppId = self.graphOwnerAppId else { return }
            if setSurfaceAutomaticTitle(ownerAppId, surface.id, title) {
                self.refreshChromeActions()
            }
        }
        workspace.ensureOpenTab()
        if let ownerAppId = graphOwnerAppId, let title = workspace.activeTitle {
            _ = setSurfaceAutomaticTitle(ownerAppId, surface.id, title)
        }
        shell.presentManagedMainView(workspace)
        workspace.focusActiveTerminal()
    }

    private func isBrowserMainSurface(_ surface: LxAppUIConfig.Surface) -> Bool {
        surface.content.kind == .url || surface.content.isNativeBrowser
    }

    private func openBrowserMainSurface(_ surface: LxAppUIConfig.Surface) throws {
        let mode: BrowserTabCoordinator.DeclaredInitialMode
        if surface.content.isNativeBrowser {
            mode = .emptyWorkspace
        } else if surface.content.kind == .url, let url = surface.content.url {
            mode = .url(url)
        } else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) has invalid browser main content")
        }
        guard shell.presentDeclaredBrowserMain(surfaceID: surface.id, mode: mode) else {
            throw LxAppUIError.unsupported("failed to present browser main surface \(surface.id)")
        }
    }

    private func didActivateBrowserMainSurface(id: String) {
        guard let ownerAppId = graphOwnerAppId,
              let surface = surfaceById[id],
              isBrowserMainSurface(surface),
              setActiveMainSurface(ownerAppId, id)
        else { return }
        for main in surfaceById.values where effectiveRole(for: main) == .main {
            visibleSurfaceIDs.remove(main.id)
        }
        openedSurfaceIDs.insert(id)
        visibleSurfaceIDs.insert(id)
        refreshChromeActions()
    }

    private func openIndependentPanelSurface(
        _ surface: LxAppUIConfig.Surface,
        sourceActivatorID: String? = nil
    ) throws {
        guard case .lxapp = surface.content.kind,
              let appId = surface.content.appId,
              !appId.isEmpty else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.appId for lxapp content")
        }

        let panel = independentPanelWindows[surface.id] ?? makeIndependentPanel(for: surface)
        independentPanelWindows[surface.id] = panel
        applyIndependentPanelPresentation(panel, for: surface)
        if let sourceActivatorID {
            independentPanelSourceActivatorIDs[surface.id] = sourceActivatorID
        } else {
            independentPanelSourceActivatorIDs.removeValue(forKey: surface.id)
        }

        let hostView = independentPanelHostViews[surface.id] ?? LxAppHostView(controller: controller)
        independentPanelHostViews[surface.id] = hostView
        if hostView.superview == nil || hostView.window !== panel {
            let container = LxAppHostThemeLayerView(role: .surfaceBackground)
            container.frame = NSRect(origin: .zero, size: panel.contentView?.bounds.size ?? .zero)
            container.layer?.cornerRadius = 10
            container.layer?.masksToBounds = true
            panel.contentView = container

            hostView.translatesAutoresizingMaskIntoConstraints = false
            container.addSubview(hostView)
            NSLayoutConstraint.activate([
                hostView.topAnchor.constraint(equalTo: container.topAnchor),
                hostView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                hostView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                hostView.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            ])
        }

        if openedSurfaceIDs.contains(surface.id) {
            if let pageInstanceId = surfacePageInstanceIDs[surface.id],
               WebViewManager.findWebView(pageInstanceId: pageInstanceId) != nil {
                _ = notifyPageInstanceVisible(pageInstanceId)
                positionIndependentPanel(panel, for: sourceActivatorID)
                panel.orderFrontRegardless()
                visibleSurfaceIDs.insert(surface.id)
                installIndependentPanelOutsideClickMonitorsIfNeeded()
                refreshChromeActions()
                return
            }
            surfacePageInstanceIDs.removeValue(forKey: surface.id)
            openedSurfaceIDs.remove(surface.id)
            visibleSurfaceIDs.remove(surface.id)
            hostView.unmount()
        }

        let path = normalizedPath(try surface.content.resolvedLxAppPath())
        let surfaceID = surface.id
        let requestedSourceActivatorID = sourceActivatorID
        independentPanelDisplayTasks[surface.id]?.cancel()
        independentPanelOpenTasks[surface.id]?.cancel()
        independentPanelOpenTasks[surface.id] = Task { @MainActor [weak self, weak panel] in
            guard let self else { return }
            defer {
                independentPanelOpenTasks[surfaceID] = nil
            }

            do {
                _ = try await controller.open(
                    LxAppOpenRequest(
                        appId: appId,
                        path: path,
                        presentation: .panel,
                        panelId: surfaceID,
                        userInfo: ["appUISurfaceId": .string(surfaceID)]
                    )
                )
            } catch is CancellationError {
                return
            } catch {
                if independentPanelSourceActivatorIDs[surfaceID] == requestedSourceActivatorID {
                    independentPanelSourceActivatorIDs.removeValue(forKey: surfaceID)
                }
                surfacePageInstanceIDs.removeValue(forKey: surfaceID)
                openedSurfaceIDs.remove(surfaceID)
                LXLog.error("AppUI failed to open independent panel surface=\(surfaceID)", category: "MacAppUI", error: error)
                panel?.orderOut(nil)
                visibleSurfaceIDs.remove(surfaceID)
                updateIndependentPanelOutsideClickMonitors()
                refreshChromeActions()
            }
        }
    }

    private func openAttachPanelSurface(_ surface: LxAppUIConfig.Surface) throws {
        if let parentID = surface.attachTo, !visibleSurfaceIDs.contains(parentID) {
            try openSurface(id: parentID)
        }

        if surface.content.isNativeTerminal {
            try openTerminalAttachPanelSurface(surface)
            return
        }

        // A companion (aside) lxapp appears in the sidebar whenever it is shown —
        // registered here (idempotent) so it re-appears on re-open, not only on
        // the first open. Its entry shows/focuses the aside, never the main.
        if surface.content.kind == .lxapp, let appId = surface.content.appId {
            shell.registerAsideLxApp(appId: appId, surfaceId: surface.id)
        }

        if openedSurfaceIDs.contains(surface.id) {
            shell.show()
            // The panel is already registered. Re-enter the graph through the
            // commit path so the layout plan places/shows it at the core edge.
            registerHostAsideForSurface(surface)
            visibleSurfaceIDs.insert(surface.id)
            refreshChromeActions()
            return
        }

        try requestAttachPanelOpenThroughRuntime(surface)
    }

    private func requestAttachPanelOpenThroughRuntime(_ surface: LxAppUIConfig.Surface) throws {
        guard effectiveRole(for: surface) == .aside else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) is not an aside")
        }
        switch surface.content.kind {
        case .lxapp:
            guard let appId = surface.content.appId, !appId.isEmpty else {
                throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.appId for lxapp content")
            }
            openPanelLxapp(surface.id, appId, normalizedPath(try surface.content.resolvedLxAppPath()))
        case .native:
            guard surface.content.isNativeTerminal else {
                throw LxAppUIError.unsupported("surface \(surface.id) uses native content that cannot be presented as an aside")
            }
            try openTerminalAttachPanelSurface(surface)
        case .page, .url:
            throw LxAppUIError.unsupported("surface \(surface.id) uses content that cannot be presented as an aside on macOS")
        }
    }

    private func openLxAppSurface(
        _ surface: LxAppUIConfig.Surface,
        presentation: LxAppOpenPresentation
    ) throws {
        switch surface.content.kind {
        case .lxapp:
            guard let appId = surface.content.appId, !appId.isEmpty else {
                throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.appId for lxapp content")
            }
            let path = normalizedPath(try surface.content.resolvedLxAppPath())
            let panelID: String?
            if case .panel = presentation {
                panelID = surface.id
            } else {
                panelID = nil
            }
            _ = try controller.openSync(
                LxAppOpenRequest(
                    appId: appId,
                    path: path,
                    presentation: presentation,
                    panelId: panelID
                )
            )
        case .page, .url, .native:
            throw LxAppUIError.unsupported("surface \(surface.id) is not lxapp content")
        }
    }

    private func openTerminalAttachPanelSurface(_ surface: LxAppUIConfig.Surface) throws {
        guard let position = panelPosition(for: surface) else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) terminal panel requires a valid aside edge")
        }
        openTerminalPanel(
            id: surface.id,
            position: position,
            defaultHeight: CGFloat(surface.size?.height ?? 320)
        ) { [weak self] in
            self?.registerHostAsideForSurface(surface)
        }
    }

    private func openTerminalPanel(
        id: String,
        position: PanelPosition,
        defaultHeight: CGFloat,
        registerAside: () -> Void
    ) {
        let reused = terminalWorkspaces[id] != nil
        logTerminal(
            "runtime.openTerminal surface=\(id) position=\(position.rawValue) reused=\(reused) defaultHeight=\(String(format: "%.1f", defaultHeight)) windowFrameBefore=\(lxTerminalRuntimeFormatRect(shell.hostWindow?.frame ?? .zero))"
        )
        shell.show()
        let workspace = terminalWorkspaces[id]
            ?? LingXiaTerminalWorkspaceView(surfaceID: id)
        terminalWorkspaces[id] = workspace
        workspace.setPresentation(.aside)
        workspace.onRequestClosePanel = { [weak self] in
            self?.logTerminal("runtime.workspaceRequestedClose surface=\(id)")
            self?.closeTerminalWorkspaceSurface(id: id)
        }
        workspace.onToggleSurfaceZoom = { [weak self] zoomed in
            guard let self else { return }
            self.logTerminal("runtime.toggleSurfaceZoom surface=\(id) enabled=\(zoomed)")
            self.shell.setPanelFullscreen(id: id, enabled: zoomed)
        }
        workspace.ensureOpenTab()
        logTerminal(
            "runtime.beforeShowPanel surface=\(id) workspaceFrame=\(lxTerminalRuntimeFormatRect(workspace.frame)) workspaceBounds=\(lxTerminalRuntimeFormatRect(workspace.bounds)) windowFrame=\(lxTerminalRuntimeFormatRect(shell.hostWindow?.frame ?? .zero))"
        )
        // Register the terminal content (hidden) before mutating the Rust surface
        // graph. registerHostAside below pushes the layout plan that places and
        // shows it.
        shell.registerPanelWithNativeContent(
            id: id,
            position: position,
            contentView: workspace,
            defaultSize: defaultHeight
        )
        registerAside()
        logTerminal(
            "runtime.afterShowPanel surface=\(id) workspaceFrame=\(lxTerminalRuntimeFormatRect(workspace.frame)) workspaceBounds=\(lxTerminalRuntimeFormatRect(workspace.bounds)) windowFrame=\(lxTerminalRuntimeFormatRect(shell.hostWindow?.frame ?? .zero))"
        )
        workspace.focusActiveTerminal()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak workspace, weak shell] in
            self.logTerminal(
                "runtime.delayedFocusTerminal surface=\(id) workspaceFrame=\(lxTerminalRuntimeFormatRect(workspace?.frame ?? .zero)) workspaceBounds=\(lxTerminalRuntimeFormatRect(workspace?.bounds ?? .zero)) windowFrame=\(lxTerminalRuntimeFormatRect(shell?.hostWindow?.frame ?? .zero))"
            )
            workspace?.focusActiveTerminal()
        }
        openedSurfaceIDs.insert(id)
        visibleSurfaceIDs.insert(id)
        logTerminal("runtime.openTerminal markedVisible surface=\(id) opened=\(openedSurfaceIDs.contains(id)) visible=\(visibleSurfaceIDs.contains(id))")
        refreshChromeActions()
    }

    private func registerHostAsideForSurface(_ surface: LxAppUIConfig.Surface) {
        guard let primaryAppId = graphOwnerAppId else { return }
        let edge = managedEdgeOverrides[surface.id] ?? surface.edge
        if surface.content.isNativeTerminal {
            _ = registerHostAsideContent(
                primaryAppId,
                surface.id,
                "terminal",
                edge?.rawValue ?? "bottom"
            )
        } else {
            _ = registerHostAside(primaryAppId, surface.id, edge?.rawValue ?? "right")
        }
    }

    private func closeTerminalWorkspaceSurface(id: String) {
        logTerminal("runtime.closeTerminalWorkspace surface=\(id) windowFrame=\(lxTerminalRuntimeFormatRect(shell.hostWindow?.frame ?? .zero))")
        shell.setPanelFullscreen(id: id, enabled: false)
        terminalWorkspaces[id]?.setSurfaceZoomEnabled(false, notifyRuntime: false)
        terminalWorkspaces[id]?.disarmInput()
        terminalWorkspaces.removeValue(forKey: id)
        openedSurfaceIDs.remove(id)
        visibleSurfaceIDs.remove(id)
        // Drop the terminal from the core graph so the aside-layout reconciler
        // (sole authority) undocks it and never re-shows it on a later
        // present_layout.
        if let primaryAppId = graphOwnerAppId {
            _ = unregisterHostAside(primaryAppId, id)
        }
        shell.unregisterPanel(id: id)
        updateIndependentPanelOutsideClickMonitors()
        refreshChromeActions()
    }

    private func bringSurfaceToFront(id: String) {
        guard visibleSurfaceIDs.contains(id),
              let surface = surfaceById[id] else { return }

        switch effectiveRole(for: surface) {
        case .main, .float:
            if isIndependentPanelSurface(surface) {
                if let panel = independentPanelWindows[id] {
                    panel.orderFrontRegardless()
                    visibleSurfaceIDs.insert(id)
                    installIndependentPanelOutsideClickMonitorsIfNeeded()
                }
            } else {
                shell.show()
            }
        case .aside:
            shell.show()
            shell.showPanel(id: id)
        }
    }

    private func closeSurface(id: String) {
        guard let surface = surfaceById[id] else { return }

        for childID in childrenByParentId[id] ?? [] {
            closeSurface(id: childID)
        }

        switch effectiveRole(for: surface) {
        case .main:
            _ = closeMainSurface(id: id)
            return
        case .float:
            if isIndependentPanelSurface(surface) {
                independentPanelOpenTasks[id]?.cancel()
                independentPanelOpenTasks[id] = nil
                independentPanelDisplayTasks[id]?.cancel()
                independentPanelDisplayTasks[id] = nil
                independentPanelSourceActivatorIDs.removeValue(forKey: id)
                if let pageInstanceId = surfacePageInstanceIDs[id]
                    ?? resolveSurfacePageInstanceId(surface)
                {
                    _ = notifyPageInstanceHidden(pageInstanceId, "programmatic")
                }
                independentPanelWindows[id]?.orderOut(nil)
            } else {
                shell.hide()
                if !shell.hasOpenTabs {
                    discardOpenedSubtree(rootID: id)
                }
            }
        case .aside:
            logTerminal("runtime.closeAttachPanel surface=\(id)")
            // A companion lxapp's sidebar entry is removed when its panel closes.
            if surface.content.kind == .lxapp, let appId = surface.content.appId {
                shell.unregisterAsideLxApp(appId: appId)
            }
            // Drop it from the core surface graph too.
            if let primaryAppId = graphOwnerAppId {
                _ = unregisterHostAside(primaryAppId, id)
            }
            shell.setPanelFullscreen(id: id, enabled: false)
            terminalWorkspaces[id]?.setSurfaceZoomEnabled(false, notifyRuntime: false)
            terminalWorkspaces[id]?.disarmInput()
            shell.hidePanel(id: id)
        }

        visibleSurfaceIDs.remove(id)
        updateIndependentPanelOutsideClickMonitors()
        refreshChromeActions()
    }

    private func logTerminal(_ message: String, type: OSLogType = .info) {
        lxTerminalRuntimeStdoutLog(message)
        let debugEnabled = ProcessInfo.processInfo.environment["LX_TERMINAL_DEBUG_LOGS"] == "1"
        guard debugEnabled || type == .error || type == .fault else {
            return
        }
        os_log("%{public}@", log: Self.log, type: type, message)
    }

    private func discardOpenedSubtree(rootID: String) {
        independentPanelOpenTasks[rootID]?.cancel()
        independentPanelOpenTasks[rootID] = nil
        independentPanelDisplayTasks[rootID]?.cancel()
        independentPanelDisplayTasks[rootID] = nil
        independentPanelSourceActivatorIDs.removeValue(forKey: rootID)
        openedSurfaceIDs.remove(rootID)
        surfacePageInstanceIDs.removeValue(forKey: rootID)
        for childID in childrenByParentId[rootID] ?? [] {
            discardOpenedSubtree(rootID: childID)
        }
        updateIndependentPanelOutsideClickMonitors()
    }

    private func refreshChromeActions() {
        let toolbarItems = toolbarActivators
            .filter { activator in
                guard let hostSurface = activator.hostSurface else { return false }
                return visibleSurfaceIDs.contains(hostSurface)
            }
            .map(makeChromeActionItem)
        let titlebarItems = titlebarActivators
            .filter { activator in
                guard let hostSurface = activator.hostSurface else { return false }
                return visibleSurfaceIDs.contains(hostSurface)
            }
            .map(makeChromeActionItem)

        shell.updateSidebarHeaderActions(runtimeSidebarActionItems(placement: "header"))
        shell.updateSidebarHostActions(runtimeSidebarActionItems(placement: "footer"))
        let switcher = graphOwnerAppId.flatMap(SurfaceSwitcherBridge.snapshot)
        shell.updateManagedMainSurfaces(
            mainSidebarItems(from: switcher),
            activeId: switcher?.activeSurfaceId
        ) { [weak self] surfaceID in
            self?.openSurfaceHandlingError(id: surfaceID)
        } onClose: { [weak self] surfaceID in
            self?.closeMainSurface(id: surfaceID)
        } onAdd: { [weak self] in
            self?.addSurfaceForActiveMain() ?? false
        } onContextMenu: { [weak self] surfaceID, event, view in
            self?.presentSurfaceMenu(surfaceID: surfaceID, event: event, from: view)
        } onRename: { [weak self] surfaceID, title in
            self?.commitSurfaceRename(surfaceID: surfaceID, title: title)
        }
        let activeContentKind = switcher?.items.first(where: { $0.active })?.content.kind
        shell.setManagedNavigationToolbarVisible(activeContentKind == "lxapp")
        shell.updateToolbarHostActions(toolbarItems)
        shell.updateTitlebarHostActions(titlebarItems)
    }

    private func addSurfaceForActiveMain() -> Bool {
        guard let ownerAppId = graphOwnerAppId,
              let snapshot = SurfaceSwitcherBridge.snapshot(ownerAppId: ownerAppId),
              let activeID = snapshot.activeSurfaceId,
              let item = snapshot.items.first(where: { $0.surfaceId == activeID }),
              item.content.kind == "native",
              item.content.capability == "terminal",
              let activeSurface = surfaceById[activeID],
              effectiveRole(for: activeSurface) == .main
        else { return false }
        let instanceKey = UUID().uuidString.lowercased()
        return openManagedNativeSurface(
            id: "native:terminal:\(instanceKey)",
            capability: "terminal",
            instanceKey: instanceKey,
            role: LxAppUIConfig.Role.main.rawValue,
            edge: nil
        )
    }

    private func presentSurfaceMenu(surfaceID: String, event: NSEvent, from view: NSView) {
        guard let ownerAppId = graphOwnerAppId,
              let snapshot = SurfaceMenuBridge.snapshot(
                  ownerAppId: ownerAppId,
                  surfaceId: surfaceID
              )
        else { return }
        surfaceMenuPresenter.present(snapshot, event: event, from: view)
    }

    private func performSurfaceMenuAction(
        revision: UInt64,
        surfaceId: String,
        action: SurfaceMenuSnapshot.Item.Action,
        value: String?
    ) {
        if action.action == "rename", value == nil {
            shell.beginManagedMainRename(surfaceId: surfaceId)
            return
        }
        guard let ownerAppId = graphOwnerAppId,
              let previousSwitcher = SurfaceSwitcherBridge.snapshot(ownerAppId: ownerAppId),
              let result = SurfaceMenuBridge.perform(
                  ownerAppId: ownerAppId,
                  revision: revision,
                  surfaceId: surfaceId,
                  action: action,
                  value: value
              )
        else { return }
        applySurfaceMenuExecution(result, previousSwitcher: previousSwitcher)
    }

    private func commitSurfaceRename(surfaceID: String, title: String) {
        guard let ownerAppId = graphOwnerAppId,
              let snapshot = SurfaceMenuBridge.snapshot(
                  ownerAppId: ownerAppId,
                  surfaceId: surfaceID
              ),
              let item = snapshot.sections
                  .flatMap(\.items)
                  .first(where: { $0.enabled && $0.action.action == "rename" }),
              let result = SurfaceMenuBridge.perform(
                  ownerAppId: ownerAppId,
                  revision: snapshot.revision,
                  surfaceId: surfaceID,
                  action: item.action,
                  value: title
              )
        else {
            refreshChromeActions()
            return
        }
        applySurfaceMenuExecution(result)
    }

    private func applySurfaceMenuExecution(
        _ execution: SurfaceMenuExecution,
        previousSwitcher: SurfaceSwitcherSnapshot? = nil
    ) {
        guard execution.accepted else {
            refreshChromeActions()
            return
        }
        for surfaceId in execution.removedSurfaceIds {
            let previousContent = previousSwitcher?.items.first(where: {
                $0.surfaceId == surfaceId
            })?.content
            discardMainSurfaceContent(id: surfaceId, previousContent: previousContent)
        }
        if !execution.removedSurfaceIds.isEmpty,
           let activeSurfaceId = execution.snapshot.activeSurfaceId,
           let ownerAppId = graphOwnerAppId {
            if surfaceById[activeSurfaceId] != nil,
               !openManagedSurfaceNow(id: activeSurfaceId) {
                refreshChromeActions()
                return
            }
            _ = setActiveMainSurface(ownerAppId, activeSurfaceId)
            if surfaceById[activeSurfaceId] == nil,
               let active = execution.snapshot.items.first(where: {
                   $0.surfaceId == activeSurfaceId
               }),
               active.content.kind == "lxapp",
               let appId = active.content.appId {
                // Closing an active ordinary tab makes the legacy tab manager
                // choose its first remaining item, while the graph chooses the
                // adjacent main. Align the provider and sidebar highlight with
                // the graph's authoritative successor.
                shell.activateMainLxAppProvider(appId: appId)
            }
        }
        refreshChromeActions()
    }

    private func discardMainSurfaceContent(
        id: String,
        previousContent: SurfaceSwitcherSnapshot.Item.Content? = nil
    ) {
        openedSurfaceIDs.remove(id)
        visibleSurfaceIDs.remove(id)
        if let surface = surfaceById[id] {
            if surface.content.isNativeTerminal {
                discardTerminalSurfaceContent(id: id)
            } else if isBrowserMainSurface(surface) {
                shell.closeDeclaredBrowserMain(surfaceID: id)
            } else if surface.content.kind == .lxapp, let appId = surface.content.appId {
                shell.closeManagedMainLxApp(appId: appId)
            }
        } else if previousContent?.kind == "lxapp", let appId = previousContent?.appId {
            shell.closeManagedMainLxApp(appId: appId)
        }
        managedRoleOverrides.removeValue(forKey: id)
        managedEdgeOverrides.removeValue(forKey: id)
    }

    private func discardTerminalSurfaceContent(id: String) {
        shell.setPanelFullscreen(id: id, enabled: false)
        terminalWorkspaces[id]?.setSurfaceZoomEnabled(false, notifyRuntime: false)
        terminalWorkspaces[id]?.disarmInput()
        terminalWorkspaces.removeValue(forKey: id)
        shell.unregisterPanel(id: id)
        managedRoleOverrides.removeValue(forKey: id)
        managedEdgeOverrides.removeValue(forKey: id)
        if !declaredSurfaceIDs.contains(id) {
            nativeInstanceSurfaceIDs = nativeInstanceSurfaceIDs.filter { $0.value != id }
            surfaceById.removeValue(forKey: id)
        }
    }

    /// Project every graph-owned main provider through one sidebar path. A
    /// dynamic lxapp has no YAML entry in `surfaceById`, but it still owns the
    /// same switcher lifecycle as a declared main and must never fall back to
    /// the legacy ordinary-tab close path.
    private func mainSidebarItems(
        from snapshot: SurfaceSwitcherSnapshot?
    ) -> [LxAppUIActionItem] {
        snapshot?.items.compactMap { item in
            guard surfaceById[item.surfaceId] != nil || item.content.kind == "lxapp" else {
                return nil
            }
            let icon = SurfaceSwitcherBridge.resolvedIcon(item.icon)
            return LxAppUIActionItem(
                id: item.surfaceId,
                label: item.title ?? item.surfaceId,
                iconURL: icon.url,
                contentAppId: item.content.appId,
                builtInIcon: icon.builtIn,
                showsLxappTabBar: item.content.kind == "lxapp",
                active: item.active,
                closable: item.closable,
                renameable: item.renameable,
                titleOverridden: item.titleOverridden
            )
        } ?? []
    }

    private func installAppActivationActivators() {
        guard !appActivationActivators.isEmpty else { return }
        appActivationObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didBecomeActiveNotification,
            object: NSApp,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.performAppActivation()
            }
        }
    }

    /// Restore the app on reactivation (dock-icon click, `didBecomeActive`).
    /// Returns whether it handled the activation, so `applicationShouldHandleReopen`
    /// can tell AppKit to skip its own default (which cannot restore an ordered-out
    /// window).
    @discardableResult
    private func performAppActivation() -> Bool {
        guard !handlingAppActivation else { return true }
        handlingAppActivation = true
        defer { handlingAppActivation = false }
        if !appActivationActivators.isEmpty {
            for activator in appActivationActivators {
                performActivator(id: activator.id)
            }
            return true
        }
        // No explicit app-activation activator: for a tray app whose main window was
        // closed to the menu bar, the dock icon must still bring it back. AppKit's
        // default reopen can't re-show an ordered-out window, so restore it here.
        if !menuBarActivators.isEmpty, !visibleSurfaceIDs.contains(rootSurface.id) {
            openSurfaceHandlingError(id: rootSurface.id)
            return true
        }
        return false
    }

    private func makeChromeActionItem(_ activator: LxAppUIConfig.Activator) -> LxAppUIActionItem {
        LxAppUIActionItem(
            id: activator.id,
            label: activator.label ?? activator.id,
            iconURL: resolvedIconURL(for: activator)
        )
    }

    private func resolvedIconURL(for activator: LxAppUIConfig.Activator) -> URL? {
        guard let icon = activator.icon else { return nil }
        return LxAppAppUIBundleLoader.resolveRelativeResource(icon, baseURL: uiConfigURL)
    }

    private func applyWindowPresentation(
        for surface: LxAppUIConfig.Surface,
        role: LxAppUIConfig.Role
    ) {
        let size = resolvedWindowSize(for: surface)
        let isResizable = surface.resizable ?? true
        let showTrafficLights = surface.showTrafficLights ?? (role == .main)
        shell.applyManagedWindowPresentation(
            title: appConfig.productName,
            size: size,
            resizable: isResizable,
            role: role,
            showTrafficLights: showTrafficLights
        )
    }

    private func positionPanelWindow(for activatorID: String?) {
        guard let window = shell.window else { return }
        positionWindow(window, for: activatorID)
    }

    private func positionIndependentPanel(_ panel: NSPanel, for activatorID: String?) {
        positionWindow(panel, for: activatorID)
    }

    private func positionWindow(_ window: NSWindow, for activatorID: String?) {
        let resolvedActivatorID = activatorID ?? trayController.defaultActivatorID
        guard let resolvedActivatorID,
              let button = trayController.button(for: resolvedActivatorID),
              let statusWindow = button.window else { return }

        let buttonFrameInScreen = statusWindow.convertToScreen(button.frame)
        var frame = window.frame
        frame.origin.x = round(buttonFrameInScreen.midX - frame.width / 2)
        frame.origin.y = round(buttonFrameInScreen.minY - frame.height - 6)

        if let screenFrame = statusWindow.screen?.visibleFrame {
            frame.origin.x = min(max(frame.origin.x, screenFrame.minX + 8), screenFrame.maxX - frame.width - 8)
            frame.origin.y = max(frame.origin.y, screenFrame.minY + 8)
        }

        window.setFrame(frame, display: false)
    }

    private func visibleIndependentPanelIDs() -> [String] {
        visibleSurfaceIDs.filter { id in
            guard let surface = surfaceById[id], isIndependentPanelSurface(surface) else {
                return false
            }
            return independentPanelWindows[id]?.isVisible == true
        }
    }

    private func dismissVisibleIndependentPanels() {
        for id in visibleIndependentPanelIDs() {
            _ = hideManagedSurface(id: id, updateGraph: true)
        }
    }

    private func eventScreenPoint(_ event: NSEvent) -> NSPoint {
        if let window = event.window {
            return window.convertPoint(toScreen: event.locationInWindow)
        }
        return event.locationInWindow
    }

    private func pointInAnyStatusItemButton(_ point: NSPoint) -> Bool {
        trayController.anyButtonContains(screenPoint: point)
    }

    private func dismissIndependentPanelsIfNeeded(for event: NSEvent) {
        let visiblePanels = visibleIndependentPanelIDs()
        guard !visiblePanels.isEmpty else { return }

        let point = eventScreenPoint(event)
        if pointInAnyStatusItemButton(point) {
            return
        }

        for id in visiblePanels {
            if let panel = independentPanelWindows[id], panel.frame.contains(point) {
                return
            }
        }

        for id in visiblePanels {
            _ = hideManagedSurface(id: id, updateGraph: true)
        }
    }

    private func installIndependentPanelOutsideClickMonitorsIfNeeded() {
        if independentPanelOutsideClickGlobalMonitor == nil {
            independentPanelOutsideClickGlobalMonitor = NSEvent.addGlobalMonitorForEvents(
                matching: [.leftMouseDown, .rightMouseDown]
            ) { [weak self] event in
                Task { @MainActor [weak self] in
                    self?.dismissIndependentPanelsIfNeeded(for: event)
                }
            }
        }

        if independentPanelOutsideClickLocalMonitor == nil {
            independentPanelOutsideClickLocalMonitor = NSEvent.addLocalMonitorForEvents(
                matching: [.leftMouseDown, .rightMouseDown]
            ) { [weak self] event in
                self?.dismissIndependentPanelsIfNeeded(for: event)
                return event
            }
        }
    }

    private func removeIndependentPanelOutsideClickMonitors() {
        if let monitor = independentPanelOutsideClickGlobalMonitor {
            NSEvent.removeMonitor(monitor)
            independentPanelOutsideClickGlobalMonitor = nil
        }
        if let monitor = independentPanelOutsideClickLocalMonitor {
            NSEvent.removeMonitor(monitor)
            independentPanelOutsideClickLocalMonitor = nil
        }
    }

    private func updateIndependentPanelOutsideClickMonitors() {
        if visibleIndependentPanelIDs().isEmpty {
            removeIndependentPanelOutsideClickMonitors()
        }
    }

    private func makeIndependentPanel(for surface: LxAppUIConfig.Surface) -> NSPanel {
        let size = resolvedWindowSize(for: surface) ?? CGSize(width: 360, height: 420)
        let panel = NSPanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        applyIndependentPanelPresentation(panel, for: surface)
        return panel
    }

    private func applyIndependentPanelPresentation(_ panel: NSPanel, for surface: LxAppUIConfig.Surface) {
        let size = resolvedWindowSize(for: surface) ?? CGSize(width: 360, height: 420)
        let resizable = surface.resizable ?? true
        panel.styleMask = resizable
            ? [.borderless, .nonactivatingPanel, .resizable]
            : [.borderless, .nonactivatingPanel]
        panel.title = appConfig.productName
        panel.level = .statusBar
        panel.collectionBehavior = [.transient, .moveToActiveSpace]
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.hasShadow = true
        // The rounded content view defines the panel silhouette. Keeping an
        // opaque window background paints the rectangular pixels exposed by
        // its clipped corners (most visibly at the bottom edge) and gives the
        // borderless tray panel a square shadow. A clear backing lets AppKit
        // derive both the visible shape and shadow from the rounded content.
        panel.backgroundColor = .clear
        panel.isOpaque = false
        if resizable {
            panel.contentMinSize = CGSize(width: 240, height: 180)
            panel.contentMaxSize = CGSize(
                width: CGFloat.greatestFiniteMagnitude,
                height: CGFloat.greatestFiniteMagnitude
            )
            panel.minSize = CGSize(width: 240, height: 180)
            panel.maxSize = CGSize(
                width: CGFloat.greatestFiniteMagnitude,
                height: CGFloat.greatestFiniteMagnitude
            )
        } else {
            panel.contentMinSize = size
            panel.contentMaxSize = size
            panel.minSize = size
            panel.maxSize = size
        }
        panel.setContentSize(size)
        for type in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            panel.standardWindowButton(type)?.isHidden = true
        }
    }

    private func isIndependentPanelSurface(_ surface: LxAppUIConfig.Surface) -> Bool {
        surface.role == .float && surface.anchor == .activator
    }

    private func resolveSurfacePageInstanceId(
        _ surface: LxAppUIConfig.Surface,
        appIdHint: String? = nil,
        pathHint: String? = nil,
        sessionIdHint: UInt64? = nil
    ) -> String? {
        guard case .lxapp = surface.content.kind else { return nil }
        let appId = (appIdHint ?? surface.content.appId ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        guard !appId.isEmpty else { return nil }

        let configuredPath = pathHint ?? (try? surface.content.resolvedLxAppPath())
        let normalized = normalizedPath(configuredPath)
        let sessionId = sessionIdHint ?? shell.resolvedSessionId(for: appId) ?? 0
        guard sessionId > 0 else { return nil }

        return WebViewManager.resolvePageInstanceId(
            appId: appId,
            path: normalized,
            sessionId: sessionId
        )
    }

    private func resolvedWindowSize(for surface: LxAppUIConfig.Surface) -> CGSize? {
        guard let size = surface.size,
              let width = size.width,
              let height = size.height,
              width > 0,
              height > 0 else {
            return nil
        }
        return CGSize(width: width, height: height)
    }

    private func normalizedPath(_ path: String?) -> String {
        guard let path else { return "" }
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty || trimmed == "/" {
            return ""
        }
        return trimmed
    }

    private func panelPosition(for surface: LxAppUIConfig.Surface) -> PanelPosition? {
        guard effectiveRole(for: surface) == .aside else { return nil }
        switch managedEdgeOverrides[surface.id] ?? surface.edge {
        case .left:
            return .left
        case .right:
            return .right
        case .bottom:
            return .bottom
        case .top:
            return .top
        case .none:
            return nil
        }
    }

    private static func validate(
        bundleConfig: LxAppGeneratedBundleConfig
    ) throws -> ValidationResult {
        let ui = bundleConfig.ui

        if ui.launch.initialSurface.isEmpty {
            throw LxAppUIError.invalidConfig("launch.initialSurface cannot be empty")
        }

        var allSurfaceIDs = Set<String>()
        for surface in ui.surfaces {
            guard !surface.id.isEmpty else {
                throw LxAppUIError.invalidConfig("surface id cannot be empty")
            }
            guard allSurfaceIDs.insert(surface.id).inserted else {
                throw LxAppUIError.invalidConfig("duplicate surface id \(surface.id)")
            }
        }

        let availableSurfaces = ui.surfaces.filter { $0.isAvailable(on: "macos") }
        let skippedSurfaceIds = Set(
            ui.surfaces
                .filter { !$0.isAvailable(on: "macos") }
                .map(\.id)
        )

        var surfaceById: [String: LxAppUIConfig.Surface] = [:]
        var seenAppIDs = Set<String>()

        for surface in availableSurfaces {
            switch surface.content.kind {
            case .lxapp:
                guard let appId = surface.content.appId, !appId.isEmpty else {
                    throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.appId")
                }
                if seenAppIDs.contains(appId) {
                    throw LxAppUIError.unsupported("macOS app UI currently requires unique lxapp content.appId values; duplicate \(appId)")
                }
                seenAppIDs.insert(appId)
            case .page:
                throw LxAppUIError.unsupported("declarative page surface \(surface.id) is not supported on macOS")
            case .url:
                guard let url = surface.content.url, !url.isEmpty else {
                    throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.url")
                }
                guard surface.role == .main else {
                    throw LxAppUIError.unsupported("URL surface \(surface.id) must use role main on macOS")
                }
            case .native:
                guard let name = surface.content.name else {
                    throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.name")
                }
                if name == .browser && surface.role != .main {
                    throw LxAppUIError.unsupported("native browser surface \(surface.id) must use role main on macOS")
                }
            }

            if surface.anchor != nil && surface.role != .float {
                throw LxAppUIError.invalidConfig("surface \(surface.id) can set anchor only when role is float")
            }
            if surface.role == .float && surface.anchor != .activator {
                throw LxAppUIError.invalidConfig("surface \(surface.id) with role float requires anchor: activator")
            }

            surfaceById[surface.id] = surface
        }

        guard !surfaceById.isEmpty else {
            throw LxAppUIError.invalidConfig("surfaces must include at least one surface available on macOS")
        }

        guard let initialSurface = surfaceById[ui.launch.initialSurface] else {
            if skippedSurfaceIds.contains(ui.launch.initialSurface) {
                throw LxAppUIError.invalidConfig("launch.initialSurface \(ui.launch.initialSurface) is not available on macOS")
            }
            throw LxAppUIError.invalidConfig("launch.initialSurface references unknown surface \(ui.launch.initialSurface)")
        }
        guard initialSurface.role == .main || initialSurface.role == .float else {
            throw LxAppUIError.unsupported("launch.initialSurface must reference a supported macOS surface")
        }

        let mainSurfaces = availableSurfaces.filter { $0.role == .main }
        let floatSurfaces = availableSurfaces.filter { $0.role == .float }
        if mainSurfaces.isEmpty && floatSurfaces.count != 1 {
            throw LxAppUIError.unsupported("macOS app UI requires at least one main or one float root")
        }
        if mainSurfaces.count > 1 {
            throw LxAppUIError.unsupported("macOS app UI requires exactly one declared main surface")
        }
        if !mainSurfaces.isEmpty && !floatSurfaces.isEmpty {
            throw LxAppUIError.unsupported("macOS app UI cannot combine main surfaces with a float root")
        }
        let rootSurface = initialSurface

        var childrenByParentId: [String: [String]] = [:]

        for surface in availableSurfaces {
            if surface.content.isNativeTerminal {
                if surface.role == .aside {
                    guard surface.edge == .bottom || surface.edge == .top else {
                        throw LxAppUIError.unsupported("terminal surface \(surface.id) must use edge top or bottom")
                    }
                } else if surface.role != .main {
                    throw LxAppUIError.unsupported("terminal surface \(surface.id) must use role main or aside")
                }
            }

            switch surface.role {
            case .main, .float:
                if surface.attachTo != nil {
                    throw LxAppUIError.invalidConfig("root window surface \(surface.id) cannot set attachTo")
                }
            case .aside:
                guard let parentID = surface.attachTo, !parentID.isEmpty else {
                    throw LxAppUIError.invalidConfig("aside surface \(surface.id) requires attachTo")
                }
                guard let parent = surfaceById[parentID] else {
                    throw LxAppUIError.invalidConfig("surface \(surface.id) attaches to unknown surface \(parentID)")
                }
                guard parent.role == .main else {
                    throw LxAppUIError.unsupported("macOS v1 does not support aside -> aside; surface \(surface.id) attaches to \(parentID)")
                }
                guard parent.id == rootSurface.id else {
                    throw LxAppUIError.unsupported("macOS v1 only supports asides attached to the root window surface")
                }
                guard surface.edge != nil else {
                    throw LxAppUIError.invalidConfig("aside surface \(surface.id) requires edge")
                }
                childrenByParentId[parentID, default: []].append(surface.id)
            }
        }

        var seenActivatorIDs = Set<String>()
        var menuBarActivators: [LxAppUIConfig.Activator] = []
        var appActivationActivators: [LxAppUIConfig.Activator] = []
        var sidebarActivators: [LxAppUIConfig.Activator] = []
        var toolbarActivators: [LxAppUIConfig.Activator] = []
        var titlebarActivators: [LxAppUIConfig.Activator] = []

        for activator in ui.activators {
            guard !activator.id.isEmpty else {
                throw LxAppUIError.invalidConfig("activator id cannot be empty")
            }
            if skippedSurfaceIds.contains(activator.action.surface) {
                continue
            }
            guard surfaceById[activator.action.surface] != nil else {
                throw LxAppUIError.invalidConfig("activator \(activator.id) references unknown surface \(activator.action.surface)")
            }

            switch activator.kind {
            case .menuBarItem:
                if activator.hostSurface != nil {
                    throw LxAppUIError.invalidConfig("menuBarItem activator \(activator.id) cannot set hostSurface")
                }
            case .appActivation:
                if activator.hostSurface != nil {
                    throw LxAppUIError.invalidConfig("appActivation activator \(activator.id) cannot set hostSurface")
                }
            case .sidebarItem:
                guard let hostSurface = activator.hostSurface else {
                    throw LxAppUIError.invalidConfig("sidebarItem activator \(activator.id) requires a valid hostSurface")
                }
                if skippedSurfaceIds.contains(hostSurface) {
                    continue
                }
                guard surfaceById[hostSurface] != nil else {
                    throw LxAppUIError.invalidConfig("sidebarItem activator \(activator.id) requires a valid hostSurface")
                }
            case .toolbarItem:
                guard let hostSurface = activator.hostSurface else {
                    throw LxAppUIError.invalidConfig("toolbarItem activator \(activator.id) requires a valid hostSurface")
                }
                if skippedSurfaceIds.contains(hostSurface) {
                    continue
                }
                guard surfaceById[hostSurface] != nil else {
                    throw LxAppUIError.invalidConfig("toolbarItem activator \(activator.id) requires a valid hostSurface")
                }
            case .titlebarItem:
                guard let hostSurface = activator.hostSurface else {
                    throw LxAppUIError.invalidConfig("titlebarItem activator \(activator.id) requires a valid hostSurface")
                }
                if skippedSurfaceIds.contains(hostSurface) {
                    continue
                }
                guard surfaceById[hostSurface] != nil else {
                    throw LxAppUIError.invalidConfig("titlebarItem activator \(activator.id) requires a valid hostSurface")
                }
            }
            guard seenActivatorIDs.insert(activator.id).inserted else {
                throw LxAppUIError.invalidConfig("duplicate activator id \(activator.id)")
            }

            switch activator.kind {
            case .menuBarItem:
                menuBarActivators.append(activator)
            case .appActivation:
                appActivationActivators.append(activator)
            case .sidebarItem:
                sidebarActivators.append(activator)
            case .toolbarItem:
                toolbarActivators.append(activator)
            case .titlebarItem:
                titlebarActivators.append(activator)
            }
        }

        return ValidationResult(
            rootSurface: rootSurface,
            surfaceById: surfaceById,
            childrenByParentId: childrenByParentId,
            menuBarActivators: menuBarActivators,
            appActivationActivators: appActivationActivators,
            sidebarActivators: sidebarActivators,
            toolbarActivators: toolbarActivators,
            titlebarActivators: titlebarActivators
        )
    }
}

private struct ValidationResult {
    let rootSurface: LxAppUIConfig.Surface
    let surfaceById: [String: LxAppUIConfig.Surface]
    let childrenByParentId: [String: [String]]
    let menuBarActivators: [LxAppUIConfig.Activator]
    let appActivationActivators: [LxAppUIConfig.Activator]
    let sidebarActivators: [LxAppUIConfig.Activator]
    let toolbarActivators: [LxAppUIConfig.Activator]
    let titlebarActivators: [LxAppUIConfig.Activator]
}
#endif
