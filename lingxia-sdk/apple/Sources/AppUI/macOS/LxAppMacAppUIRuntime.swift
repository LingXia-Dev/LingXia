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
    let id: String
    let label: String
    let iconURL: URL?
}

@MainActor
final class LxAppMacAppUIRuntime: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "MacAppUI")
    private static let panelFirstPaintPollNs: UInt64 = 50_000_000
    private static let panelFirstPaintMaxPolls = 24
    private static let panelFirstPaintSettleNs: UInt64 = 16_000_000

    nonisolated(unsafe) static weak var active: LxAppMacAppUIRuntime?

    let appConfig: LxAppGeneratedAppConfig
    let uiConfig: LxAppUIConfig
    let controller: LxAppController
    let shell: LxAppShell
    let uiConfigURL: URL

    private let rootSurface: LxAppUIConfig.Surface
    private let surfaceById: [String: LxAppUIConfig.Surface]
    private let childrenByParentId: [String: [String]]
    private let menuBarActivators: [LxAppUIConfig.Activator]
    private let appActivationActivators: [LxAppUIConfig.Activator]
    private let sidebarActivators: [LxAppUIConfig.Activator]
    private let toolbarActivators: [LxAppUIConfig.Activator]
    private let titlebarActivators: [LxAppUIConfig.Activator]

    private var visibleSurfaceIDs = Set<String>()
    private var openedSurfaceIDs = Set<String>()
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
    nonisolated(unsafe) private var independentPanelOutsideClickGlobalMonitor: Any?
    nonisolated(unsafe) private var independentPanelOutsideClickLocalMonitor: Any?
    nonisolated(unsafe) private var appActivationObserver: NSObjectProtocol?
    private var handlingAppActivation = false

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
        self.childrenByParentId = validation.childrenByParentId
        self.menuBarActivators = validation.menuBarActivators
        self.appActivationActivators = validation.appActivationActivators
        self.sidebarActivators = validation.sidebarActivators
        self.toolbarActivators = validation.toolbarActivators
        self.titlebarActivators = validation.titlebarActivators

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
        shell.setSidebarHostActionHandler { [weak self] actionID in
            self?.performActivator(id: actionID)
        }
        shell.setToolbarHostActionHandler { [weak self] actionID in
            self?.performActivator(id: actionID)
        }
        shell.setTitlebarHostActionHandler { [weak self] actionID in
            self?.performActivator(id: actionID)
        }
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
        trayController.installMenuBarActivators(menuBarActivators)
        installAppActivationActivators()
        refreshChromeActivators()
        if uiConfig.launch.openOnLaunch ?? true {
            try openSurface(id: uiConfig.launch.initialSurface)
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
        guard let surface = surfaceById[panelId] else {
            return false
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
                os_log(
                    "independent panel missing page instance id panel=%{public}@ appId=%{public}@ path=%{public}@",
                    log: Self.log,
                    type: .error,
                    panelId,
                    appId,
                    path
                )
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
                    refreshChromeActivators()
                } catch is CancellationError {
                    return
                } catch {
                    surfacePageInstanceIDs.removeValue(forKey: panelId)
                    openedSurfaceIDs.remove(panelId)
                    visibleSurfaceIDs.remove(panelId)
                    updateIndependentPanelOutsideClickMonitors()
                    os_log(
                        "independent panel webview mount failed panel=%{public}@ appId=%{public}@ path=%{public}@ error=%{public}@",
                        log: Self.log,
                        type: .error,
                        panelId,
                        appId,
                        path,
                        String(describing: error)
                    )
                }
            }
            return true
        }

        guard surface.role == .aside,
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
        refreshChromeActivators()
        return true
    }

    private func handleRootWindowCloseRequest() {
        if menuBarActivators.isEmpty {
            NSApp.terminate(nil)
            return
        }
        closeSurface(id: rootSurface.id)
    }

    private func performActivator(id: String) {
        guard let activator = uiConfig.activators.first(where: { $0.id == id }) else { return }

        switch activator.action.kind {
        case .toggleSurface:
            if let surface = surfaceById[activator.action.surface],
               isIndependentPanelSurface(surface),
               independentPanelWindows[surface.id]?.isVisible == true {
                closeSurface(id: surface.id)
                return
            }
            toggleSurface(id: activator.action.surface, sourceActivatorID: activator.id)
        case .openSurface:
            openSurfaceHandlingError(id: activator.action.surface, sourceActivatorID: activator.id)
        }
    }

    private func toggleSurface(id: String, sourceActivatorID: String? = nil) {
        if visibleSurfaceIDs.contains(id) {
            closeSurface(id: id)
        } else {
            openSurfaceHandlingError(id: id, sourceActivatorID: sourceActivatorID)
        }
    }

    /// Toggle a host-declared surface's visibility. Returns `false` if `id` is
    /// not a declared surface, so the caller can report the failure.
    @discardableResult
    func toggleManagedSurface(id: String) -> Bool {
        guard surfaceById[id] != nil else { return false }
        toggleSurface(id: id)
        return true
    }

    /// Show a host-declared surface (no-op if already visible). Returns `false`
    /// for an unknown surface `id`.
    @discardableResult
    func openManagedSurface(id: String) -> Bool {
        guard surfaceById[id] != nil else { return false }
        if !visibleSurfaceIDs.contains(id) {
            openSurfaceHandlingError(id: id)
        }
        return true
    }

    /// Hide a host-declared surface (no-op if already hidden). Returns `false`
    /// for an unknown surface `id`.
    @discardableResult
    func closeManagedSurface(id: String) -> Bool {
        guard surfaceById[id] != nil else { return false }
        if visibleSurfaceIDs.contains(id) {
            closeSurface(id: id)
        }
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
            os_log(
                "AppUI failed to open surface=%{public}@ activator=%{public}@ error=%{public}@",
                log: Self.log,
                type: .error,
                id,
                sourceActivatorID ?? "nil",
                String(describing: error)
            )
        }
    }

    private func openSurface(id: String, sourceActivatorID: String? = nil) throws {
        guard let surface = surfaceById[id] else {
            throw LxAppUIError.invalidConfig("unknown surface id \(id)")
        }

        switch surface.role {
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
        applyWindowPresentation(for: surface)
        if surface.role == .float {
            positionPanelWindow(for: sourceActivatorID)
        }

        if openedSurfaceIDs.contains(surface.id) {
            shell.show()
            visibleSurfaceIDs.insert(surface.id)
            refreshChromeActivators()
            return
        }

        shell.show()
        try openLxAppSurface(surface, presentation: .normal)
        openedSurfaceIDs.insert(surface.id)
        visibleSurfaceIDs.insert(surface.id)
        refreshChromeActivators()
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
            let container = NSView(frame: NSRect(origin: .zero, size: panel.contentView?.bounds.size ?? .zero))
            container.wantsLayer = true
            container.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
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
                refreshChromeActivators()
                return
            }
            surfacePageInstanceIDs.removeValue(forKey: surface.id)
            openedSurfaceIDs.remove(surface.id)
            visibleSurfaceIDs.remove(surface.id)
            hostView.unmount()
        }

        let path = normalizedPath(surface.content.path)
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
                os_log(
                    "AppUI failed to open independent panel surface=%{public}@ error=%{public}@",
                    log: Self.log,
                    type: .error,
                    surfaceID,
                    String(describing: error)
                )
                panel?.orderOut(nil)
                visibleSurfaceIDs.remove(surfaceID)
                updateIndependentPanelOutsideClickMonitors()
                refreshChromeActivators()
            }
        }
    }

    private func openAttachPanelSurface(_ surface: LxAppUIConfig.Surface) throws {
        if let parentID = surface.attachTo, !visibleSurfaceIDs.contains(parentID) {
            try openSurface(id: parentID)
        }

        if surface.content.kind == .terminal {
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
            refreshChromeActivators()
            return
        }

        try requestAttachPanelOpenThroughRuntime(surface)
    }

    private func requestAttachPanelOpenThroughRuntime(_ surface: LxAppUIConfig.Surface) throws {
        guard surface.role == .aside else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) is not an aside")
        }
        switch surface.content.kind {
        case .lxapp:
            guard let appId = surface.content.appId, !appId.isEmpty else {
                throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.appId for lxapp content")
            }
            openPanelLxapp(surface.id, appId, normalizedPath(surface.content.path))
        case .terminal:
            try openTerminalAttachPanelSurface(surface)
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
            let path = normalizedPath(surface.content.path)
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
        case .terminal:
            throw LxAppUIError.unsupported("surface \(surface.id) uses unsupported terminal content on macOS")
        }
    }

    private func openTerminalAttachPanelSurface(_ surface: LxAppUIConfig.Surface) throws {
        guard let position = panelPosition(for: surface) else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) terminal panel requires a valid aside edge")
        }
        let reused = terminalWorkspaces[surface.id] != nil
        let defaultHeight = CGFloat(surface.size?.height ?? 320)
        logTerminal(
            "runtime.openTerminal surface=\(surface.id) position=\(position.rawValue) reused=\(reused) defaultHeight=\(String(format: "%.1f", defaultHeight)) windowFrameBefore=\(lxTerminalRuntimeFormatRect(shell.hostWindow?.frame ?? .zero))"
        )
        shell.show()
        let workspace = terminalWorkspaces[surface.id]
            ?? LingXiaTerminalWorkspaceView(surfaceID: surface.id)
        terminalWorkspaces[surface.id] = workspace
        workspace.onRequestClosePanel = { [weak self] in
            self?.logTerminal("runtime.workspaceRequestedClose surface=\(surface.id)")
            self?.closeTerminalWorkspaceSurface(id: surface.id)
        }
        workspace.onToggleSurfaceZoom = { [weak self] zoomed in
            guard let self else { return }
            self.logTerminal("runtime.toggleSurfaceZoom surface=\(surface.id) enabled=\(zoomed)")
            self.shell.setPanelFullscreen(id: surface.id, enabled: zoomed)
        }
        workspace.ensureOpenTab()
        logTerminal(
            "runtime.beforeShowPanel surface=\(surface.id) workspaceFrame=\(lxTerminalRuntimeFormatRect(workspace.frame)) workspaceBounds=\(lxTerminalRuntimeFormatRect(workspace.bounds)) windowFrame=\(lxTerminalRuntimeFormatRect(shell.hostWindow?.frame ?? .zero))"
        )
        // Register the terminal content (hidden) before mutating the Rust surface
        // graph. registerHostAside below pushes the layout plan that places and
        // shows it.
        shell.registerPanelWithNativeContent(
            id: surface.id,
            position: position,
            contentView: workspace,
            defaultSize: defaultHeight
        )
        registerHostAsideForSurface(surface)
        logTerminal(
            "runtime.afterShowPanel surface=\(surface.id) workspaceFrame=\(lxTerminalRuntimeFormatRect(workspace.frame)) workspaceBounds=\(lxTerminalRuntimeFormatRect(workspace.bounds)) windowFrame=\(lxTerminalRuntimeFormatRect(shell.hostWindow?.frame ?? .zero))"
        )
        workspace.focusActiveTerminal()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak workspace, weak shell] in
            self.logTerminal(
                "runtime.delayedFocusTerminal surface=\(surface.id) workspaceFrame=\(lxTerminalRuntimeFormatRect(workspace?.frame ?? .zero)) workspaceBounds=\(lxTerminalRuntimeFormatRect(workspace?.bounds ?? .zero)) windowFrame=\(lxTerminalRuntimeFormatRect(shell?.hostWindow?.frame ?? .zero))"
            )
            workspace?.focusActiveTerminal()
        }
        openedSurfaceIDs.insert(surface.id)
        visibleSurfaceIDs.insert(surface.id)
        logTerminal("runtime.openTerminal markedVisible surface=\(surface.id) opened=\(openedSurfaceIDs.contains(surface.id)) visible=\(visibleSurfaceIDs.contains(surface.id))")
        refreshChromeActivators()
    }

    private func registerHostAsideForSurface(_ surface: LxAppUIConfig.Surface) {
        guard let primaryAppId = rootSurface.content.appId else { return }
        _ = registerHostAside(primaryAppId, surface.id, surface.edge?.rawValue ?? "right")
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
        if let primaryAppId = rootSurface.content.appId {
            _ = unregisterHostAside(primaryAppId, id)
        }
        shell.hidePanel(id: id)
        updateIndependentPanelOutsideClickMonitors()
        refreshChromeActivators()
    }

    private func bringSurfaceToFront(id: String) {
        guard visibleSurfaceIDs.contains(id),
              let surface = surfaceById[id] else { return }

        switch surface.role {
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

        switch surface.role {
        case .main, .float:
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
            if let primaryAppId = rootSurface.content.appId {
                _ = unregisterHostAside(primaryAppId, id)
            }
            shell.setPanelFullscreen(id: id, enabled: false)
            terminalWorkspaces[id]?.setSurfaceZoomEnabled(false, notifyRuntime: false)
            terminalWorkspaces[id]?.disarmInput()
            shell.hidePanel(id: id)
        }

        visibleSurfaceIDs.remove(id)
        updateIndependentPanelOutsideClickMonitors()
        refreshChromeActivators()
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

    private func refreshChromeActivators() {
        let sidebarItems = sidebarActivators
            .filter { activator in
                guard let hostSurface = activator.hostSurface else { return false }
                return visibleSurfaceIDs.contains(hostSurface)
            }
            .map(makeChromeActionItem)

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

        shell.updateSidebarHostActions(sidebarItems)
        shell.setManagedNavigationToolbarVisible(true)
        shell.updateToolbarHostActions(toolbarItems)
        shell.updateTitlebarHostActions(titlebarItems)
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

    private func applyWindowPresentation(for surface: LxAppUIConfig.Surface) {
        let size = resolvedWindowSize(for: surface)
        let isResizable = surface.resizable ?? true
        let showTrafficLights = surface.showTrafficLights ?? (surface.role == .main)
        shell.applyManagedWindowPresentation(
            title: appConfig.productName,
            size: size,
            resizable: isResizable,
            role: surface.role,
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
            closeSurface(id: id)
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
        panel.backgroundColor = .windowBackgroundColor
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

        let normalized = normalizedPath(pathHint ?? surface.content.path)
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
        guard surface.role == .aside else { return nil }
        switch surface.edge {
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
            case .terminal:
                break
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
        guard initialSurface.role == .main
            || initialSurface.role == .float
            || initialSurface.role == .aside else {
            throw LxAppUIError.unsupported("launch.initialSurface must reference a supported macOS surface")
        }

        let windowSurfaces = availableSurfaces.filter {
            $0.role == .main || $0.role == .float
        }
        guard windowSurfaces.count == 1, let rootSurface = windowSurfaces.first else {
            throw LxAppUIError.unsupported("macOS app UI currently requires exactly one root window surface")
        }

        var childrenByParentId: [String: [String]] = [:]

        for surface in availableSurfaces {
            if surface.content.kind == .terminal {
                guard surface.role == .aside else {
                    throw LxAppUIError.unsupported("terminal surface \(surface.id) must use role aside")
                }
                guard surface.edge == .bottom || surface.edge == .top else {
                    throw LxAppUIError.unsupported("terminal surface \(surface.id) must use edge top or bottom")
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
                guard parent.role == .main || parent.role == .float else {
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
