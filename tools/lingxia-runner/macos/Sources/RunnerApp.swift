import AppKit
import os.log
@_spi(Runner) import lingxia

/// LingXia Runner - Development tool with Simulator mode
/// Provides Xcode-like simulator interface for testing LxApps
@MainActor
public class RunnerApp {
    public static let shared = RunnerApp()
    private static let log = OSLog(subsystem: "LingXiaRunner", category: "RunnerApp")
    
    private var windowController: SimulatorWindowController?
    private var webTargetURL: URL?
    private var surfaceShellHost: RunnerSurfaceShellHost?
    private var controller: LxAppController?
    private var controllerEventsTask: Task<Void, Never>?
    private let appMenu = RunnerAppMenu()
    private var pendingPhoneLifecycleReopens: [String: (appId: String, path: String, sessionId: UInt64)] = [:]
    /// App ids whose next phone reopen should reload the webview in place (restart),
    /// keeping the device-frame window visible instead of rebuilding it.
    private var phoneRestartReloadPending: Set<String> = []
    /// The lxapp stack behind the current phone window: opening another lxapp
    /// suspends the current window (hidden, session and page stack intact);
    /// closing the one on top reveals it again.
    private var suspendedPhoneControllers: [SimulatorWindowController] = []
    /// App ids in a surface-shell (pad/desktop) restart — the runner stands down on
    /// `.didClose` and lets the runtime's recreate re-attach the fresh session.
    private var pendingSurfaceLifecycleReopens: Set<String> = []
    private(set) var selectedDeviceSize: MobileDeviceSize = .defaultDevice
    private(set) var deviceOrientation: RunnerDeviceOrientation = .portrait
    private(set) var deviceSize: MobileDeviceSize = .defaultDevice
    
    private init() {
        deviceOrientation = Self.defaultOrientation(for: selectedDeviceSize)
    }

    /// Pads and desktops read most naturally in landscape; phones in portrait.
    private static func defaultOrientation(for device: MobileDeviceSize) -> RunnerDeviceOrientation {
        guard device.supportsOrientation else { return .portrait }
        return (device.shape == .pad || device.shape == .desktop) ? .landscape : .portrait
    }
    
    // MARK: - Configuration
    
    /// Set device size for the Runner window
    /// This can be called to change device while running
    public func setDeviceSize(_ size: MobileDeviceSize) {
        setDeviceSize(size, orientation: nil)
    }

    /// Apply a device and an optional explicit orientation as one host update.
    /// A nil orientation preserves the selector's existing family behavior.
    func setDeviceSize(
        _ size: MobileDeviceSize,
        orientation: RunnerDeviceOrientation?
    ) {
        let shapeChanged = size.shape != selectedDeviceSize.shape
        selectedDeviceSize = size
        if let orientation, size.supportsOrientation {
            deviceOrientation = orientation
        } else if shapeChanged || !size.supportsOrientation {
            // Reset to the shape's natural orientation on a device-family
            // change; keep the user's rotation within the same family.
            deviceOrientation = Self.defaultOrientation(for: size)
        }
        applyDeviceConfiguration()
    }

    public func setDeviceOrientation(_ orientation: RunnerDeviceOrientation) {
        guard selectedDeviceSize.supportsOrientation else { return }
        deviceOrientation = orientation
        applyDeviceConfiguration()
    }

    public func toggleDeviceOrientation() {
        setDeviceOrientation(deviceOrientation.toggled)
    }

    private func applyDeviceConfiguration() {
        let effectiveDevice = selectedDeviceSize.oriented(deviceOrientation)
        deviceSize = effectiveDevice
        SimulatorWindowController.setWindowSize(effectiveDevice)
        configureOpenURLHandlingForCurrentShape()
        os_log(
            "Device size changed to: %@ %@",
            log: Self.log,
            type: .info,
            selectedDeviceSize.displayName,
            effectiveDevice.supportsOrientation ? deviceOrientation.displayName : ""
        )

        if webTargetURL != nil {
            windowController?.applyDeviceChange(effectiveDevice)
        } else if effectiveDevice.usesSurfaceShell {
            switchToSurfaceShellHost(device: effectiveDevice)
        } else {
            switchToPhoneSimulatorHost(device: effectiveDevice)
        }
    }

