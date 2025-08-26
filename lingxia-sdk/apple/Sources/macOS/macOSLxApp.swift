import SwiftUI
import os.log
import CLingXiaFFI

#if os(macOS)
import AppKit

/// Window style options for LxApp on macOS
public enum LxAppWindowStyle {
    case capsuleStyle
    case tabStyle
}

/// Predefined mobile device sizes for macOS window sizing
public enum MobileDeviceSize {
    case iPhone11           // 414 x 896
    case iPhone13Mini       // 375 x 812
    case iPhone13Pro        // 390 x 844
    case iPhone15Pro        // 393 x 852
    case iPhoneSE           // 375 x 667
    case custom(width: CGFloat, height: CGFloat)

    public var width: CGFloat {
        switch self {
        case .iPhone11: return 414
        case .iPhone13Mini, .iPhoneSE: return 375
        case .iPhone13Pro: return 390
        case .iPhone15Pro: return 393
        case .custom(let width, _): return width
        }
    }

    public var height: CGFloat {
        switch self {
        case .iPhone11: return 896
        case .iPhone13Mini: return 812
        case .iPhone13Pro: return 844
        case .iPhone15Pro: return 852
        case .iPhoneSE: return 667
        case .custom(_, let height): return height
        }
    }
}

/// macOS LxApp implementation - exact port of macOSLxApp
@MainActor
public class macOSLxApp: ObservableObject {
    public static let shared = macOSLxApp()
    private static var isInitialized = false
    private static let log = OSLog(subsystem: "LingXia", category: "macOSLxApp")

    private static var activeWindowControllers: [LxAppWindowController] = []
    private static var tabWindowController: LxAppWindowController?

    private init() {}

    /// Open specific LxApp
    public static func openLxApp(appId: String, path: String) {
        // Get app info and cache initial route for navigation logic
        let lxappInfo = getLxAppInfo(appId)
        let initialRoute = lxappInfo.initial_route.toString()
        LxPageNavigation.cacheInitialRoute(appId: appId, initialRoute: initialRoute)

        // Use initial route if path is empty
        let requestedPath = path.isEmpty ? initialRoute : path

        // Check if using tab style
        if LxAppWindowController.getWindowStyle() == .tabStyle {
            if let tabController = tabWindowController {
                tabController.openLxApp(appId: appId, path: requestedPath)
                tabController.window?.makeKeyAndOrderFront(nil)
            } else {
                openTabStyleWindow()
                tabWindowController?.openLxApp(appId: appId, path: requestedPath)
            }
            return
        }

        // Check if window already exists for this app
        if let existingController = activeWindowControllers.first(where: { $0.appId == appId }) {
            let _ = onLxappOpened(appId, requestedPath)
            existingController.window?.makeKeyAndOrderFront(nil as Any?)
            switchPage(appId: appId, path: requestedPath)
            return
        }

        let storedPath = LxAppCore.getLastActivePath(for: appId)
        let actualPath = (!storedPath.isEmpty && storedPath != requestedPath && appId != LxAppCore.getHomeLxAppId()) ? storedPath : requestedPath

        let _ = onLxappOpened(appId, actualPath)

        let windowController = LxAppWindowController(appId: appId, path: actualPath)
        windowController.showWindow(nil as Any?)
        windowController.window?.makeKeyAndOrderFront(nil as Any?)
        NSApp.activate(ignoringOtherApps: true)
        activeWindowControllers.append(windowController)
    }

    private static func openTabStyleWindow() {
        if tabWindowController == nil {
            tabWindowController = LxAppWindowController()
            tabWindowController?.showWindow(nil)
            NSApp.activate(ignoringOtherApps: true)
        } else {
            tabWindowController?.window?.makeKeyAndOrderFront(nil)
        }
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

    /// Remove window controller from active list
    internal static func removeWindowController(_ controller: LxAppWindowController) {
        activeWindowControllers.removeAll { $0 === controller }
    }

    /// Remove tab window controller
    public static func removeTabWindowController(_ controller: LxAppWindowController) {
        if tabWindowController === controller {
            tabWindowController = nil
        }
    }

    /// Set window size for all LxApp windows using physical dimensions
    /// - Parameters:
    ///   - widthCm: Window width in centimeters
    ///   - heightCm: Window height in centimeters
    public static func setWindowSize(widthCm: CGFloat, heightCm: CGFloat) {
        let widthInches = widthCm / 2.54
        let heightInches = heightCm / 2.54

        guard let screen = NSScreen.main else {
            let defaultDPI: CGFloat = 72.0
            let widthPoints = widthInches * defaultDPI
            let heightPoints = heightInches * defaultDPI
            LxAppWindowController.setWindowSize(width: widthPoints, height: heightPoints)
            return
        }

        let dpi = screen.backingScaleFactor * 72.0
        let widthPoints = widthInches * dpi
        let heightPoints = heightInches * dpi
        LxAppWindowController.setWindowSize(width: widthPoints, height: heightPoints)
    }

    /// Set window size using predefined device size (convenience method)
    /// - Parameter deviceSize: Predefined device size to use
    public static func setWindowSize(_ deviceSize: MobileDeviceSize) {
        LxAppWindowController.setWindowSize(width: deviceSize.width, height: deviceSize.height)
    }

    /// Set window style for all LxApp windows
    /// - Parameter style: Window style to use
    public static func setWindowStyle(_ style: LxAppWindowStyle) {
        let oldStyle = LxAppWindowController.getWindowStyle()

        LxAppWindowController.setWindowStyle(style)

        // If switching from tab style to capsule style, close tab window
        if oldStyle == .tabStyle && style == .capsuleStyle {
            if let tabController = tabWindowController {
                tabController.window?.close()
                tabWindowController = nil
            }
        }

        // If switching from capsule style to tab style, close all individual windows
        if oldStyle == .capsuleStyle && style == .tabStyle {
            for controller in activeWindowControllers {
                controller.window?.close()
            }
            activeWindowControllers.removeAll()
        }
    }

    /// Open home LxApp
    internal static func openHomeLxApp() {
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else {
            return
        }

        // Ensure we're on the main thread for UI operations
        if Thread.isMainThread {
            performOpenHomeLxApp(homeLxAppId: homeLxAppId)
        } else {
            DispatchQueue.main.async {
                performOpenHomeLxApp(homeLxAppId: homeLxAppId)
            }
        }
    }

    private static func performOpenHomeLxApp(homeLxAppId: String) {
        let currentStyle = LxAppWindowController.getWindowStyle()

        if currentStyle == .tabStyle {
            openTabStyleWindow()
        } else {
            // Pass empty path - openLxApp will use initial route
            openLxApp(appId: homeLxAppId, path: "")
        }
    }

    /// Get active window controllers
    internal static func getActiveWindowControllers() -> [LxAppWindowController] {
        return activeWindowControllers
    }

    /// Initialize LxApps system
    /// - Returns: true if initialization successful, false otherwise
    public static func initialize() -> Bool {
        // Check if already initialized
        if isInitialized {
            return true
        }

        // Use LxAppCore.initializeCore() instead of duplicating the logic
        LxAppCore.initializeCore()

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
