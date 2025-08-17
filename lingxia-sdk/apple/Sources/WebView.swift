import Foundation
import WebKit
import CLingXiaFFI
import OSLog
@preconcurrency import ObjectiveC

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// WebView extensions for display-related functionality
extension WKWebView {

    /// Associated object keys for WebView properties
    private static var appIdKey: UInt8 = 0
    private static var currentPathKey: UInt8 = 0

    /// App ID (stored using Associated Objects)
    var appId: String? {
        get {
            return objc_getAssociatedObject(self, &Self.appIdKey) as? String
        }
        set {
            objc_setAssociatedObject(self, &Self.appIdKey, newValue, .OBJC_ASSOCIATION_COPY_NONATOMIC)
        }
    }

    /// Current path (stored using Associated Objects)
    var currentPath: String? {
        get {
            return objc_getAssociatedObject(self, &Self.currentPathKey) as? String
        }
        set {
            objc_setAssociatedObject(self, &Self.currentPathKey, newValue, .OBJC_ASSOCIATION_COPY_NONATOMIC)
        }
    }

    /// Simple page loaded check
    var pageLoaded: Bool {
        return url != nil && !isLoading
    }

    /// Hide WebView
    @MainActor
    func pauseWebView() {
        isHidden = true
    }

    /// Show WebView
    @MainActor
    func resumeWebView() {
        isHidden = false
    }

    /// Setup WebView with app info
    @MainActor
    func setup(appId: String, path: String) {
        self.appId = appId
        self.currentPath = path
    }

    /// Registration state
    var isRegistered: Bool {
        get {
            return objc_getAssociatedObject(self, &AssociatedKeys.isRegistered) as? Bool ?? false
        }
        set {
            objc_setAssociatedObject(self, &AssociatedKeys.isRegistered, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        }
    }
}

/// Associated object keys
private struct AssociatedKeys {
    nonisolated(unsafe) static var isRegistered: UInt8 = 0
}

/// Simple WebView manager - Rust handles creation/lifecycle
@MainActor
public class WebViewManager {
    private static let log = OSLog(subsystem: "LingXia", category: "WebView")
    private static var debuggingEnabled = false

    /// Find WebView from Rust layer
    public static func findWebView(appId: String, path: String) -> WKWebView? {
        let webViewPtr = lingxia.findWebView(appId, path)
        guard webViewPtr != 0 else { return nil }

        // Safely convert pointer to WebView with error handling
        guard let rawPointer = UnsafeRawPointer(bitPattern: webViewPtr) else {
            os_log("Warning: Invalid WebView pointer received from Rust layer", log: log, type: .error)
            return nil
        }

        let webView = Unmanaged<WKWebView>.fromOpaque(rawPointer).takeUnretainedValue()
        webView.setup(appId: appId, path: path)

        if debuggingEnabled {
            if #available(iOS 16.4, macOS 13.3, *) {
                webView.isInspectable = true
            }
        }

        return webView
    }

    /// Switch between WebViews
    static func switchWebView(from current: WKWebView?, to new: WKWebView?) {
        current?.pauseWebView()
        new?.resumeWebView()
    }

    /// Enable WebView debugging globally
    /// This affects all WKWebView instances created after this call
    /// Apple platform support turn on debugging for webview instance, but in order
    /// to align with Android/Harmony, we provide the same mechanism also.
    public static func enableDebugging() {
        debuggingEnabled = true
    }
}
