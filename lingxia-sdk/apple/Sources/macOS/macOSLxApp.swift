#if os(macOS)
import Foundation
import Cocoa
import os.log
import CLingXiaFFI

public enum LxAppWindowStyle {
    case customCapsule
    case tabStyle
}

// Directory provider is now in shared LxAppDirectoryProvider.swift

@MainActor
public class macOSLxApp {
    private static let log = OSLog(subsystem: "LingXia", category: "macOSLxApp")

    private static var activeWindowControllers: [macOSWindowController] = []
    private static var tabWindowController: macOSWindowController?
    private static var isInitialized = false

    /// Set window size for all LxApp windows using physical dimensions
    /// - Parameters:
    ///   - widthCm: Window width in centimeters
    ///   - heightCm: Window height in centimeters
    public static func setWindowSize(widthCm: CGFloat, heightCm: CGFloat) {
        let widthInches = widthCm / 2.54
        let heightInches = heightCm / 2.54

        guard let screen = NSScreen.main else {
            let defaultDPI: CGFloat = 72.0
            macOSWindowController.setWindowSize(width: widthInches * defaultDPI, height: heightInches * defaultDPI)
            return
        }

        let dpi = screen.deviceDescription[NSDeviceDescriptionKey.resolution] as! NSSize
        let widthPoints = widthInches * dpi.width
        let heightPoints = heightInches * dpi.height

        macOSWindowController.setWindowSize(width: widthPoints, height: heightPoints)
    }

    /// Set window style for all LxApp windows
    /// - Parameter style: Window style to use
    public static func setWindowStyle(_ style: LxAppWindowStyle) {
        macOSWindowController.setWindowStyle(style)
    }

    /// Open home LxApp
    public static func openHomeLxApp() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            os_log("Home LxApp not configured", log: log, type: .error)
            return
        }

        let initialRoute = LxAppCore.getHomeLxAppInitialRoute()

        // Check if using tab style
        if macOSWindowController.getWindowStyle() == .tabStyle {
            openTabStyleWindow()
        } else {
            openLxApp(appId: homeLxAppId, path: initialRoute)
        }
    }

    /// Open specific LxApp
    public static func openLxApp(appId: String, path: String) {
        // Check if using tab style
        if macOSWindowController.getWindowStyle() == .tabStyle {
            if let tabController = tabWindowController {
                tabController.openLxApp(appId: appId, path: path)
                tabController.window?.makeKeyAndOrderFront(nil)
            } else {
                openTabStyleWindow()
                tabWindowController?.openLxApp(appId: appId, path: path)
            }
            return
        }

        // Get app info and cache initial route for navigation logic
        let lxappInfo = getLxAppInfo(appId)
        let initialRoute = lxappInfo.initial_route.toString()
        PageNavigationCore.cacheInitialRoute(appId: appId, initialRoute: initialRoute)

        // Check if window already exists for this app
        if let existingController = activeWindowControllers.first(where: { $0.appId == appId }) {
            let _ = onLxappOpened(appId, path)
            existingController.window?.makeKeyAndOrderFront(nil as Any?)
            switchPage(appId: appId, path: path)
            return
        }

        let storedPath = LxAppCore.getLastActivePath(for: appId)
        let actualPath = (!storedPath.isEmpty && storedPath != path && appId != LxAppCore.getHomeLxAppId()) ? storedPath : path

        let _ = onLxappOpened(appId, actualPath)

        let windowController = macOSWindowController(appId: appId, path: actualPath)
        windowController.showWindow(nil as Any?)
        windowController.reapplyWindowSize()
        windowController.window?.makeKeyAndOrderFront(nil as Any?)

        NSApp.activate(ignoringOtherApps: true)
        activeWindowControllers.append(windowController)
    }

    /// Close LxApp (String version for convenience)
    public static func closeLxApp(appId: String) {
        if let controller = activeWindowControllers.first(where: { $0.appId == appId }) {
            controller.window?.close()
        }
    }

    internal static func handleAppClosing(appId: String) {
        let _ = onLxappClosed(appId)
    }

    /// Switch to page in LxApp (String version for convenience)
    public static func switchPage(appId: String, path: String) {
        if let controller = activeWindowControllers.first(where: { $0.appId == appId }),
           let viewController = controller.window?.contentViewController as? macOSLxAppViewController {
            viewController.switchPage(targetPath: path)

            NotificationCenter.default.post(
                name: NSNotification.Name(ACTION_SWITCH_PAGE),
                object: nil,
                userInfo: ["appId": appId, "path": path]
            )
        }
    }

    internal static func removeWindowController(_ controller: macOSWindowController) {
        activeWindowControllers.removeAll { $0 === controller }
    }

    internal static func removeTabWindowController(_ controller: macOSWindowController) {
        if tabWindowController === controller {
            tabWindowController = nil
        }
    }

    /// Open tab-style window
    private static func openTabStyleWindow() {
        if tabWindowController == nil {
            tabWindowController = macOSWindowController()
            tabWindowController?.showWindow(nil)
            NSApp.activate(ignoringOtherApps: true)
        } else {
            tabWindowController?.window?.makeKeyAndOrderFront(nil)
        }
    }

    /// Get active window controllers
    internal static func getActiveWindowControllers() -> [macOSWindowController] {
        return activeWindowControllers
    }

    /// Initialize LxApps system
    /// - Returns: true if initialization successful, false otherwise
    public static func initialize() -> Bool {
        // Check if already initialized
        if isInitialized {
            return true
        }

        // Set platform directory provider
        LxAppCore.setPlatformDirectoryProvider(macOSDirectoryProvider.self)

        // Use LxAppCore.initialize() instead of duplicating the logic
        LxAppCore.initialize()

        // Check if initialization was successful
        if LxAppCore.getHomeLxAppId() != nil {
            isInitialized = true
            return true
        } else {
            os_log("Failed to initialize LxApps - no home app ID", log: log, type: .error)
            return false
        }
    }
}

#endif