    public func bind(controller: LxAppController) {
        self.controller = controller
        appMenu.installIfNeeded()
        configureOpenURLHandlingForCurrentShape()
        controllerEventsTask?.cancel()
        controllerEventsTask = Task { [weak self, controller] in
            for await event in controller.events {
                guard let self else { return }
                switch event {
                case .didNavigate(let sessionId, let path, let animation):
                    guard let session = controller.sessions[sessionId] else { continue }
                    self.handleNavigation(
                        appId: session.appId,
                        path: path,
                        animationType: animation
                    )
                case .didClose(let session):
                    RunnerSupport.Runtime.removeSessionId(for: session.appId)
                    // Keep the shared shell tab list in sync regardless of the
                    // current device shape — a stale tab would resurface in
                    // the sidebar after a later switch to desktop.
                    let shellActiveAppId = RunnerSupport.Runtime.removeShellTab(for: session.appId)
                    if let phoneHost = self.windowController, phoneHost.appId == session.appId {
                        if let pending = self.pendingPhoneLifecycleReopens.removeValue(forKey: session.appId) {
                            // Restart: reload the webview in place so the device frame
                            // never disappears.
                            self.phoneRestartReloadPending.insert(session.appId)
                            self.reopenCurrentAppAfterLifecycleAction(pending)
                        } else if let previous = self.suspendedPhoneControllers.popLast() {
                            // A closed lxapp reveals the one it covered — same
                            // window object, so its page stack is untouched.
                            phoneHost.closeFromRuntime()
                            self.confirmRuntimeClose(session)
                            self.windowController = previous
                            previous.window?.makeKeyAndOrderFront(nil)
                            self.alignPhoneHostWithRuntime(previous)
                        } else {
                            phoneHost.closeFromRuntime()
                            self.confirmRuntimeClose(session)
                            self.revealRuntimeCurrentOrHome(
                                closedAppId: session.appId, controller: controller)
                        }
                    } else if self.surfaceShellHost?.appId == session.appId {
                        if self.pendingSurfaceLifecycleReopens.remove(session.appId) != nil {
                            // Restart: the runtime's recreate re-attaches the fresh
                            // session through the shell; the runner stands down.
                        } else if let next = shellActiveAppId {
                            // The shell already switched to the surviving tab
                            // (its page state intact) — just realign the host
                            // record; reopening home would clobber that page.
                            self.confirmRuntimeClose(session)
                            self.surfaceShellHost?.noteActiveLxApp(appId: next)
                        } else {
                            self.confirmRuntimeClose(session)
                            self.reopenHomeAfterRuntimeClose(appId: session.appId, controller: controller)
                        }
                    }
                default:
                    continue
                }
            }
        }
    }

    /// A runtime-initiated close (e.g. capsule) only hides the lxapp and waits
    /// for the host to confirm the UI is gone. The phone host suppresses the
    /// window-close notification, so without this echo the closed lxapp stays
    /// on the runtime navigation stack with an empty page stack — and every
    /// later reveal or shape switch reads that stale state (home page shown,
    /// tabbar items missing). Synchronous, so runtime queries right after
    /// already see the popped stack.
    private func confirmRuntimeClose(_ session: LxAppSession) {
        _ = onLxappClosed(session.appId, session.id.rawValue)
    }

    /// Show whatever the runtime navigation stack now has on top, at its real
    /// current page; fall back to home only when the stack is empty (this is
    /// how a close after a desktop→phone switch finds the lxapp that was
    /// never suspended into a phone window).
    private func revealRuntimeCurrentOrHome(closedAppId: String, controller: LxAppController) {
        let current = getCurrentLxApp()
        let appId = current.appid.toString()
        if !appId.isEmpty, appId != closedAppId, current.session_id > 0 {
            openInPhoneSimulator(
                appId: appId,
                path: current.path.toString(),
                sessionId: current.session_id
            )
        } else {
            reopenHomeAfterRuntimeClose(appId: closedAppId, controller: controller)
        }
    }

