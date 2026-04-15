#if os(macOS)
import AppKit
import CLingXiaRustAPI
import OSLog

@MainActor
struct LxAppUIActionItem: Sendable {
    let id: String
    let label: String
    let iconURL: URL?
}

@MainActor
final class LxAppMacAppUIRuntime: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "MacAppUI")

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
    private var statusItems: [String: NSStatusItem] = [:]
    private var defaultMenuBarActivatorID: String?
    private var sidebarChromeEnabled = false
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
        shell.setSidebarHostActionHandler { [weak self] actionID in
            self?.performActivator(id: actionID)
        }
        shell.setToolbarHostActionHandler { [weak self] actionID in
            self?.performActivator(id: actionID)
        }
        shell.setSidebarChromeEnabled(!sidebarActivators.isEmpty)
        sidebarChromeEnabled = !sidebarActivators.isEmpty

        Self.active = self
    }

    func start() throws {
        if menuBarActivators.isEmpty || !appActivationActivators.isEmpty {
            NSApp.setActivationPolicy(.regular)
        } else {
            NSApp.setActivationPolicy(.accessory)
        }
        installMenuBarActivators()
        installAppActivationActivators()
        refreshChromeActivators()
        if uiConfig.launch.openOnLaunch ?? true {
            try openSurface(id: uiConfig.launch.initialSurface)
        }
    }

    deinit {
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
        active.performAppActivation()
        return !active.appActivationActivators.isEmpty
    }

    private func handleOpenedPanel(
        appId: String,
        path: String,
        sessionId: UInt64,
        panelId: String
    ) -> Bool {
        guard let surface = surfaceById[panelId],
              surface.presentation.style == .attachedPanel,
              let position = panelPosition(for: surface) else {
            return false
        }

        shell.appSessions[appId] = sessionId
        LxAppCore.setSessionId(sessionId, for: appId)
        shell.showPanelWithContent(id: panelId, position: position, appId: appId, path: path)
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
            toggleSurface(id: activator.action.surface, sourceActivatorID: activator.id)
        case .openSurface:
            openSurfaceHandlingError(id: activator.action.surface, sourceActivatorID: activator.id)
        case .closeSurface:
            closeSurface(id: activator.action.surface)
        case .focusSurface:
            focusSurface(id: activator.action.surface)
        }
    }

    private func toggleSurface(id: String, sourceActivatorID: String? = nil) {
        if visibleSurfaceIDs.contains(id) {
            closeSurface(id: id)
        } else {
            openSurfaceHandlingError(id: id, sourceActivatorID: sourceActivatorID)
        }
    }

    func toggleManagedSurface(id: String) {
        toggleSurface(id: id)
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

        switch surface.presentation.style {
        case .window, .statusPanel:
            try openWindowSurface(surface, sourceActivatorID: sourceActivatorID)
        case .attachedPanel:
            try openAttachedPanelSurface(surface)
        case .sheet, .embedded:
            throw LxAppUIError.unsupported("surface \(surface.id) uses unsupported style \(surface.presentation.style.rawValue) on macOS")
        }
    }

    private func openWindowSurface(
        _ surface: LxAppUIConfig.Surface,
        sourceActivatorID: String? = nil
    ) throws {
        applyWindowPresentation(for: surface)
        if surface.presentation.style == .statusPanel {
            positionStatusPanelWindow(for: sourceActivatorID)
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

    private func openAttachedPanelSurface(_ surface: LxAppUIConfig.Surface) throws {
        if let parentID = surface.presentation.attachTo, !visibleSurfaceIDs.contains(parentID) {
            try openSurface(id: parentID)
        }

        if openedSurfaceIDs.contains(surface.id) {
            shell.show()
            shell.showPanel(id: surface.id)
            visibleSurfaceIDs.insert(surface.id)
            refreshChromeActivators()
            return
        }

        try requestAttachedPanelOpenThroughRuntime(surface)
    }

    private func requestAttachedPanelOpenThroughRuntime(_ surface: LxAppUIConfig.Surface) throws {
        guard surface.presentation.style == .attachedPanel else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) is not an attachedPanel")
        }
        guard case .lxapp = surface.content.kind,
              let appId = surface.content.appId,
              !appId.isEmpty else {
            throw LxAppUIError.invalidConfig("surface \(surface.id) requires content.appId for lxapp content")
        }

        openPanelLxapp(surface.id, appId, normalizedPath(surface.content.path))
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

    private func focusSurface(id: String) {
        guard visibleSurfaceIDs.contains(id),
              let surface = surfaceById[id] else { return }

        switch surface.presentation.style {
        case .window, .statusPanel:
            shell.show()
        case .attachedPanel:
            shell.show()
            shell.showPanel(id: id)
        case .sheet, .embedded:
            return
        }
    }

    private func closeSurface(id: String) {
        guard let surface = surfaceById[id] else { return }

        for childID in childrenByParentId[id] ?? [] {
            closeSurface(id: childID)
        }

        switch surface.presentation.style {
        case .window, .statusPanel:
            shell.hide()
            if !shell.hasOpenTabs {
                discardOpenedSubtree(rootID: id)
            }
        case .attachedPanel:
            shell.hidePanel(id: id)
        case .sheet, .embedded:
            break
        }

        visibleSurfaceIDs.remove(id)
        refreshChromeActivators()
    }

    private func discardOpenedSubtree(rootID: String) {
        openedSurfaceIDs.remove(rootID)
        for childID in childrenByParentId[rootID] ?? [] {
            discardOpenedSubtree(rootID: childID)
        }
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
        shell.setManagedNavigationToolbarVisible(!toolbarItems.isEmpty)
        shell.updateToolbarHostActions(toolbarItems)
        shell.updateTitlebarHostActions(titlebarItems)
    }

    private func installMenuBarActivators() {
        for activator in menuBarActivators {
            let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
            statusItems[activator.id] = statusItem
            if defaultMenuBarActivatorID == nil {
                defaultMenuBarActivatorID = activator.id
            }

            guard let button = statusItem.button else { continue }
            button.identifier = NSUserInterfaceItemIdentifier(activator.id)
            button.target = self
            button.action = #selector(statusItemClicked(_:))
            button.toolTip = activator.label ?? activator.id
            button.sendAction(on: [.leftMouseUp, .rightMouseUp, .otherMouseUp])

            if let iconURL = resolvedIconURL(for: activator),
               let image = NSImage(contentsOf: iconURL) {
                image.size = NSSize(width: 18, height: 18)
                image.isTemplate = iconURL.pathExtension.lowercased() == "pdf"
                button.image = image
                button.imagePosition = .imageOnly
            } else {
                os_log(
                    "AppUI menubar icon unavailable or unsuitable for activator=%{public}@ icon=%{public}@; using system fallback",
                    log: Self.log,
                    type: .info,
                    activator.id,
                    activator.icon ?? "nil"
                )
                if let fallbackImage = NSImage(systemSymbolName: "app.fill", accessibilityDescription: activator.label) {
                    fallbackImage.isTemplate = true
                    fallbackImage.size = NSSize(width: 16, height: 16)
                    button.image = fallbackImage
                    button.imagePosition = .imageOnly
                } else {
                    button.title = shortMenuBarTitle(for: activator)
                }
            }
        }
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

    private func performAppActivation() {
        guard !handlingAppActivation else { return }
        handlingAppActivation = true
        defer { handlingAppActivation = false }
        for activator in appActivationActivators {
            performActivator(id: activator.id)
        }
    }

    @objc private func statusItemClicked(_ sender: NSStatusBarButton) {
        guard let actionID = sender.identifier?.rawValue else { return }
        performActivator(id: actionID)
    }

    private func shortMenuBarTitle(for activator: LxAppUIConfig.Activator) -> String {
        if let label = activator.label, let first = label.first {
            return String(first)
        }
        if let first = appConfig.productName.first {
            return String(first)
        }
        return "L"
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
        let isResizable = surface.presentation.resizable ?? true
        let showTrafficLights = surface.presentation.showTrafficLights ?? (surface.presentation.style == .window)
        shell.applyManagedWindowPresentation(
            title: appConfig.productName,
            size: size,
            resizable: isResizable,
            style: surface.presentation.style,
            showTrafficLights: showTrafficLights
        )
    }

    private func positionStatusPanelWindow(for activatorID: String?) {
        guard let window = shell.window else { return }
        let resolvedActivatorID = activatorID ?? defaultMenuBarActivatorID
        guard let resolvedActivatorID,
              let button = statusItems[resolvedActivatorID]?.button,
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

    private func resolvedWindowSize(for surface: LxAppUIConfig.Surface) -> CGSize? {
        guard let size = surface.presentation.size,
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
        guard surface.presentation.style == .attachedPanel else { return nil }
        switch surface.presentation.edge {
        case .leading:
            return .left
        case .trailing:
            return .right
        case .bottom:
            return .bottom
        case .top, .none:
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

        var surfaceById: [String: LxAppUIConfig.Surface] = [:]
        var seenAppIDs = Set<String>()

        for surface in ui.surfaces {
            guard !surface.id.isEmpty else {
                throw LxAppUIError.invalidConfig("surface id cannot be empty")
            }
            guard surfaceById[surface.id] == nil else {
                throw LxAppUIError.invalidConfig("duplicate surface id \(surface.id)")
            }

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
                throw LxAppUIError.unsupported("surface \(surface.id) uses unsupported terminal content on macOS")
            }

            surfaceById[surface.id] = surface
        }

        guard let initialSurface = surfaceById[ui.launch.initialSurface] else {
            throw LxAppUIError.invalidConfig("launch.initialSurface references unknown surface \(ui.launch.initialSurface)")
        }
        guard initialSurface.presentation.style == .window
            || initialSurface.presentation.style == .statusPanel
            || initialSurface.presentation.style == .attachedPanel else {
            throw LxAppUIError.unsupported("launch.initialSurface must reference a supported macOS surface")
        }

        let windowSurfaces = ui.surfaces.filter {
            $0.presentation.style == .window || $0.presentation.style == .statusPanel
        }
        guard windowSurfaces.count == 1, let rootSurface = windowSurfaces.first else {
            throw LxAppUIError.unsupported("macOS app UI currently requires exactly one root window surface")
        }

        var childrenByParentId: [String: [String]] = [:]

        for surface in ui.surfaces {
            switch surface.presentation.style {
            case .window, .statusPanel:
                if surface.presentation.attachTo != nil {
                    throw LxAppUIError.invalidConfig("root window surface \(surface.id) cannot set attachTo")
                }
            case .attachedPanel:
                guard let parentID = surface.presentation.attachTo, !parentID.isEmpty else {
                    throw LxAppUIError.invalidConfig("attachedPanel surface \(surface.id) requires attachTo")
                }
                guard let parent = surfaceById[parentID] else {
                    throw LxAppUIError.invalidConfig("surface \(surface.id) attaches to unknown surface \(parentID)")
                }
                guard parent.presentation.style == .window || parent.presentation.style == .statusPanel else {
                    throw LxAppUIError.unsupported("macOS v1 does not support attachedPanel -> attachedPanel; surface \(surface.id) attaches to \(parentID)")
                }
                guard parent.id == rootSurface.id else {
                    throw LxAppUIError.unsupported("macOS v1 only supports panels attached to the root window surface")
                }
                guard surface.presentation.edge != .top else {
                    throw LxAppUIError.unsupported("macOS v1 does not support top-attached panels")
                }
                guard surface.presentation.edge != nil else {
                    throw LxAppUIError.invalidConfig("attachedPanel surface \(surface.id) requires edge")
                }
                childrenByParentId[parentID, default: []].append(surface.id)
            case .sheet, .embedded:
                throw LxAppUIError.unsupported("surface \(surface.id) uses unsupported style \(surface.presentation.style.rawValue) on macOS")
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
            guard seenActivatorIDs.insert(activator.id).inserted else {
                throw LxAppUIError.invalidConfig("duplicate activator id \(activator.id)")
            }
            guard surfaceById[activator.action.surface] != nil else {
                throw LxAppUIError.invalidConfig("activator \(activator.id) references unknown surface \(activator.action.surface)")
            }

            switch activator.kind {
            case .menuBarItem:
                if activator.hostSurface != nil {
                    throw LxAppUIError.invalidConfig("menuBarItem activator \(activator.id) cannot set hostSurface")
                }
                menuBarActivators.append(activator)
            case .trayItem:
                if activator.hostSurface != nil {
                    throw LxAppUIError.invalidConfig("trayItem activator \(activator.id) cannot set hostSurface")
                }
                continue
            case .appActivation:
                if activator.hostSurface != nil {
                    throw LxAppUIError.invalidConfig("appActivation activator \(activator.id) cannot set hostSurface")
                }
                appActivationActivators.append(activator)
            case .sidebarItem:
                guard let hostSurface = activator.hostSurface, surfaceById[hostSurface] != nil else {
                    throw LxAppUIError.invalidConfig("sidebarItem activator \(activator.id) requires a valid hostSurface")
                }
                sidebarActivators.append(activator)
            case .toolbarItem:
                guard let hostSurface = activator.hostSurface, surfaceById[hostSurface] != nil else {
                    throw LxAppUIError.invalidConfig("toolbarItem activator \(activator.id) requires a valid hostSurface")
                }
                toolbarActivators.append(activator)
            case .titlebarItem:
                guard let hostSurface = activator.hostSurface, surfaceById[hostSurface] != nil else {
                    throw LxAppUIError.invalidConfig("titlebarItem activator \(activator.id) requires a valid hostSurface")
                }
                titlebarActivators.append(activator)
            case .deepLink:
                if activator.hostSurface != nil {
                    throw LxAppUIError.invalidConfig("deepLink activator \(activator.id) cannot set hostSurface")
                }
                continue
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
