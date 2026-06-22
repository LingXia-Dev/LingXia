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
    private var surfaceShellHost: RunnerSurfaceShellHost?
    private var controller: LxAppController?
    private var controllerEventsTask: Task<Void, Never>?
    private let deviceMenu = RunnerDeviceMenu()
    private(set) var deviceSize: MobileDeviceSize = .defaultDevice
    
    private init() {}
    
    // MARK: - Configuration
    
    /// Set device size for the Runner window
    /// This can be called to change device while running
    public func setDeviceSize(_ size: MobileDeviceSize) {
        self.deviceSize = size
        SimulatorWindowController.setWindowSize(size)
        configureOpenURLHandlingForCurrentShape()
        deviceMenu.updateSelectedDevice(size)
        os_log("Device size changed to: %@", log: Self.log, type: .info, size.displayName)

        if size.usesSurfaceShell {
            switchToSurfaceShellHost(device: size)
        } else {
            switchToPhoneSimulatorHost(device: size)
        }
    }

    public func bind(controller: LxAppController) {
        self.controller = controller
        deviceMenu.installIfNeeded()
        deviceMenu.updateSelectedDevice(deviceSize)
        configureOpenURLHandlingForCurrentShape()
        controllerEventsTask?.cancel()
        controllerEventsTask = Task { [weak self, controller] in
            for await event in controller.events {
                guard let self else { return }
                switch event {
                case .didNavigate(let sessionId, let path):
                    guard let session = controller.sessions[sessionId] else { continue }
                    self.handleNavigation(
                        appId: session.appId,
                        path: path,
                        animationType: .none
                    )
                case .didClose(let session):
                    RunnerSupport.Runtime.removeSessionId(for: session.appId)
                    if self.windowController?.appId == session.appId {
                        self.windowController?.closeFromRuntime()
                    }
                default:
                    continue
                }
            }
        }
    }

    private func configureOpenURLHandlingForCurrentShape() {
        installRunnerOpenURLHandler()
    }

    private func installRunnerOpenURLHandler() {
        os_log("Installing Runner openURL self handler", log: Self.log, type: .info)
        RunnerSupport.Runtime.setOpenUrlHandler { [weak self] ownerAppId, ownerSessionId, url in
            self?.handleOpenURL(
                ownerAppId: ownerAppId,
                ownerSessionId: ownerSessionId,
                url: url
            ) ?? false
        }
    }
    
    // MARK: - LxApp Management
    
    /// Open LxApp in Simulator window
    public func openLxApp(appId: String, path: String = "") {
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
            existingController.navigate(to: path)
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
        url rawURL: String
    ) -> Bool {
        let phoneHost = windowController?.appId == ownerAppId ? windowController : nil
        let surfaceHost = surfaceShellHost?.appId == ownerAppId ? surfaceShellHost : nil
        guard phoneHost != nil || surfaceHost != nil else {
            os_log("Runner rejected self openURL for non-active appId=%@", log: Self.log, type: .info, ownerAppId)
            return false
        }
        guard RunnerSupport.Runtime.sessionId(for: ownerAppId) == ownerSessionId else {
            os_log("Runner rejected self openURL for stale session appId=%@ session=%{public}llu", log: Self.log, type: .info, ownerAppId, ownerSessionId)
            return false
        }
        guard let tabId = RunnerSupport.Browser.openTab(
            ownerAppId: ownerAppId,
            ownerSessionId: ownerSessionId,
            url: rawURL
        ) else {
            os_log("Runner failed to open browser tab for appId=%@ url=%@", log: Self.log, type: .error, ownerAppId, rawURL)
            return false
        }

        os_log("Runner presenting browser tab appId=%@ tab=%@ url=%@", log: Self.log, type: .info, ownerAppId, tabId, rawURL)
        if let phoneHost {
            phoneHost.presentBrowserTab(id: tabId)
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
            return (windowController.appId, windowController.currentPath, sessionId)
        }

        if let surfaceShellHost {
            surfaceShellHost.refreshCurrentPathFromRuntime()
            if let sessionId = RunnerSupport.Runtime.sessionId(for: surfaceShellHost.appId),
               sessionId > 0 {
                return (surfaceShellHost.appId, surfaceShellHost.currentPath, sessionId)
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
