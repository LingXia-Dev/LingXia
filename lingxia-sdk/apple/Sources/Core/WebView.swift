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
    private static var nativeComponentPageInstanceKey: UInt8 = 0
    private static var nativeComponentAttachmentGenerationKey: UInt8 = 0

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

    var nativeComponentSurfaceBinding: NativeComponentSurfaceBinding {
        guard let pageInstanceID = objc_getAssociatedObject(
            self, &Self.nativeComponentPageInstanceKey) as? String,
            !pageInstanceID.isEmpty
        else {
            return .unproven
        }
        let generation = (objc_getAssociatedObject(
            self, &Self.nativeComponentAttachmentGenerationKey) as? NSNumber)?.uint64Value ?? 0
        let identity = UInt(bitPattern: Unmanaged.passUnretained(self).toOpaque())
        guard let binding = NativeComponentPageBinding(
            pageInstanceID: pageInstanceID,
            webViewIdentity: identity,
            attachmentGeneration: generation
        ) else {
            return .unproven
        }
        return .lxAppPage(binding)
    }

    /// Bind the native-component handler to one native-created PageInstance.
    /// Empty bindings explicitly classify browser/external surfaces as unproven.
    @MainActor
    fileprivate func bindNativeComponentPageInstance(_ rawPageInstanceID: String?) {
        let pageInstanceID = rawPageInstanceID?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let current = objc_getAssociatedObject(
            self, &Self.nativeComponentPageInstanceKey) as? String ?? ""
        guard current != pageInstanceID else { return }
        let generation = (objc_getAssociatedObject(
            self, &Self.nativeComponentAttachmentGenerationKey) as? NSNumber)?.uint64Value ?? 0
        objc_setAssociatedObject(
            self,
            &Self.nativeComponentAttachmentGenerationKey,
            NSNumber(value: generation &+ 1),
            .OBJC_ASSOCIATION_RETAIN_NONATOMIC
        )
        objc_setAssociatedObject(
            self,
            &Self.nativeComponentPageInstanceKey,
            pageInstanceID.isEmpty ? nil : pageInstanceID,
            .OBJC_ASSOCIATION_COPY_NONATOMIC
        )
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
        LxAppAppearanceRegistry.register(self, appId: appId)
        #if os(macOS)
        // The page decides whether it can be pulled, so the answer is re-read
        // here rather than per scroll event.
        lxPullToRefreshController?.refreshEnabledFlag()
        #endif
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

    /// Whether the canvas was configured transparent, so appearance changes
    /// leave it alone. Tracked here because the private key the transparency
    /// setter writes has no matching getter: reading it back throws
    /// NSUnknownKeyException.
    var drawsTransparentCanvas: Bool {
        get {
            return objc_getAssociatedObject(self, &AssociatedKeys.drawsTransparentCanvas) as? Bool ?? false
        }
        set {
            objc_setAssociatedObject(self, &AssociatedKeys.drawsTransparentCanvas, newValue, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        }
    }
}

/// Associated object keys
private struct AssociatedKeys {
    nonisolated(unsafe) static var isRegistered: UInt8 = 0
    nonisolated(unsafe) static var drawsTransparentCanvas: UInt8 = 0
}

/// Shared WebView manager
@MainActor
final class WebViewManager {
    private static let log = OSLog(subsystem: "LingXia", category: "WebView")
    private static var debuggingEnabled = false
    #if os(macOS)
    // Keep the Objective-C-visible browser subclass linked so Rust can instantiate it by name.
    private static let browserContextMenuWebViewClass: AnyClass = BrowserContextMenuWebView.self
    #endif

    static func registerRuntimeClasses() {
        #if os(macOS)
        _ = browserContextMenuWebViewClass
        #endif
    }

    static func findWebView(pageInstanceId: String) -> WKWebView? {
        let trimmed = pageInstanceId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let webViewPtr = lingxia.findWebViewByPageInstanceId(trimmed)
        guard webViewPtr != 0 else { return nil }
        guard let rawPointer = UnsafeRawPointer(bitPattern: webViewPtr) else {
            LXLog.error("Warning: Invalid WebView pointer received from Rust layer", category: "WebView")
            return nil
        }
        let webView = Unmanaged<WKWebView>.fromOpaque(rawPointer).takeUnretainedValue()
        webView.bindNativeComponentPageInstance(trimmed)
        if debuggingEnabled {
            if #available(iOS 16.4, macOS 13.3, *) {
                webView.isInspectable = true
            }
        }
        #if os(iOS)
        NativeBridge.attachIfNeeded(to: webView)
        #endif
        return webView
    }

    private static func lookupBinding(
        appId: String,
        path: String,
        sessionId: UInt64
    ) -> (pageInstanceId: String, webViewPtr: UInt)? {
        guard sessionId > 0 else {
            LXLog.error("lookupBinding rejected invalid session for \(appId)", category: "WebView")
            return nil
        }
        let binding = resolvePageBinding(appId, path, sessionId)
        let pageInstanceId = binding.page_instance_id
            .toString()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        // Browser tabs showing external documents have a WebView but no bound
        // lxapp PageInstance — a valid pointer alone is a usable binding.
        guard !pageInstanceId.isEmpty || binding.webview_ptr != 0 else {
            return nil
        }
        return (pageInstanceId: pageInstanceId, webViewPtr: binding.webview_ptr)
    }

    static func resolvePageInstanceId(appId: String, path: String, sessionId: UInt64) -> String? {
        guard let binding = lookupBinding(appId: appId, path: path, sessionId: sessionId),
              !binding.pageInstanceId.isEmpty else {
            return nil
        }
        return binding.pageInstanceId
    }

    static func resolveWebView(appId: String, path: String, sessionId: UInt64) -> WKWebView? {
        guard let binding = lookupBinding(appId: appId, path: path, sessionId: sessionId) else { return nil }
        let webViewPtr = binding.webViewPtr
        guard webViewPtr != 0 else { return nil }
        guard let rawPointer = UnsafeRawPointer(bitPattern: webViewPtr) else {
            LXLog.error("Warning: Invalid WebView pointer received from Rust layer", category: "WebView")
            return nil
        }
        let webView = Unmanaged<WKWebView>.fromOpaque(rawPointer).takeUnretainedValue()
        webView.bindNativeComponentPageInstance(binding.pageInstanceId)
        if debuggingEnabled {
            if #available(iOS 16.4, macOS 13.3, *) {
                webView.isInspectable = true
            }
        }
        #if os(iOS)
        NativeBridge.attachIfNeeded(to: webView)
        #endif
        webView.setup(appId: appId, path: path)
        return webView
    }

    /// Convenience resolve using stored runtime session for the app.
    static func resolveWebView(appId: String, path: String) -> WKWebView? {
        guard let sessionId = LxAppCore.sessionId(for: appId), sessionId > 0 else {
            LXLog.error("resolveWebView missing session for \(appId)", category: "WebView")
            return nil
        }
        return resolveWebView(appId: appId, path: path, sessionId: sessionId)
    }

    /// Switch between WebViews
    static func switchWebView(from current: WKWebView?, to new: WKWebView?) {
        current?.pauseWebView()
        new?.resumeWebView()
    }

    /// Mount an lxapp page's web view: the view itself, the native-component
    /// bridge, and — on macOS — pull-to-refresh. One call, so every host that
    /// shows a page gets the same set instead of assembling it by hand.
    @MainActor
    static func attachLxAppWebView(_ webView: WKWebView, to container: PlatformView) {
        #if os(macOS)
        // A leftover sibling (previous page, or its pull-to-refresh strip) sits
        // behind the current web view. Sliding this page down to reveal the
        // refresh indicator would then show that sibling instead of the dots.
        for subview in container.subviews {
            if let existing = subview as? WKWebView, existing !== webView {
                detachLxAppWebView(existing)
            }
        }
        // Pull-to-refresh moves the page by its own vertical constraints, so it
        // supplies them here instead of the default full-container pinning.
        let pullToRefresh = MacPullToRefreshController.attachIfNeeded(to: webView, in: container)
        attachWebViewToContainer(
            webView, container: container, constraints: pullToRefresh.pageConstraints(in: container))
        pullToRefresh.didAttachPage()
        MacNativeBridge.attachIfNeeded(to: webView, in: container)
        #else
        attachWebViewToContainer(webView, container: container)
        #endif
    }

    /// Remove an lxapp page and every container-owned companion installed for
    /// it. Callers must not detach the WKWebView first, because the refresh
    /// strip is a sibling and cannot then be discovered from the container.
    static func detachLxAppWebView(_ webView: WKWebView) {
        #if os(macOS)
        webView.lxPullToRefreshController?.unmount()
        #endif
        webView.pauseWebView()
        webView.removeFromSuperview()
    }

    /// Shared WebView attachment logic
    ///
    /// `reportsPageShow` drives the path-keyed show report. Pass false when the
    /// caller drives the page instance channel itself: that channel names the
    /// instance, and a bare route cannot name an isolated one.
    static func attachWebViewToContainer(
        _ webView: WKWebView,
        container: PlatformView,
        constraints: [NSLayoutConstraint]? = nil,
        reportsPageShow: Bool = true
    ) {
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
        if reportsPageShow, let appId = webView.appId, let path = webView.currentPath {
            lingxia.onPageShow(appId, path)
        }
    }

    /// Configure WebView transparency - shared logic with platform-specific optimizations
    static func configureWebViewTransparency(_ webView: WKWebView, transparent: Bool) {
        webView.drawsTransparentCanvas = transparent
        #if os(iOS)
        // Resolve from the appearance registry, not the webview's traits: this
        // path runs before the webview joins the hierarchy, where its trait
        // collection still reports the ambient (light) style, and it re-runs
        // on page display, so an ambient cgColor would re-freeze light.
        let dark =
            webView.appId
            .flatMap { LxAppAppearanceRegistry.resolvedDark(appId: $0) } ?? false
        let backgroundColor =
            transparent
            ? PlatformColor.clear
            : PlatformColor.systemBackground.resolvedColor(
                with: UITraitCollection(userInterfaceStyle: dark ? .dark : .light))
        let isOpaque = !transparent

        // Configure WebView
        webView.backgroundColor = backgroundColor
        webView.isOpaque = isOpaque
        webView.layer.backgroundColor = backgroundColor.cgColor
        if !transparent {
            webView.underPageBackgroundColor = backgroundColor
        }

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
        let backgroundColor = transparent
            ? PlatformColor.clear
            : opaquePageStyle(appId: webView.appId).color
        webView.wantsLayer = true
        webView.layer?.backgroundColor = backgroundColor.cgColor
        webView.underPageBackgroundColor = backgroundColor
        webView.setValue(transparent, forKey: "drawsTransparentBackground")
        #endif
    }

    #if os(macOS)
    /// Match the placeholder shown before an opaque lxapp WebView attaches.
    static func configureOpaquePagePlaceholder(_ view: NSView, appId: String?) {
        let style = opaquePageStyle(appId: appId)
        view.appearance = style.appearance
        view.wantsLayer = true
        view.layer?.backgroundColor = style.color.cgColor
    }

    private static func opaquePageStyle(appId: String?) -> (appearance: NSAppearance, color: NSColor) {
        let dark =
            appId.flatMap { LxAppAppearanceRegistry.resolvedDark(appId: $0) }
            ?? (NSApp?.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua)
        let appearance = NSAppearance(named: dark ? .darkAqua : .aqua)!

        // The host declares what the page actually paints, so prefer it over a
        // system colour that only happens to look plausible. `controlBackground`
        // is pure white, and a page on a tinted canvas — the showcase paints
        // #f2f2f7 — then flashes white before its own background arrives. iOS
        // already resolves the canvas this way; macOS was the one host reading
        // the declaration and never using it.
        if let declared = pageBackgroundHex(dark: dark) {
            return (appearance, declared)
        }

        var backgroundColor = NSColor.clear
        appearance.performAsCurrentDrawingAppearance {
            let color = NSColor.controlBackgroundColor
            backgroundColor = NSColor(cgColor: color.cgColor) ?? color
        }
        return (appearance, backgroundColor)
    }

    /// `#RRGGBB` from the host theme, as the CLI writes it into `app.json`.
    private static func pageBackgroundHex(dark: Bool) -> NSColor? {
        declaredPageColor(dark: dark).map { NSColor(cgColor: $0.cgColor) ?? $0 }
    }
    #endif

    /// The colour the page itself paints, as the host declared it.
    ///
    /// Chrome that sits over the page — a load placeholder, an overflow panel,
    /// the ground behind a transition — has to match this, not a system colour
    /// that merely looks like a surface. Every platform had invented its own
    /// answer here and they disagreed with each other and with the page.
    ///
    /// Returns `nil` when the host declares nothing, leaving the caller to pick
    /// a platform default rather than guessing on its behalf.
    static func declaredPageColor(dark: Bool) -> PlatformColor? {
        var value = pageBackgroundColor(dark).toString()
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if value.hasPrefix("#") { value.removeFirst() }
        guard value.count == 6, let rgb = UInt32(value, radix: 16) else { return nil }
        let red = CGFloat((rgb >> 16) & 0xFF) / 255
        let green = CGFloat((rgb >> 8) & 0xFF) / 255
        let blue = CGFloat(rgb & 0xFF) / 255
        #if os(macOS)
        return NSColor(srgbRed: red, green: green, blue: blue, alpha: 1)
        #else
        return UIColor(red: red, green: green, blue: blue, alpha: 1)
        #endif
    }

    /// Whether the lxapp is presenting dark, falling back to the system.
    static func resolvedDarkAppearance(appId: String?) -> Bool {
        if let appId, let resolved = LxAppAppearanceRegistry.resolvedDark(appId: appId) {
            return resolved
        }
        #if os(macOS)
        return NSApp?.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        #else
        return UITraitCollection.current.userInterfaceStyle == .dark
        #endif
    }

    /// The plate behind a tab bar overflow panel when the bar is transparent.
    ///
    /// The panel floats over the page, so the page's colour is what makes it
    /// read as part of the same surface. Only when the host declares nothing
    /// does a system colour make sense.
    static func overflowPanelColor(appId: String?) -> PlatformColor {
        let dark = resolvedDarkAppearance(appId: appId)
        if let declared = declaredPageColor(dark: dark) {
            return declared
        }
        #if os(macOS)
        return NSColor.windowBackgroundColor
        #else
        return UIColor.secondarySystemBackground
        #endif
    }

    /// Enable WebView debugging globally
    static func enableDebugging() {
        debuggingEnabled = true
    }
}

#if os(iOS)
typealias PlatformView = UIView
#else
typealias PlatformView = NSView
#endif
