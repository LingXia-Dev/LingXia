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

/// Shared UI layout constants for macOS windows
public struct LxAppWindowLayout {
    public static let titleBarHeight: CGFloat = 32        // SwiftUI custom title bar height
    public static let macOSTabViewHeight: CGFloat = 32    // macOS window tab view height (for switching between LxApps)
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

/// macOS LxApp implementation
@MainActor
public class macOSLxApp: ObservableObject, LxAppRenderer {
    public static let shared = macOSLxApp()
    private static var isInitialized = false
    private static let log = OSLog(subsystem: "LingXia", category: "macOSLxApp")

    private static var activeWindowControllers: [LxAppWindowController] = []
    private static var tabWindowController: LxAppWindowController?

    private init() {}

    /// Open specific LxApp
    public static func openLxApp(appId: String, path: String) {
        os_log("macOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)

        // Use shared core logic
        LxAppCore.executeOpenLxApp(appId: appId, path: path, renderer: shared)
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

    /// Navigate to page with specific navigation type
    public static func navigate(appId: String, path: String, navigationType: NavigationType) {
        // Use shared core logic
        LxAppCore.executeNavigation(appId: appId, path: path, navigationType: navigationType, renderer: shared)
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

// LxAppRenderer Protocol Implementation
extension macOSLxApp {
    /// Handle platform-specific openLxApp setup
    public func openLxApp(appId: String, path: String) {
        // Handle macOS-specific window/tab creation logic
        if LxAppWindowController.getWindowStyle() == .tabStyle {
            handleTabStyleOpenLxApp(appId: appId, path: path)
        } else {
            handleCapsuleStyleOpenLxApp(appId: appId, path: path)
        }
    }

    /// Render TabBar based on state
    public func renderTabBar(_ state: TabBarState, appId: String, path: String) {

        // Find the appropriate view controller
        if let controller = Self.activeWindowControllers.first(where: { $0.appId == appId }),
           let viewController = controller.window?.contentViewController as? macOSLxAppViewController {
            viewController.showTabBar(state.show)
            if state.updateSelection, let selectedPath = state.selectedPath {
                viewController.syncTabBarWithPath(selectedPath)
            }
        } else if let tabController = Self.tabWindowController,
                  let viewController = tabController.getViewController(for: appId) {
            viewController.showTabBar(state.show)
            if state.updateSelection, let selectedPath = state.selectedPath {
                viewController.syncTabBarWithPath(selectedPath)
            }
        }
    }

    /// Render NavigationBar based on state
    public func renderNavigationBar(_ state: NavBarState) {
        guard state.shouldUpdate else { return }

        if let controller = Self.activeWindowControllers.first(where: { $0.appId == state.appId }),
           let viewController = controller.window?.contentViewController as? macOSLxAppViewController {
            viewController.updateNavigationBar(appId: state.appId, path: state.path)
        } else if let tabController = Self.tabWindowController,
                  let viewController = tabController.getViewController(for: state.appId) {
            viewController.updateNavigationBar(appId: state.appId, path: state.path)
        }
    }

    /// Render Capsule button - macOS capsuleStyle always shows capsule buttons
    public func renderCapsuleButton(appId: String) {
        // In capsule style mode, WindowController handles floating capsule buttons
        if LxAppWindowManager.shared.windowStyle == .capsuleStyle {
            if let controller = Self.activeWindowControllers.first(where: { $0.appId == appId }),
               let floatingContainer = controller.floatingCapsuleContainer {
                floatingContainer.isHidden = false // Always show in macOS capsuleStyle
            }
            return
        }

        // Tab mode logic - only update when switching apps
        if LxAppWindowManager.shared.windowStyle == .tabStyle {
            if let tabController = Self.tabWindowController {
                let currentActiveAppId = LxAppTabManager.shared.activeTab?.appId
                if currentActiveAppId != appId,
                   let viewController = tabController.getViewController(for: appId) {
                    viewController.updateCapsuleButtonVisibility(appId: appId)
                }
            }
            return
        }

        // Fallback for other modes
        if let controller = Self.activeWindowControllers.first(where: { $0.appId == appId }),
           let viewController = controller.window?.contentViewController as? macOSLxAppViewController {
            viewController.updateCapsuleButtonVisibility(appId: appId)
        }
    }

    /// Execute lifecycle action
    public func executeLifecycleAction(_ action: LifecycleAction, appId: String, path: String) {

        switch action {
        case .openApp:
            // onLxappOpened already called in prepareOpenLxApp
            lingxia.onPageShow(appId, path)
        case .switchTab:
            // Handle tab switch logic
            lingxia.onPageShow(appId, path)
        case .pageShow:
            lingxia.onPageShow(appId, path)
        case .backPressed:
            let handled = lingxia.onBackPressed(appId)
            if !handled {
                lingxia.onPageShow(appId, path)
            }
        }
    }

    /// Handle platform-specific navigation logic
    public func handlePlatformSpecificNavigation(_ plan: NavigationPlan) {

        // Handle macOS-specific window/tab management
        if plan.navigationType == .launch {
            // Launch navigation is handled in openLxApp
            return
        } else {
            handleRegularNavigation(plan)
        }
    }

    /// Get current path for duplicate navigation check
    public func getCurrentPath(for appId: String) -> String? {
        // Check in individual windows first
        if let controller = Self.activeWindowControllers.first(where: { $0.appId == appId }),
           let viewController = controller.window?.contentViewController as? macOSLxAppViewController {
            return viewController.currentWebView?.currentPath
        }

        // Check in tab window controller
        if let tabController = Self.tabWindowController,
           let viewController = tabController.getViewController(for: appId) {
            return viewController.currentWebView?.currentPath
        }

        return nil
    }

    private func handleTabStyleOpenLxApp(appId: String, path: String) {
        if let tabController = Self.tabWindowController {
            tabController.openLxApp(appId: appId, path: path)
            tabController.window?.makeKeyAndOrderFront(nil)
        } else {
            Self.openTabStyleWindow()
            Self.tabWindowController?.openLxApp(appId: appId, path: path)
        }
    }

    private func handleCapsuleStyleOpenLxApp(appId: String, path: String) {
        // Check if window already exists for this app
        if let existingController = Self.activeWindowControllers.first(where: { $0.appId == appId }) {
            existingController.window?.makeKeyAndOrderFront(nil as Any?)
            return
        }

        // Create new window controller
        let storedPath = LxAppCore.getLastActivePath(for: appId)
        let actualPath = (!storedPath.isEmpty && storedPath != path && appId != LxAppCore.getHomeLxAppId()) ? storedPath : path

        let windowController = LxAppWindowController(appId: appId, path: actualPath)
        windowController.showWindow(nil as Any?)
        windowController.window?.makeKeyAndOrderFront(nil as Any?)
        NSApp.activate(ignoringOtherApps: true)
        Self.activeWindowControllers.append(windowController)
    }

    private func handleRegularNavigation(_ plan: NavigationPlan) {
        // Find the appropriate view controller and delegate navigation
        if let controller = Self.activeWindowControllers.first(where: { $0.appId == plan.appId }),
           let viewController = controller.window?.contentViewController as? macOSLxAppViewController {
            viewController.navigate(appId: plan.appId, to: plan.path, with: plan.navigationType)
        } else if let tabController = Self.tabWindowController,
                  let viewController = tabController.getViewController(for: plan.appId) {
            viewController.navigate(appId: plan.appId, to: plan.path, with: plan.navigationType)
        }
    }
}

#endif
