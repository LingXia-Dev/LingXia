import Foundation
import CLingXiaFFI

#if os(iOS)
import UIKit
#elseif os(macOS)
import Cocoa
#endif

/// Main LxApp interface - unified API for both iOS and macOS
/// This class provides a clean, consistent API that delegates to platform-specific implementations
@MainActor
public class LxApp {

    /// Initialize the LxApp system
    public static func initialize() {
        #if os(iOS)
        iOSLxApp.initialize()
        #elseif os(macOS)
        let _ = macOSLxApp.initialize()
        #endif
    }

    /// Set home LxApp configuration
    public static func setHomeLxApp(appId: String, initialRoute: String = "/") {
        LxAppCore.setHomeLxApp(appId: appId, initialRoute: initialRoute)
    }

    #if os(iOS)
    /// Set launch mode (iOS only)
    public static func setLaunchMode(_ mode: LxAppLaunchMode) {
        LxAppCore.setLaunchMode(mode)
    }

    /// Configure transparent system bars (iOS only)
    public static func configureTransparentSystemBars(viewController: UIViewController, lightStatusBarIcons: Bool = false) {
        LxAppPlatform.configureTransparentSystemBars(viewController: viewController, lightStatusBarIcons: lightStatusBarIcons)
    }
    #elseif os(macOS)
    /// Set window size using physical dimensions (macOS only)
    /// - Parameters:
    ///   - widthCm: Window width in centimeters
    ///   - heightCm: Window height in centimeters
    public static func setWindowSize(widthCm: CGFloat, heightCm: CGFloat) {
        LxAppPlatform.setWindowSize(widthCm: widthCm, heightCm: heightCm)
    }

    /// Set window style (macOS only)
    /// - Parameter style: Window style to use
    public static func setWindowStyle(_ style: LxAppWindowStyle) {
        LxAppPlatform.setWindowStyle(style)
    }
    #endif

    /// Open home LxApp
    public static func openHomeLxApp() {
        LxAppPlatform.openHomeLxApp()
    }

    /// Open specific LxApp
    public static func openLxApp(appId: String, path: String = "") {
        LxAppPlatform.openLxApp(appId: appId, path: path)
    }

    /// Close LxApp
    public static func closeLxApp(appId: String) {
        LxAppPlatform.closeLxApp(appId: appId)
    }

    /// Switch to page in LxApp
    public static func switchPage(appId: String, path: String) {
        LxAppPlatform.switchPage(appId: appId, path: path)
    }
}

extension LxApp {
    /// Open specific LxApp
    nonisolated public static func openLxApp(appid: RustStr, path: RustStr) -> Bool {
        #if os(iOS)
        let appIdString = appid.toString()
        let pathString = path.toString()
        executeOnMainThread {
            LxAppPlatform.openLxApp(appId: appIdString, path: pathString)
        }
        return true
        #elseif os(macOS)
        return LxAppPlatform.openLxApp(appid: appid, path: path)
        #endif
    }

    /// Close LxApp
    nonisolated public static func closeLxApp(appid: RustStr) -> Bool {
        #if os(iOS)
        let appIdString = appid.toString()
        executeOnMainThread {
            LxAppPlatform.closeLxApp(appId: appIdString)
        }
        return true
        #elseif os(macOS)
        return LxAppPlatform.closeLxApp(appid: appid)
        #endif
    }

    /// Switch to page in LxApp
    nonisolated public static func switchPage(appid: RustStr, path: RustStr) -> Bool {
        #if os(iOS)
        let appIdString = appid.toString()
        let pathString = path.toString()
        executeOnMainThread {
            LxAppPlatform.switchPage(appId: appIdString, path: pathString)
        }
        return true
        #elseif os(macOS)
        return LxAppPlatform.switchPage(appid: appid, path: path)
        #endif
    }

    #if os(iOS)
    nonisolated private static func executeOnMainThread(_ action: @MainActor () -> Void) {
        if Thread.isMainThread {
            MainActor.assumeIsolated(action)
        } else {
            DispatchQueue.main.sync {
                MainActor.assumeIsolated(action)
            }
        }
    }
    #endif
}

#if os(iOS)
typealias LxAppPlatform = iOSLxApp
#elseif os(macOS)
typealias LxAppPlatform = macOSLxApp
#endif
