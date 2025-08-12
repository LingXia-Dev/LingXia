import Foundation
import OSLog
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

    nonisolated(unsafe) fileprivate static let log = OSLog(subsystem: "LingXia", category: "LxApp")

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

    /// Enable WebView debugging
    public static func enableWebViewDebugging() {
        LxAppCore.enableWebViewDebugging()
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

    /// Set window size using predefined device size (macOS only)
    /// - Parameter deviceSize: Predefined device size to use
    public static func setWindowSize(_ deviceSize: MobileDeviceSize) {
        LxAppPlatform.setWindowSize(deviceSize)
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

    nonisolated public static func launchWithUrl(url: RustStr) {
        let urlString = url.toString()
        guard let url = URL(string: urlString) else {
            os_log(.error, log: Self.log, "Invalid URL for launchWithUrl: %{public}@", urlString)
            return
        }
        #if os(iOS)
        DispatchQueue.main.async {
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
        }
        #elseif os(macOS)
        NSWorkspace.shared.open(url)
        #endif
    }
}

#if os(iOS)
typealias LxAppPlatform = iOSLxApp
#elseif os(macOS)
typealias LxAppPlatform = macOSLxApp
#endif
