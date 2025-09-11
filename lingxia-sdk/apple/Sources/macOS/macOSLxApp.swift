import SwiftUI
import os.log
import CLingXiaRustAPI

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

/// iPhone notch specifications for accurate system status bar simulation
public enum iPhoneNotchSpec: Sendable {
    case iPhone11           // Standard notch
    case iPhone13Mini       // Standard notch
    case iPhone13Pro        // Standard notch
    case iPhone15Pro        // Dynamic Island
    case iPhoneSE           // No notch
    case custom(width: CGFloat, height: CGFloat)

    public var width: CGFloat {
        switch self {
        case .iPhone11: return 210        // iPhone 11 notch width (actual: ~210pt)
        case .iPhone13Mini: return 210    // iPhone 13 Mini notch width (actual: ~210pt)
        case .iPhone13Pro: return 210     // iPhone 13 Pro notch width (actual: ~210pt)
        case .iPhone15Pro: return 126     // iPhone 15 Pro Dynamic Island width (actual: 126pt)
        case .iPhoneSE: return 0          // No notch
        case .custom(let width, _): return width
        }
    }

    public var height: CGFloat {
        switch self {
        case .iPhone11: return 30         // iPhone 11 notch height (actual: 30pt)
        case .iPhone13Mini: return 30     // iPhone 13 Mini notch height (actual: 30pt)
        case .iPhone13Pro: return 30      // iPhone 13 Pro notch height (actual: 30pt)
        case .iPhone15Pro: return 37      // iPhone 15 Pro Dynamic Island height (actual: 37pt)
        case .iPhoneSE: return 0          // No notch
        case .custom(_, let height): return height
        }
    }

    public var cornerRadius: CGFloat {
        switch self {
        case .iPhone11, .iPhone13Mini, .iPhone13Pro: return 15  // Standard notch corner radius
        case .iPhone15Pro: return 18.5                          // Dynamic Island corner radius (actual: 18.5pt)
        case .iPhoneSE: return 0                                 // No notch
        case .custom: return 15                                  // Default corner radius
        }
    }

    public var statusBarHeight: CGFloat {
        switch self {
        case .iPhone11: return 44         // iPhone 11 status bar height (actual: 44pt)
        case .iPhone13Mini: return 44     // iPhone 13 Mini status bar height (actual: 44pt)
        case .iPhone13Pro: return 47      // iPhone 13 Pro status bar height (actual: 47pt)
        case .iPhone15Pro: return 54      // iPhone 15 Pro status bar height (actual: 54pt)
        case .iPhoneSE: return 20         // iPhone SE status bar height (actual: 20pt)
        case .custom: return 44           // Default status bar height
        }
    }
}

/// macOS LxApp implementation
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
        os_log("macOS openLxApp: %@ at path: %@", log: log, type: .info, appId, path)
        LxAppCore.executeOpenLxApp(appId: appId, path: path)
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
        // Call FFI close handler first
        let _ = onLxappClosed(appId)

        // Get next LxApp from Rust stack and open it
        let currentLxApp = getCurrentLxApp()
        let appidStr = currentLxApp.appid.toString()
        let pathStr = currentLxApp.path.toString()
        if !appidStr.isEmpty {
            os_log("Opening next LxApp from stack: %@:%@", log: log, type: .info, appidStr, pathStr)
            openLxApp(appId: appidStr, path: pathStr)
        } else {
            os_log("No more LxApps in stack", log: log, type: .info)
        }
    }

    /// Navigate to page with specific navigation type
    public static func navigate(appId: String, path: String, navigationType: NavigationType) {
        LxAppCore.executeNavigation(appId: appId, path: path, navigationType: navigationType)
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

        // Update the system status bar to match the device
        updateSystemStatusBarForDevice(deviceSize)
    }

    /// Update the system status bar specification to match the selected device
    private static func updateSystemStatusBarForDevice(_ deviceSize: MobileDeviceSize) {
        let notchSpec: iPhoneNotchSpec

        switch deviceSize {
        case .iPhone11:
            notchSpec = .iPhone11
        case .iPhone13Mini:
            notchSpec = .iPhone13Mini
        case .iPhone13Pro:
            notchSpec = .iPhone13Pro
        case .iPhone15Pro:
            notchSpec = .iPhone15Pro
        case .iPhoneSE:
            notchSpec = .iPhoneSE
        case .custom:
            notchSpec = .iPhoneSE  // Default for custom sizes
        }

        // Update the current notch specification
        LxAppWindowController.Layout.currentNotchSpec = notchSpec
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
        guard let homeLxAppId = LxAppCore.getHomeLxAppId() else { return }

        Task { @MainActor in
            if LxAppWindowController.getWindowStyle() == .tabStyle {
                openTabStyleWindow()
            } else {
                openLxApp(appId: homeLxAppId, path: "")
            }
        }
    }

    /// Get active window controllers
    internal static func getActiveWindowControllers() -> [LxAppWindowController] {
        return activeWindowControllers
    }

    /// Initialize LxApps system
    public static func initialize() -> Bool {
        if isInitialized { return true }

        LxAppCore.initializeCore()
        isInitialized = LxAppCore.getHomeLxAppId() != nil

        if !isInitialized {
            os_log("Failed to initialize LxApps - no home app ID", log: log, type: .error)
        }
        return isInitialized
    }
}

