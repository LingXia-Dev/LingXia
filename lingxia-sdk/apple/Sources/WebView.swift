import Foundation
import WebKit
import CLingXiaFFI
@preconcurrency import ObjectiveC

#if os(iOS)
import UIKit
#elseif os(macOS)
import Cocoa
#endif

/// WebView extensions for display-related functionality
extension WKWebView {

    /// App ID (stored in accessibilityIdentifier)
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

    /// Current path (stored in accessibilityLabel)
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

    private static var debuggingEnabled = false

    /// Find WebView from Rust layer
    public static func findWebView(appId: String, path: String) -> WKWebView? {
        let webViewPtr = lingxia.findWebView(appId, path)
        guard webViewPtr != 0 else { return nil }

        let webView = Unmanaged<WKWebView>.fromOpaque(UnsafeRawPointer(bitPattern: webViewPtr)!).takeUnretainedValue()
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
