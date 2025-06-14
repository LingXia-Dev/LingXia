import UIKit
import WebKit
import os.log

/// Extensions for WKWebView to work with Rust-created WebViews
/// This provides compatibility methods for WebViews created by Rust using objc2
extension WKWebView {

    /// Get the app ID associated with this WebView (stored in accessibilityIdentifier)
    var appId: String? {
        get { return accessibilityIdentifier }
        set { accessibilityIdentifier = newValue }
    }

    /// Get the current path associated with this WebView (stored in accessibilityLabel)
    var currentPath: String? {
        get { return accessibilityLabel }
        set { accessibilityLabel = newValue }
    }

    /// Pause WebView operations (simplified version for Rust WebViews)
    @objc func pauseWebView() {
        isHidden = true
    }

    /// Resume WebView operations (simplified version for Rust WebViews)
    @objc func resumeWebView() {
        isHidden = false
    }

    /// Set up the WebView with app ID and path for Rust integration
    /// Note: All WebView configuration is now handled by Rust layer
    @MainActor
    func setup(appId: String, path: String) {
        self.appId = appId
        self.currentPath = path
    }
}

/// Helper class for managing Rust-created WebViews in Swift
class WebViewManager {
    private static let log = OSLog(subsystem: "LingXia", category: "RustWebView")

    /// Find WebView for the given appId and path from Rust layer
    @MainActor
    static func findWebView(appId: String, path: String) -> WKWebView? {
        let webViewPtr = lingxia.findWebView(appId, path)

        if webViewPtr != 0 {
            // Convert raw WKWebView pointer from Rust directly to WKWebView
            let webView = Unmanaged<WKWebView>.fromOpaque(UnsafeRawPointer(bitPattern: webViewPtr)!).takeUnretainedValue()

            // Set up the WebView with app info
            webView.setup(appId: appId, path: path)

            os_log("Found existing WebView for %@ at %@ - using Rust-created WKWebView directly",
                   log: log, type: .info, appId, path)
            return webView
        }

        return nil
    }
}
