import Foundation
import WebKit
import os.log
import CLingXiaFFI
@preconcurrency import ObjectiveC

#if os(iOS)
import UIKit
#elseif os(macOS)
import Cocoa
#endif

/// Shared WebView extensions for both iOS and macOS
extension WKWebView {

    /// Get the app ID associated with this WebView (stored in accessibilityIdentifier)
    var appId: String? {
        get {
            #if os(iOS)
            return accessibilityIdentifier
            #else
            return accessibilityIdentifier()
            #endif
        }
        set {
            #if os(iOS)
            accessibilityIdentifier = newValue
            #else
            setAccessibilityIdentifier(newValue)
            #endif
        }
    }

    /// Get the current path associated with this WebView (stored in accessibilityLabel)
    var currentPath: String? {
        get {
            #if os(iOS)
            return accessibilityLabel
            #else
            return accessibilityLabel()
            #endif
        }
        set {
            #if os(iOS)
            accessibilityLabel = newValue
            #else
            setAccessibilityLabel(newValue)
            #endif
        }
    }

    /// Check if page is loaded
    var pageLoaded: Bool {
        return url != nil && !isLoading
    }

    /// Pause WebView operations
    @objc func pauseWebView() {
        isHidden = true
    }

    /// Resume WebView operations
    @objc func resumeWebView() {
        isHidden = false
    }

    /// Set up the WebView with app ID and path for Rust integration
    @MainActor
    func setup(appId: String, path: String) {
        self.appId = appId
        self.currentPath = path
    }

    /// Check if WebView is registered with Rust layer
    var isRegistered: Bool {
        get {
            return objc_getAssociatedObject(self, &AssociatedKeys.isRegistered) as? Bool ?? false
        }
        set {
            objc_setAssociatedObject(self, &AssociatedKeys.isRegistered, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        }
    }
}

/// Associated object keys for WebView properties
private struct AssociatedKeys {
    nonisolated(unsafe) static var isRegistered = "isRegistered"
}

/// WebView manager for both iOS and macOS
class WebViewManager {
    private static let log = OSLog(subsystem: "LingXia", category: "WebView")

    /// Find WebView for the given appId and path from Rust layer
    @MainActor
    static func findWebView(appId: String, path: String) -> WKWebView? {
        let webViewPtr = lingxia.findWebView(appId, path)
        guard webViewPtr != 0 else { return nil }

        let webView = Unmanaged<WKWebView>.fromOpaque(UnsafeRawPointer(bitPattern: webViewPtr)!).takeUnretainedValue()
        webView.setup(appId: appId, path: path)
        return webView
    }


}
