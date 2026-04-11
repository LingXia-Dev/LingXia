import AppKit
import os.log
@_spi(Runner) import lingxia

/// LingXia Runner - Development tool with Simulator mode
/// Provides Xcode-like simulator interface for testing LxApps
@MainActor
public class RunnerApp {
    public static let shared = RunnerApp()
    private static let log = OSLog(subsystem: "LingXiaRunner", category: "RunnerApp")
    
    private var windowController: CapsuleWindowController?
    private var controller: LxAppController?
    private var controllerEventsTask: Task<Void, Never>?
    private(set) var deviceSize: MobileDeviceSize = .iPhoneSE
    
    private init() {}
    
    // MARK: - Configuration
    
    /// Set device size for the Runner window
    /// This can be called to change device while running
    public func setDeviceSize(_ size: MobileDeviceSize) {
        self.deviceSize = size
        CapsuleWindowController.setWindowSize(size)
        os_log("Device size changed to: %@", log: Self.log, type: .info, size.displayName)
    }

    public func bind(controller: LxAppController) {
        self.controller = controller
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
    
    // MARK: - LxApp Management
    
    /// Open LxApp in Capsule window
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

        // Check if window already exists
        if let existingController = windowController, existingController.appId == appId {
            RunnerSupport.Runtime.setSessionId(sessionId, for: appId)
            RunnerSupport.Runtime.setCurrentApp(appId: appId, path: resolvedPath)
            existingController.window?.makeKeyAndOrderFront(nil)
            existingController.navigate(to: resolvedPath)
            return
        }

        RunnerSupport.Runtime.setSessionId(sessionId, for: appId)
        RunnerSupport.Runtime.setCurrentApp(appId: appId, path: resolvedPath)
        
        // Create new window controller
        let controller = CapsuleWindowController(appId: appId, path: resolvedPath)
        controller.showWindow(self)
        controller.window?.makeKeyAndOrderFront(self)
        NSApp.activate(ignoringOtherApps: true)
        
        windowController = controller
    }

    private func resolveOpenPath(appId: String, requestedPath: String, sessionId: UInt64) -> String {
        if !requestedPath.isEmpty {
            return requestedPath
        }
        return onLxappOpened(appId, "", sessionId).toString()
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
        windowController?.navigate(to: path)
    }
    
    /// Handle navigation with animation type (called from SDK handler)
    public func handleNavigation(appId: String, path: String, animationType: LxAppAnimation) {
        if windowController?.appId == appId {
            windowController?.navigate(to: path, animationType: animationType)
        }
    }
    
    /// Close current LxApp
    public func closeLxApp() {
        windowController?.window?.close()
        windowController = nil
    }

    func handleWindowClosed(_ controller: CapsuleWindowController) {
        if windowController === controller {
            windowController = nil
        }
    }

    func discardSession(appId: String, sessionId: UInt64) {
        _ = controller?.discardSession(appId: appId, sessionId: sessionId)
    }
}
