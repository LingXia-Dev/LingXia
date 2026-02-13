import Foundation
import WebKit
import CLingXiaRustAPI
import OSLog
@preconcurrency import ObjectiveC

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// WebView extensions for display-related functionality
extension WKWebView {

    private static var appIdKey: UInt8 = 0
    private static var currentPathKey: UInt8 = 0

    var appId: String? {
        get {
            return objc_getAssociatedObject(self, &Self.appIdKey) as? String
        }
        set {
            objc_setAssociatedObject(self, &Self.appIdKey, newValue, .OBJC_ASSOCIATION_COPY_NONATOMIC)
        }
    }

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
        #if os(iOS)
        NativeBridge.notifyPageInactive(for: self)
        #elseif os(macOS)
        MacNativeBridge.notifyPageInactive(for: self)
        #endif
    }

    /// Show WebView
    @MainActor
    func resumeWebView() {
        isHidden = false
        #if os(iOS)
        NativeBridge.notifyPageActive(for: self)
        #elseif os(macOS)
        MacNativeBridge.notifyPageActive(for: self)
        #endif
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

/// Shared WebView manager
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

        #if os(iOS)
        // Ensure native component bridge is installed before page load so JS can see window.webkit.messageHandlers.NativeComponent
        NativeBridge.attachIfNeeded(to: webView)
        #endif

        return webView
    }

    /// Switch between WebViews
    public static func switchWebView(from current: WKWebView?, to new: WKWebView?) {
        current?.pauseWebView()
        new?.resumeWebView()
    }

    /// Shared WebView attachment logic
    public static func attachWebViewToContainer(_ webView: WKWebView, container: PlatformView, constraints: [NSLayoutConstraint]? = nil) {
        // Remove from previous parent if any
        webView.removeFromSuperview()

        // Add to new container
        container.addSubview(webView)
        webView.translatesAutoresizingMaskIntoConstraints = false

        // Apply provided constraints or default full-container constraints
        if let customConstraints = constraints {
            NSLayoutConstraint.activate(customConstraints)
        } else {
            NSLayoutConstraint.activate([
                webView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                webView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                webView.topAnchor.constraint(equalTo: container.topAnchor),
                webView.bottomAnchor.constraint(equalTo: container.bottomAnchor)
            ])
        }

        // Force layout and show
        #if os(iOS)
        container.setNeedsLayout()
        container.layoutIfNeeded()
        #else
        container.needsLayout = true
        container.layoutSubtreeIfNeeded()
        #endif

        webView.resumeWebView()

        // Trigger onPageShow when WebView is attached and visible
        if let appId = webView.appId, let path = webView.currentPath {
            lingxia.onPageShow(appId, path)
        }
    }

    /// Configure WebView transparency - shared logic with platform-specific optimizations
    public static func configureWebViewTransparency(_ webView: WKWebView, transparent: Bool) {
        #if os(iOS)
        let backgroundColor = transparent ? PlatformColor.clear : PlatformColor.systemBackground
        let isOpaque = !transparent

        // Configure WebView
        webView.backgroundColor = backgroundColor
        webView.isOpaque = isOpaque
        webView.layer.backgroundColor = backgroundColor.cgColor

        // Configure ScrollView (iOS-specific)
        webView.scrollView.backgroundColor = backgroundColor
        webView.scrollView.isOpaque = isOpaque
        webView.scrollView.layer.backgroundColor = backgroundColor.cgColor
        webView.scrollView.layer.isOpaque = isOpaque

        // Configure scroll behavior
        webView.scrollView.contentInsetAdjustmentBehavior = .never
        webView.scrollView.indicatorStyle = .default
        webView.scrollView.showsVerticalScrollIndicator = true
        webView.scrollView.showsHorizontalScrollIndicator = true
        #else
        let backgroundColor = transparent ? PlatformColor.clear : PlatformColor.controlBackgroundColor
        webView.layer?.backgroundColor = backgroundColor.cgColor
        webView.setValue(transparent, forKey: "drawsTransparentBackground")
        #endif
    }

    /// Enable WebView debugging globally
    public static func enableDebugging() {
        debuggingEnabled = true
    }
}

#if os(iOS)
public typealias PlatformView = UIView
#else
public typealias PlatformView = NSView
#endif
