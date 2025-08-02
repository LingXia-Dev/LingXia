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
    /// Simulate a mobile device (macOS only) - automatically uses capsule window style
    /// - Parameters:
    ///   - width: Device width in points
    ///   - height: Device height in points
    public static func simulateMobileDevice(width: CGFloat, height: CGFloat) {
        LxAppPlatform.simulateMobileDevice(width: width, height: height)
    }

    /// Simulate a specific mobile device (macOS only) - automatically uses capsule window style
    /// - Parameter device: Predefined device to simulate
    public static func simulateDevice(_ device: MobileDevice) {
        LxAppPlatform.simulateDevice(device)
    }
    #endif

    /// Open home LxApp
    public static func openHomeLxApp() {
        LxAppPlatform.openHomeLxApp()
    }
}

/// FFI interface for LxApp
extension LxApp {
    /// Open specific LxApp
    nonisolated public static func openLxApp(appid: RustStr, path: RustStr) -> Bool {
        FFIUtils.executeFFICallWithRustStr(appid: appid, path: path) { appIdString, pathString in
            LxAppPlatform.openLxApp(appId: appIdString, path: pathString ?? "")
        }
        return true
    }

    /// Close LxApp
    nonisolated public static func closeLxApp(appid: RustStr) -> Bool {
        FFIUtils.executeFFICallWithSingleRustStr(appid: appid) { appIdString in
            LxAppPlatform.closeLxApp(appId: appIdString)
        }
        return true
    }

    /// Switch to page in LxApp
    nonisolated public static func switchPage(appid: RustStr, path: RustStr) -> Bool {
        FFIUtils.executeFFICallWithRustStr(appid: appid, path: path) { appIdString, pathString in
            LxAppPlatform.switchPage(appId: appIdString, path: pathString ?? "")
        }
        return true
    }
}

#if os(iOS)
typealias LxAppPlatform = iOSLxApp
#elseif os(macOS)
typealias LxAppPlatform = macOSLxApp
#endif