    /// The runtime's page for `appId` when it is the stack-top lxapp; the
    /// host caches miss navigations they didn't host (tab switches, pages
    /// opened while the app lived in the other shape).
    private func runtimePath(for appId: String, fallback: String) -> String {
        let current = getCurrentLxApp()
        let path = current.path.toString()
        return (current.appid.toString() == appId && !path.isEmpty) ? path : fallback
    }

    /// Realign a revealed phone host with the runtime's authoritative page —
    /// the host cache misses navigations that happened while it was covered,
    /// and re-navigating also re-attaches the webview if the closed lxapp
    /// had taken over the attachment.
    private func alignPhoneHostWithRuntime(_ host: SimulatorWindowController) {
        let path = runtimePath(for: host.appId, fallback: host.currentPath)
        host.navigate(to: path)
        RunnerSupport.Runtime.setCurrentApp(appId: host.appId, path: path)
    }

    private func reopenHomeAfterRuntimeClose(appId: String, controller: LxAppController) {
        os_log("Runner reopening home after runtime close appId=%@", log: Self.log, type: .info, appId)
        Task { @MainActor [weak self, controller] in
            guard self != nil else { return }
            _ = try? await controller.openHomeApp()
        }
    }

    private func configureOpenURLHandlingForCurrentShape() {
        installRunnerOpenURLHandler()
    }

    private func installRunnerOpenURLHandler() {
        os_log("Installing Runner openURL self handler", log: Self.log, type: .info)
        RunnerSupport.Runtime.setOpenUrlHandler { [weak self] ownerAppId, ownerSessionId, url, aside in
            self?.handleOpenURL(
                ownerAppId: ownerAppId,
                ownerSessionId: ownerSessionId,
                url: url,
                aside: aside
            ) ?? false
        }
    }
    
    // MARK: - LxApp Management

    /// Open an ordinary web development target without creating an lxapp.
    public func openWeb(url: URL) {
        webTargetURL = url
        surfaceShellHost?.shell.window?.close()
        surfaceShellHost = nil
        windowController?.window?.close()
        let host = SimulatorWindowController(webURL: url)
        host.showWindow(self)
        host.window?.makeKeyAndOrderFront(self)
        NSApp.activate(ignoringOtherApps: true)
        windowController = host
    }
    
    /// Open LxApp in Simulator window
    public func openLxApp(appId: String, path: String = "") {
        webTargetURL = nil
        os_log("Runner openLxApp: %@ at path: %@", log: Self.log, type: .info, appId, path)

        let sessionId = RunnerSupport.Runtime.sessionId(for: appId) ?? getLxAppSessionId(appId)
        guard sessionId > 0 else {
            os_log("Missing session for %@", log: Self.log, type: .error, appId)
            return
        }

        let resolvedPath = resolveOpenPath(appId: appId, requestedPath: path, sessionId: sessionId)
        guard !resolvedPath.isEmpty else {
            os_log("Runner openLxApp rejected by Rust for %@", log: Self.log, type: .info, appId)
            return
        }

        RunnerSupport.Runtime.setSessionId(sessionId, for: appId)
        RunnerSupport.Runtime.setCurrentApp(appId: appId, path: resolvedPath)
        if pendingPhoneLifecycleReopens[appId]?.sessionId != sessionId {
            pendingPhoneLifecycleReopens.removeValue(forKey: appId)
        }

        if deviceSize.usesSurfaceShell {
            openInSurfaceShell(appId: appId, path: resolvedPath, sessionId: sessionId)
        } else {
            openInPhoneSimulator(appId: appId, path: resolvedPath, sessionId: sessionId)
        }
    }