// Direct platform implementation (no more LxAppRenderer protocol)
extension macOSLxApp {
    /// Direct openLxApp implementation (called from LxAppCore)
    internal static func openLxAppDirect(appId: String, path: String) {
        // Handle macOS-specific window/tab creation logic
        if LxAppWindowController.getWindowStyle() == .tabStyle {
            shared.handleTabStyleOpenLxApp(appId: appId, path: path)
        } else {
            shared.handleCapsuleStyleOpenLxApp(appId: appId, path: path)
        }
    }

    /// Direct navigation implementation (called from LxAppCore)
    internal static func handleNavigationDirect(_ plan: NavigationPlan) {
        // Platform-specific setup/switch WebView first
        handlePlatformSpecificNavigationDirect(plan)

        // Render UI components based on state
        renderTabBarDirect(plan.tabBarState, appId: plan.appId, path: plan.path)
        renderNavigationBarDirect(plan.navBarState)
        renderCapsuleButtonDirect(appId: plan.appId)
    }

    /// Direct platform-specific navigation logic
    private static func handlePlatformSpecificNavigationDirect(_ plan: NavigationPlan) {
        // Handle macOS-specific window/tab management
        if plan.navigationType == .launch {
            // Launch navigation is handled in openLxApp
            return
        } else {
            shared.handleRegularNavigation(plan)
        }
    }

    /// Direct TabBar rendering
    private static func renderTabBarDirect(_ state: TabBarState, appId: String, path: String) {
        // Find the appropriate view controller
        let viewController: macOSLxAppViewController? = {
            if let controller = Self.activeWindowControllers.first(where: { $0.appId == appId }) {
                return controller.window?.contentViewController as? macOSLxAppViewController
            } else if let tabController = Self.tabWindowController {
                return tabController.getViewController(for: appId)
            }
            return nil
        }()

        guard let vc = viewController else { return }

        // Use state visibility (from prepareNavigation)
        if state.show {
            vc.syncTabBarSelection(path: path)
        }
        vc.showTabBar(state.show)
    }

    /// Direct NavigationBar rendering
    private static func renderNavigationBarDirect(_ state: NavBarState) {
        guard state.shouldUpdate else { return }

        if let controller = Self.activeWindowControllers.first(where: { $0.appId == state.appId }),
           let viewController = controller.window?.contentViewController as? macOSLxAppViewController {
            viewController.updateNavigationBar(appId: state.appId, path: state.path)
        } else if let tabController = Self.tabWindowController,
                  let viewController = tabController.getViewController(for: state.appId) {
            viewController.updateNavigationBar(appId: state.appId, path: state.path)
        }
    }

    /// Direct Capsule button rendering
    private static func renderCapsuleButtonDirect(appId: String) {
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
        // Use the provided path directly since we now have centralized state management
        let actualPath = path

        let windowController = LxAppWindowController(appId: appId, path: actualPath)
        windowController.showWindow(nil as Any?)
        windowController.window?.makeKeyAndOrderFront(nil as Any?)
        NSApp.activate(ignoringOtherApps: true)
        Self.activeWindowControllers.append(windowController)
    }

    fileprivate func handleRegularNavigation(_ plan: NavigationPlan) {
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