    private func openInPhoneSimulator(appId: String, path: String, sessionId: UInt64) {
        surfaceShellHost?.hideForHostSwitch()
        if let controller {
            Lingxia.activate(controller: controller)
        }
        installRunnerOpenURLHandler()
        RunnerSupport.Runtime.setSessionId(sessionId, for: appId)
        RunnerSupport.Runtime.setCurrentApp(appId: appId, path: path)

        if let existingController = windowController, existingController.appId == appId {
            existingController.applyDeviceChange(deviceSize)
            existingController.window?.makeKeyAndOrderFront(nil)
            if phoneRestartReloadPending.remove(appId) != nil {
                // Fresh session after a restart: swap the inner webview in place.
                existingController.reloadContentForRestart(path: path)
            } else {
                existingController.navigate(to: path)
            }
            return
        }

        // Suspend the current phone window instead of discarding it — its
        // session and page stack stay intact for when the new lxapp closes.
        if let current = windowController {
            current.window?.orderOut(nil)
            suspendedPhoneControllers.append(current)
        }
        // Reopening an lxapp that is already suspended resumes its window.
        if let index = suspendedPhoneControllers.lastIndex(where: { $0.appId == appId }) {
            let resumed = suspendedPhoneControllers.remove(at: index)
            resumed.applyDeviceChange(deviceSize)
            resumed.window?.makeKeyAndOrderFront(nil)
            resumed.navigate(to: path)
            windowController = resumed
            return
        }

        let controller = SimulatorWindowController(appId: appId, path: path)
        controller.showWindow(self)
        controller.window?.makeKeyAndOrderFront(self)
        NSApp.activate(ignoringOtherApps: true)

        windowController = controller
    }

    private func openInSurfaceShell(appId: String, path: String, sessionId: UInt64) {
        guard let controller else {
            os_log("Runner controller not configured", log: Self.log, type: .error)
            return
        }

        windowController?.detachForHostSwitch()
        windowController = nil

        if let host = surfaceShellHost {
            host.applyDevice(deviceSize)
            host.open(appId: appId, path: path, sessionId: sessionId)
            return
        }

        let host = RunnerSurfaceShellHost(
            controller: controller,
            appId: appId,
            path: path,
            sessionId: sessionId,
            device: deviceSize
        )
        host.onClose = { [weak self] closedHost in
            guard self?.surfaceShellHost === closedHost else { return }
            self?.surfaceShellHost = nil
        }
        surfaceShellHost = host
    }

    private func resolveOpenPath(appId: String, requestedPath: String, sessionId: UInt64) -> String {
        let created = createPageInstance(appId, requestedPath, sessionId, 0, "")
        guard created.ok else {
            os_log(
                "Runner createPageInstance rejected appId=%@ session=%{public}llu error=%@",
                log: Self.log,
                type: .info,
                appId,
                sessionId,
                created.error.toString()
            )
            return ""
        }
        return created.resolved_path.toString()
    }
    
    /// Open home LxApp
    public func openHomeLxApp() async {
        guard let controller else {
            os_log("Runner controller not configured", log: Self.log, type: .error)
            return
        }
        _ = try? await controller.openHomeApp()
    }
    
    /// Navigate to path in current LxApp
    public func navigate(to path: String) {
        handleNavigation(
            appId: windowController?.appId ?? surfaceShellHost?.appId ?? "",
            path: path,
            animationType: .none
        )
    }

    private func handleOpenURL(
        ownerAppId: String,
        ownerSessionId: UInt64,
        url rawURL: String,
        aside: Bool
    ) -> Bool {
        var phoneHost = windowController?.appId == ownerAppId ? windowController : nil
        var surfaceHost = surfaceShellHost?.appId == ownerAppId ? surfaceShellHost : nil
        // In-page new-tab requests (`target="_blank"` / `window.open`) are owned
        // by the builtin browser app, not the lxapp, so they don't match either
        // host's appId. Route them to whichever host is presenting the browser.
        let isBuiltinBrowserTab = ownerAppId == RunnerSupport.Browser.builtinAppId
        if phoneHost == nil, surfaceHost == nil, isBuiltinBrowserTab {
            if windowController?.isPresentingBrowser == true {
                phoneHost = windowController
            } else if let host = surfaceShellHost {
                surfaceHost = host
            }
        }
        guard phoneHost != nil || surfaceHost != nil else {
            os_log("Runner rejected self openURL for non-active appId=%@", log: Self.log, type: .info, ownerAppId)
            return false
        }
        // The builtin browser's session isn't tracked in the lxapp session map,
        // and a new-tab request from a live browser tab is inherently current —
        // so only staleness-check lxapp-owned (`target="self"`) opens.
        if !isBuiltinBrowserTab {
            guard RunnerSupport.Runtime.sessionId(for: ownerAppId) == ownerSessionId else {
                os_log("Runner rejected self openURL for stale session appId=%@ session=%{public}llu", log: Self.log, type: .info, ownerAppId, ownerSessionId)
                return false
            }
        }
        guard let tabId = RunnerSupport.Browser.openTab(
            ownerAppId: ownerAppId,
            ownerSessionId: ownerSessionId,
            url: rawURL,
            aside: aside
        ) else {
            os_log("Runner failed to open browser tab for appId=%@ url=%@", log: Self.log, type: .error, ownerAppId, rawURL)
            return false
        }

        os_log("Runner presenting browser tab appId=%@ tab=%@ url=%@", log: Self.log, type: .info, ownerAppId, tabId, rawURL)
        if let phoneHost {
            phoneHost.presentBrowserTab(id: tabId, ownerAppId: ownerAppId, ownerSessionId: ownerSessionId)
        } else {
            surfaceHost?.presentBrowserTab(id: tabId)
        }
        return true
    }
    
    /// Handle navigation with animation type (called from SDK handler)
    public func handleNavigation(appId: String, path: String, animationType: LxAppAnimation) {
        if windowController?.appId == appId {
            windowController?.navigate(to: path, animationType: animationType)
        } else if let host = surfaceShellHost, host.appId == appId {
            host.navigate(to: path, animationType: animationType)
        }
    }
    
    /// Close current LxApp
    public func closeLxApp() {
        windowController?.window?.close()
        windowController = nil
        surfaceShellHost?.shell.window?.close()
        surfaceShellHost = nil
    }

    public func restartCurrentLxApp() {
        triggerCurrentLxAppAction("restart", reopenAfterAction: true)
    }

    public func cleanCacheAndRestartCurrentLxApp() {
        triggerCurrentLxAppAction("clean_cache_restart", reopenAfterAction: true)
    }

    public func closeCurrentLxAppFromCapsule() {
        // The close circle closes the current lxapp — never the runner app
        // (quitting stays on the red dot). A covered lxapp is revealed by the
        // didClose handler; the home lxapp's close clears its stack to home.
        triggerCurrentLxAppAction("close", reopenAfterAction: false)
    }

    private func triggerCurrentLxAppAction(_ action: String, reopenAfterAction: Bool) {
        guard let current = currentOpenApp() else {
            os_log("Runner ignored lxapp action without current app: %@", log: Self.log, type: .info, action)
            return
        }

        let shouldReopenPhoneHost = reopenAfterAction && windowController?.appId == current.appId
        if shouldReopenPhoneHost {
            pendingPhoneLifecycleReopens[current.appId] = current
        }

        let handled = onLxappEvent(current.appId, LxAppUiEventType.CapsuleClick, action)
        os_log(
            "Runner lxapp action=%@ appId=%@ handled=%{public}@",
            log: Self.log,
            type: .info,
            action,
            current.appId,
            handled ? "true" : "false"
        )

        guard handled else {
            // The runtime rejected the action: drop any pending markers so a later
            // real close isn't mistaken for this (never-happened) restart.
            pendingPhoneLifecycleReopens.removeValue(forKey: current.appId)
            return
        }

        if reopenAfterAction && !shouldReopenPhoneHost {
            // Surface-shell restart: stand down (the runtime's recreate re-attaches);
            // mark it so `.didClose` stands down too.
            pendingSurfaceLifecycleReopens.insert(current.appId)
        }
    }

    private func reopenCurrentAppAfterLifecycleAction(
        _ current: (appId: String, path: String, sessionId: UInt64)
    ) {
        Task { @MainActor [weak self] in
            guard let self else { return }
            let deadline = Date().addingTimeInterval(3.0)
            while Date() < deadline {
                if let activeSession = RunnerSupport.Runtime.sessionId(for: current.appId),
                   activeSession > 0,
                   activeSession != current.sessionId,
                   (self.windowController?.appId == current.appId || self.surfaceShellHost?.appId == current.appId),
                   !self.phoneRestartReloadPending.contains(current.appId) {
                    return
                }

                let sessionId = getLxAppSessionId(current.appId)
                if sessionId > 0, sessionId != current.sessionId {
                    self.openLxApp(appId: current.appId, path: "")
                    return
                }

                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            self.openLxApp(appId: current.appId, path: "")
        }
    }

    public func restartRunner() {
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = true
        configuration.createsNewApplicationInstance = true
        configuration.environment = ProcessInfo.processInfo.environment
        let log = Self.log

        NSWorkspace.shared.openApplication(
            at: Bundle.main.bundleURL,
            configuration: configuration
        ) { _, error in
            Task { @MainActor in
                if let error {
                    os_log("Failed to restart LingXia Runner: %@", log: log, type: .error, error.localizedDescription)
                    return
                }
                NSApp.terminate(nil)
            }
        }
    }

    func handleWindowClosed(_ controller: SimulatorWindowController) {
        if windowController === controller {
            windowController = nil
        }
    }

    func discardSession(appId: String, sessionId: UInt64) {
        _ = controller?.discardSession(appId: appId, sessionId: sessionId)
    }

    private func switchToSurfaceShellHost(device: MobileDeviceSize) {
        guard let current = currentOpenApp() else {
            surfaceShellHost?.applyDevice(device)
            return
        }
        // Bring the whole phone lxapp stack over, bottom first — the shell's
        // sidebar switcher lists every open lxapp, with current active last.
        let suspended = suspendedPhoneControllers
        suspendedPhoneControllers.removeAll()
        for controller in suspended {
            let appId = controller.appId
            let path = controller.currentPath
            let sessionId = RunnerSupport.Runtime.sessionId(for: appId) ?? 0
            controller.detachForHostSwitch()
            if sessionId > 0 {
                openInSurfaceShell(appId: appId, path: path, sessionId: sessionId)
            }
        }
        openInSurfaceShell(
            appId: current.appId,
            path: current.path,
            sessionId: current.sessionId
        )
    }

    private func switchToPhoneSimulatorHost(device: MobileDeviceSize) {
        guard let current = currentOpenApp() else {
            windowController?.applyDeviceChange(device)
            if let controller {
                Lingxia.activate(controller: controller)
            }
            installRunnerOpenURLHandler()
            return
        }

        surfaceShellHost?.hideForHostSwitch()
        openInPhoneSimulator(
            appId: current.appId,
            path: current.path,
            sessionId: current.sessionId
        )
    }

    private func currentOpenApp() -> (appId: String, path: String, sessionId: UInt64)? {
        if let windowController,
           let sessionId = RunnerSupport.Runtime.sessionId(for: windowController.appId),
           sessionId > 0 {
            return (
                windowController.appId,
                runtimePath(for: windowController.appId, fallback: windowController.currentPath),
                sessionId
            )
        }

        if surfaceShellHost != nil {
            // The shell closes/switches tabs on its own (sidebar ×, chrome),
            // without a controller didClose — the host's cached appId can point
            // at a closed lxapp. The runtime nav stack is the authority.
            let current = getCurrentLxApp()
            let appId = current.appid.toString()
            if !appId.isEmpty, current.session_id > 0 {
                return (appId, current.path.toString(), current.session_id)
            }
        }

        if let appId = RunnerSupport.Runtime.currentAppId(),
           let sessionId = RunnerSupport.Runtime.sessionId(for: appId),
           sessionId > 0 {
            return (appId, RunnerSupport.Runtime.currentPath(), sessionId)
        }

        return nil
    }
}
