import UIKit
import WebKit
import Foundation
import os.log
import CLingXiaFFI

// Global log instance for WebView-related logging (not bound to MainActor)
private let webViewLog = OSLog(subsystem: "LingXia", category: "WebView")

/// Data structure representing a web resource response
public struct WebResourceResponseData {
    let mimeType: String
    let encoding: String
    let statusCode: Int
    let reasonPhrase: String
    let responseHeaders: [String: String]
    let data: Data?

    public init(mimeType: String, encoding: String, statusCode: Int, reasonPhrase: String, responseHeaders: [String: String], data: Data?) {
        self.mimeType = mimeType
        self.encoding = encoding
        self.statusCode = statusCode
        self.reasonPhrase = reasonPhrase
        self.responseHeaders = responseHeaders
        self.data = data
    }
}

/// Configuration structure for WebView instances
public struct WebViewConfig {
    let enableJavaScript: Bool
    let enableDomStorage: Bool

    public init(enableJavaScript: Bool = true, enableDomStorage: Bool = false) {
        self.enableJavaScript = enableJavaScript
        self.enableDomStorage = enableDomStorage
    }
}

/// Enhanced WebView component for mini apps based on WKWebView
///
/// Features:
/// - JavaScript message channel communication
/// - Page lifecycle management (start, finish, show events)
/// - Scroll event handling with throttling
/// - Pause/resume functionality for performance optimization
/// - Automatic registration with native layer
/// - Thread-safe creation and management
///
/// Usage:
/// ```swift
/// let webView = try LingXiaWebView.createWebView(appId: "myapp", path: "index.html")
/// webView.resumeWebView()
/// ```
///
/// - Note: WebView instances are managed by Rust layer through direct pointer references
public class LingXiaWebView: WKWebView {
    internal var appId: String?
    internal var currentPath: String?
    internal var isRegistered = false
    private var isFirstLoad = true
    internal var pageLoaded = false
    private var savedScrollX: CGFloat = 0
    private var savedScrollY: CGFloat = 0
    private var savedScale: CGFloat = 1.0
    private var savedUrl: String?
    internal var showEventSent = false  // Made internal for MiniAppViewController access
    private var messageChannel: WKScriptMessageHandler?
    internal var channelInitialized = false  // Made internal for MiniAppViewController access
    private var processingRequest = false  // Prevent infinite loops

    private var lastScrollX: CGFloat = 0
    private var lastScrollY: CGFloat = 0
    private var scrollEventThrottleMs: TimeInterval = 0.1
    private var lastScrollEventTime: TimeInterval = 0
    private var scrollEventEnabled: Bool = false

    private let config: WebViewConfig

    /// Applies screen-sized layout to a view and its container
    /// - Parameters:
    ///   - view: The view to apply layout to
    ///   - container: Optional container view to also apply layout to
    public static func applyScreenLayout(view: UIView, container: UIView? = nil) {
        let screenBounds = UIScreen.main.bounds
        applyLayout(view: view, width: screenBounds.width, height: screenBounds.height, container: container)
    }

    /// Applies custom-sized layout to a view and its container
    /// - Parameters:
    ///   - view: The view to apply layout to
    ///   - width: The width to apply
    ///   - height: The height to apply
    ///   - container: Optional container view to also apply layout to
    public static func applyLayout(view: UIView, width: CGFloat, height: CGFloat, container: UIView? = nil) {
        let frame = CGRect(x: 0, y: 0, width: width, height: height)

        container?.frame = frame
        view.frame = frame

        os_log("Applied layout: %fx%f", log: webViewLog, type: .info, width, height)
    }

    /// Creates a new WebView instance with the specified parameters
    ///
    /// This is the primary API for creating WebView instances. The method ensures
    /// thread safety by executing on the main thread if called from a background thread.
    ///
    /// **Thread Safety**: This method MUST be called on the main thread or will automatically
    /// dispatch to main thread. WKWebView creation requires main thread execution.
    ///
    /// - Parameters:
    ///   - appId: The mini app identifier
    ///   - path: The page path within the mini app
    ///   - enableJavaScript: Whether to enable JavaScript execution (default: true)
    ///   - enableDomStorage: Whether to enable DOM storage (default: false)
    /// - Returns: A configured LingXiaWebView instance
    /// - Throws: Error if WebView creation fails
    /// - Note: Created WebViews are initially hidden and should be made visible when needed
    /// - Warning: WebView creation is a heavy operation and should be done sparingly
    @MainActor
    public static func createWebView(
        appId: String,
        path: String,
        enableJavaScript: Bool = true,
        enableDomStorage: Bool = false
    ) throws -> LingXiaWebView {
        // CRITICAL: WebView creation must happen on main thread
        // This is now enforced by @MainActor
        return try createWebViewOnMainThread(
            appId: appId,
            path: path,
            enableJavaScript: enableJavaScript,
            enableDomStorage: enableDomStorage
        )
    }

    /// Internal method to create WebView on main thread
    /// - Warning: This method assumes it's being called on the main thread
    internal static func createWebViewOnMainThread(
        appId: String,
        path: String,
        enableJavaScript: Bool,
        enableDomStorage: Bool
    ) throws -> LingXiaWebView {
        assert(Thread.isMainThread, "createWebViewOnMainThread must be called on main thread")

        let config = WebViewConfig(enableJavaScript: enableJavaScript, enableDomStorage: enableDomStorage)

        // Create WebView with pre-configured scheme handler
        let webView = LingXiaWebView(config: config, appId: appId)

        // Set appId and path
        webView.appId = appId
        webView.currentPath = path

        // All WebViews are created as invisible by default
        // Visibility will be controlled by Rust layer
        webView.isHidden = true

        return webView
    }

    public init(config: WebViewConfig = WebViewConfig()) {
        self.config = config

        let configuration = WKWebViewConfiguration()

        // Optimize for faster loading
        configuration.allowsInlineMediaPlayback = true
        configuration.mediaTypesRequiringUserActionForPlayback = []
        if #available(iOS 14.0, *) {
            configuration.limitsNavigationsToAppBoundDomains = false
        }

        // Enable faster networking
        configuration.upgradeKnownHostsToHTTPS = false

        super.init(frame: .zero, configuration: configuration)

        os_log("WebView initialized with frame: %{public}@", log: webViewLog, type: .info, String(describing: frame))

        // CRITICAL: Force transparent background at all levels
        backgroundColor = UIColor.clear
        isOpaque = false

        // Force the layer to be transparent as well
        layer.backgroundColor = UIColor.clear.cgColor
        layer.isOpaque = false

        // Disable any default background drawing
        layer.masksToBounds = false

        initializeWebView()
    }

    /// Initialize WebView with appId for scheme handler setup
    public init(config: WebViewConfig = WebViewConfig(), appId: String) {
        self.config = config

        let configuration = WKWebViewConfiguration()

        // Optimize for faster loading
        configuration.allowsInlineMediaPlayback = true
        configuration.mediaTypesRequiringUserActionForPlayback = []
        if #available(iOS 14.0, *) {
            configuration.limitsNavigationsToAppBoundDomains = false
        }

        // Enable faster networking
        configuration.upgradeKnownHostsToHTTPS = false

        // CRITICAL: Set up scheme handler BEFORE creating WebView
        let schemeHandler = SchemeHandler(appId: appId)
        configuration.setURLSchemeHandler(schemeHandler, forURLScheme: "lx")

        super.init(frame: .zero, configuration: configuration)

        os_log("WebView initialized with frame: %{public}@ and lx:// scheme handler for appId=%@", log: webViewLog, type: .info, String(describing: frame), appId)

        // CRITICAL: Force transparent background at all levels
        backgroundColor = UIColor.clear
        isOpaque = false

        // Force the layer to be transparent as well
        layer.backgroundColor = UIColor.clear.cgColor
        layer.isOpaque = false

        // Disable any default background drawing
        layer.masksToBounds = false

        initializeWebView()
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func initializeWebView() {
        // Basic initialization without appId-dependent setup
        applyWebViewSettings()
        setupWebViewDelegates()
        setupConsoleInterceptor()
    }

    /// Setup JavaScript console interceptor
    private func setupConsoleInterceptor() {
        let consoleScript = """
        (function() {
            const originalLog = console.log;
            const originalError = console.error;
            const originalWarn = console.warn;
            const originalInfo = console.info;

            function sendConsoleMessage(level, args) {
                const message = args.map(arg =>
                    typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
                ).join(' ');

                try {
                    // Use dedicated console handler
                    window.webkit.messageHandlers.LingXiaConsole.postMessage({
                        level: level,
                        message: message
                    });
                } catch (e) {
                    // Fallback if message handler not ready
                }
            }

            console.log = function(...args) {
                sendConsoleMessage('log', args);
                originalLog.apply(console, args);
            };

            console.error = function(...args) {
                sendConsoleMessage('error', args);
                originalError.apply(console, args);
            };

            console.warn = function(...args) {
                sendConsoleMessage('warn', args);
                originalWarn.apply(console, args);
            };

            console.info = function(...args) {
                sendConsoleMessage('info', args);
                originalInfo.apply(console, args);
            };
        })();
        """

        let userScript = WKUserScript(source: consoleScript, injectionTime: .atDocumentStart, forMainFrameOnly: false)
        configuration.userContentController.addUserScript(userScript)
    }

    private func applyWebViewSettings() {
        // Configure WebKit settings
        configuration.defaultWebpagePreferences.allowsContentJavaScript = config.enableJavaScript

        // Handle DOM storage control for mini-app architecture
        if !config.enableDomStorage {
            // Use non-persistent data store to prevent storage persistence
            configuration.websiteDataStore = WKWebsiteDataStore.nonPersistent()
        }

        allowsBackForwardNavigationGestures = true
        scrollView.bounces = true
        scrollView.showsVerticalScrollIndicator = true
        scrollView.showsHorizontalScrollIndicator = true

        // Optimize for faster rendering
        isInspectable = false // Disable web inspector for performance

        // CRITICAL: Force transparent background at all WebView levels
        scrollView.backgroundColor = UIColor.clear
        scrollView.isOpaque = false
        scrollView.layer.backgroundColor = UIColor.clear.cgColor
        scrollView.layer.isOpaque = false

        underPageBackgroundColor = UIColor.clear

        // Force system background to be clear
        overrideUserInterfaceStyle = .unspecified

        // Default to edge-to-edge content behavior (will be managed by MiniAppViewController)
        scrollView.contentInsetAdjustmentBehavior = .never

    }

    private func setupWebViewDelegates() {
        navigationDelegate = self
        uiDelegate = self

        // Setup scroll view delegate for scroll events
        scrollView.delegate = self
    }

    private func handleScrollChange(scrollX: CGFloat, scrollY: CGFloat, oldScrollX: CGFloat, oldScrollY: CGFloat) {
        // Only send scroll events if enabled by Rust layer
        guard scrollEventEnabled else {
            return
        }

        // Throttle scroll events to avoid excessive native calls
        let currentTime = Date().timeIntervalSince1970
        if currentTime - lastScrollEventTime < scrollEventThrottleMs {
            return
        }
        lastScrollEventTime = currentTime

        // Only send scroll events if WebView is properly initialized and visible
        if let appId = appId, let currentPath = currentPath, pageLoaded && !isHidden {
            // Calculate scroll range
            let maxScrollX = scrollView.contentSize.width - scrollView.frame.width
            let maxScrollY = scrollView.contentSize.height - scrollView.frame.height

            // Send scroll event to native layer
            let _ = lingxia.onScrollChanged(
                appId,
                currentPath,
                Int32(scrollX),
                Int32(scrollY),
                Int32(maxScrollX),
                Int32(maxScrollY)
            )
        }
    }

    /// Ensures the bridge.js compatibility is set up
    /// This method sets up the message handler for bridge.js to communicate with native layer
    /// - Note: Called from WebView lifecycle methods which are already on main thread
    public func ensureBridgeReady() {
        // CRITICAL FIX: Only reset bridge if it's actually broken or not initialized
        // For reused WebViews that are working, don't break the existing bridge
        if channelInitialized && messageChannel != nil {
            return
        }

        os_log("ensureBridgeReady: Setting up bridge for appId=%@ path=%@ (channelInitialized=%@)",
               log: webViewLog, type: .info, appId ?? "nil", currentPath ?? "nil", String(channelInitialized))

        // Clean up existing channels if any - use try-catch to handle cases where handlers don't exist
        if messageChannel != nil {
            do {
                configuration.userContentController.removeScriptMessageHandler(forName: "LingXia")
                os_log("ensureBridgeReady: Removed existing LingXia handler", log: webViewLog, type: .info)
            } catch {
                // Handler doesn't exist, ignore
            }

            do {
                configuration.userContentController.removeScriptMessageHandler(forName: "LingXiaConsole")
                os_log("ensureBridgeReady: Removed existing LingXiaConsole handler", log: webViewLog, type: .info)
            } catch {
                // Handler doesn't exist, ignore
            }
        }

        messageChannel = nil
        channelInitialized = false

        // Setup main message handler for bridge.js communication
        let messageHandler = WebViewMessageHandler { [weak self] message in
            guard let self = self else { return }

            // Check for LXPortRdy event to mark channel as initialized
            if !self.channelInitialized && message.contains("\"name\":\"LXPortRdy\"") && message.contains("\"type\":\"event\"") {
                os_log("LXPortRdy event detected, message channel is ready", log: webViewLog, type: .info)
                self.channelInitialized = true
            }

            // Forward message to native layer
            guard let appId = self.appId, let currentPath = self.currentPath else {
                os_log("WebViewMessageHandler: Missing appId or currentPath, cannot forward message", log: webViewLog, type: .error)
                return
            }

            let _ = lingxia.handlePostMessage(appId, currentPath, message)
        }

        // Setup dedicated console message handler
        let consoleHandler = WebViewMessageHandler { [weak self] message in
            guard let self = self else { return }
            self.handleConsoleMessage(message)
        }

        messageChannel = messageHandler
        configuration.userContentController.add(messageHandler, name: "LingXia")
        configuration.userContentController.add(consoleHandler, name: "LingXiaConsole")

        os_log("WebView bridge setup for appId=%@ path=%@ (waiting for LXPortRdy)", log: webViewLog, type: .info,
               appId ?? "nil", currentPath ?? "nil")
    }

    /// Handle console messages from JavaScript
    private func handleConsoleMessage(_ message: String) {
        guard let appId = appId, let currentPath = currentPath else { return }

        // Parse console message JSON (simplified format from dedicated handler)
        guard let data = message.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let level = json["level"] as? String,
              let consoleMessage = json["message"] as? String else {
            os_log("Failed to parse console message: %{public}@", log: webViewLog, type: .error, message)
            return
        }

        // Map console levels to log levels
        let logLevel: Int32
        let osLogType: OSLogType
        switch level {
        case "error":
            logLevel = 6 // ERROR
            osLogType = .error
        case "warn":
            logLevel = 5 // WARN
            osLogType = .default
        case "info":
            logLevel = 4 // INFO
            osLogType = .info
        case "log":
            logLevel = 4 // INFO
            osLogType = .info
        default:
            logLevel = 4 // INFO
            osLogType = .info
        }

        // Forward to Rust
        let _ = lingxia.consoleMessage(appId, currentPath, logLevel, consoleMessage)
    }



    /// Clears all browsing data for this WebView
    ///
    /// This includes cookies, cache, local storage, and other website data.
    /// The operation is asynchronous and completion is logged.
    public func clearBrowsingData() {
        let dataStore = configuration.websiteDataStore
        let dataTypes = WKWebsiteDataStore.allWebsiteDataTypes()

        dataStore.removeData(ofTypes: dataTypes, modifiedSince: Date.distantPast) {
            // Data cleared
        }
    }

    /// Resets the WebView's viewport to default zoom and scroll position
    ///
    /// This method restores the WebView to its initial zoom level (1.0) and
    /// scrolls to the top-left corner.
    public func resetViewport() {
        // Reset zoom and scroll position
        scrollView.setZoomScale(1.0, animated: false)
        scrollView.setContentOffset(.zero, animated: false)
    }

    /// Pauses WebView operations and saves current state
    ///
    /// This method saves the current scroll position, zoom level, and URL.
    /// Call resumeWebView() to restore the state.
    ///
    /// - Note: This is useful for performance optimization when the WebView is not visible
    public func pauseWebView() {
        if pageLoaded {
            savedScrollX = scrollView.contentOffset.x
            savedScrollY = scrollView.contentOffset.y
            savedScale = scrollView.zoomScale
            savedUrl = url?.absoluteString
            showEventSent = false  // Reset the flag when paused
        }

        // Hide the WebView to save resources
        isHidden = true
    }

    /// Resumes WebView operations and restores saved state WITHOUT reloading URL
    ///
    /// This method restores the WebView to its previous state, including scroll position,
    /// zoom level, and bridge connectivity, but does NOT reload the URL. This is useful
    /// for reusing existing WebViews without triggering content reload.
    ///
    /// - Note: The WebView becomes visible after calling this method
    /// - Warning: Must be called on the main thread
    public func resumeWebViewWithoutReload() {
        // Ensure we're on the main thread for UI operations
        guard Thread.isMainThread else {
            os_log("resumeWebViewWithoutReload called from background thread, dispatching to main", log: webViewLog, type: .debug)
            DispatchQueue.main.async { [weak self] in
                self?.resumeWebViewWithoutReloadOnMainThread()
            }
            return
        }

        resumeWebViewWithoutReloadOnMainThread()
    }

    /// Internal method to resume WebView on main thread without reloading URL
    /// - Warning: This method assumes it's being called on the main thread
    private func resumeWebViewWithoutReloadOnMainThread() {
        assert(Thread.isMainThread, "resumeWebViewWithoutReloadOnMainThread must be called on main thread")

        // Set to visible
        isHidden = false

        // Ensure bridge is working when resuming
        if !channelInitialized || messageChannel == nil {
            ensureBridgeReady()
        }

        // Restore scroll position and scale if page was already loaded
        if pageLoaded {
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.scrollView.setContentOffset(CGPoint(x: self.savedScrollX, y: self.savedScrollY), animated: false)
                self.scrollView.setZoomScale(self.savedScale, animated: false)
                os_log("resumeWebViewWithoutReload: Restored scroll position and zoom", log: webViewLog, type: .info)
            }
        }

        os_log("resumeWebViewWithoutReload: WebView resumed without URL reload", log: webViewLog, type: .info)
    }

    /// Resumes WebView operations and restores saved state
    ///
    /// This method restores the WebView to its previous state, including scroll position,
    /// zoom level, and URL. It also re-establishes the message channel if needed and
    /// triggers appropriate page lifecycle events.
    ///
    /// - Note: The WebView becomes visible after calling this method
    /// - Warning: Must be called on the main thread
    public func resumeWebView() {
        // Ensure we're on the main thread for UI operations
        guard Thread.isMainThread else {
            os_log("resumeWebView called from background thread, dispatching to main", log: webViewLog, type: .debug)
            DispatchQueue.main.async { [weak self] in
                self?.resumeWebViewOnMainThread()
            }
            return
        }

        resumeWebViewOnMainThread()
    }

    /// Internal method to resume WebView on main thread
    /// - Warning: This method assumes it's being called on the main thread
    private func resumeWebViewOnMainThread() {
        assert(Thread.isMainThread, "resumeWebViewOnMainThread must be called on main thread")

        // Set to visible
        isHidden = false

        // Ensure bridge is working when resuming
        if !channelInitialized || messageChannel == nil {
            ensureBridgeReady()
        }

        // Only trigger PageShow if we haven't already in this session
        if let appId = appId, let currentPath = currentPath, !showEventSent {
            if !isFirstLoad && pageLoaded {
                // Page already loaded, restore scroll position and scale
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.scrollView.setContentOffset(CGPoint(x: self.savedScrollX, y: self.savedScrollY), animated: false)
                    self.scrollView.setZoomScale(self.savedScale, animated: false)

                    // Only reload URL if needed
                    if let savedUrl = self.savedUrl, self.url?.absoluteString != savedUrl {
                        if let url = URL(string: savedUrl) {
                            let _ = self.load(URLRequest(url: url))
                        }
                    } else {
                        // If we're resuming an already loaded page, trigger PageShow
                        // This matches Android's behavior in WebView.kt line 525
                        if !self.showEventSent {
                            lingxia.onPageShow(appId, currentPath)
                            self.showEventSent = true  // Mark that we've sent the event
                        }
                    }
                }
            } else if isFirstLoad {
                // First load, PageShow will be triggered in navigation delegate
            }
        }
    }

    public override func willMove(toSuperview newSuperview: UIView?) {
        super.willMove(toSuperview: newSuperview)

        if newSuperview != nil {
            // WebView is being attached to superview
            // Note: onWebviewAttached will be called by MiniAppViewController after proper layout
            os_log("WebView attached to superview for appId=%@ path=%@", log: webViewLog, type: .debug, appId ?? "nil", currentPath ?? "nil")
        } else {
            // Clean up resources when being removed from superview
            if messageChannel != nil {
                do {
                    configuration.userContentController.removeScriptMessageHandler(forName: "LingXia")
                } catch {
                    // Handler doesn't exist, ignore
                }

                do {
                    configuration.userContentController.removeScriptMessageHandler(forName: "LingXiaConsole")
                } catch {
                    // Handler doesn't exist, ignore
                }

                messageChannel = nil
            }
            channelInitialized = false  // Reset the flag when detached
            pauseWebView()
            isRegistered = false  // Reset registration status when detached
        }
    }

    public func setUserAgent(_ userAgent: String) {
        customUserAgent = userAgent
    }

    public override func load(_ request: URLRequest) -> WKNavigation? {

        savedUrl = request.url?.absoluteString
        resetViewport()
        isHidden = false
        return super.load(request)
    }

    public func getPageConfig() -> NavigationBarConfig? {
        guard let appId = appId, let currentPath = currentPath else { return nil }
        let configJson = lingxia.getPageConfig(appId, currentPath)?.toString()
        return NavigationBarConfig.fromJson(configJson)
    }

    /**
     * Enable or disable scroll event listener with optional throttle time.
     * This is called by Rust layer to control when scroll events should be fired.
     */
    public func setScrollListenerEnabled(enabled: Bool, throttleMs: TimeInterval = 0.1) {
        // Ensure thread safety for UI operations
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }

            // Set throttle time first (with validation)
            self.scrollEventThrottleMs = max(0.016, throttleMs) // Minimum 16ms for 60fps
            self.scrollEventEnabled = enabled

            os_log("WebView.setScrollListenerEnabled: enabled=%@ throttleMs=%.3f",
                   log: webViewLog, type: .info, String(enabled), throttleMs)

            if enabled {
                // Enable scroll delegate if not already set
                if self.scrollView.delegate == nil {
                    self.scrollView.delegate = self
                }
                self.lastScrollEventTime = 0 // Reset throttle timer
            }
        }
    }



    private func handlePageFinished(url: String?) {
        guard let appId = appId, let currentPath = currentPath else { return }

        let _ = lingxia.onPageFinished(appId, currentPath)

        // If page is loaded and attached to superview, and we haven't sent PageShow yet
        // This matches Android's behavior in WebView.kt line 706
        if superview != nil && url != nil && !showEventSent {
            lingxia.onPageShow(appId, currentPath)
            showEventSent = true
        }
    }

    /// Helper function to get WebView from pointer
    nonisolated private static func getWebView(from pointer: UInt) -> LingXiaWebView? {
        guard pointer != 0 else {
            os_log("LingXiaWebView.getWebView: Invalid pointer (0)", log: webViewLog, type: .error)
            return nil
        }

        let webView = Unmanaged<LingXiaWebView>.fromOpaque(UnsafeRawPointer(bitPattern: pointer)!).takeUnretainedValue()
        return webView
    }

    /// Creates a WebView and returns its pointer as UInt for Rust to hold
    /// This function is called by Rust through swift-bridge
    nonisolated public static func createWebViewPtr(appid: RustStr, path: RustStr) -> UInt {
        // Convert RustStr to String
        let appidString = appid.toString()
        let pathString = path.toString()

        // Use DispatchQueue.main.sync to ensure we're on main thread
        // This works whether we're already on main thread or not
        var result: UInt = 0
        var webView: LingXiaWebView?

        // Use MainActor.assumeIsolated since we know we're on main thread
        // This allows us to call @MainActor methods synchronously
        if Thread.isMainThread {
            do {
                webView = try MainActor.assumeIsolated {
                    try LingXiaWebView.createWebView(appId: appidString, path: pathString)
                }
                if let webView = webView {
                    let pointer = Unmanaged.passRetained(webView).toOpaque()
                    result = UInt(bitPattern: pointer)
                    os_log("LingXiaWebView.createWebViewPtr: WebView created successfully, pointer=%{public}@", log: webViewLog, type: .info, String(describing: result))
                }
            } catch {
                os_log("LingXiaWebView.createWebViewPtr: Failed to create WebView: %@", log: webViewLog, type: .error, error.localizedDescription)
                result = 0
            }
        } else {
            // Not on main thread, use sync dispatch
            DispatchQueue.main.sync {
                do {
                    webView = try MainActor.assumeIsolated {
                        try LingXiaWebView.createWebView(appId: appidString, path: pathString)
                    }
                    if let webView = webView {
                        let pointer = Unmanaged.passRetained(webView).toOpaque()
                        result = UInt(bitPattern: pointer)
                        os_log("LingXiaWebView.createWebViewPtr: WebView created successfully via sync dispatch, pointer=%{public}@", log: webViewLog, type: .info, String(describing: result))
                    }
                } catch {
                    os_log("LingXiaWebView.createWebViewPtr: Failed to create WebView via sync dispatch: %@", log: webViewLog, type: .error, error.localizedDescription)
                    result = 0
                }
            }
        }

        // Send notification that WebView was created (instead of immediately triggering attached)
        // This allows UI layer to attach WebView properly before triggering attached event
        if let _ = webView, result != 0 {
            DispatchQueue.main.async {
                NotificationCenter.default.post(
                    name: NSNotification.Name("WebViewCreated"),
                    object: nil,
                    userInfo: [
                        "appId": appidString,
                        "path": pathString,
                        "webViewPtr": result
                    ]
                )
                os_log("LingXiaWebView.createWebViewPtr: Posted WebViewCreated notification for appId=%{public}@, path=%{public}@", log: webViewLog, type: .info, appidString, pathString)
            }
        }

        return result
    }

    /// Load URL in WebView - static method for swift-bridge
    nonisolated public static func loadUrl(webview_ptr: UInt, url: RustStr) -> Bool {
        let urlString = url.toString()
        os_log("LingXiaWebView.loadUrl: ptr=%{public}@, url=%{public}@", log: webViewLog, type: .info, String(webview_ptr), urlString)

        guard let webView = getWebView(from: webview_ptr) else {
            return false
        }

        guard let url = URL(string: urlString) else {
            return false
        }

        DispatchQueue.main.async {
            let request = URLRequest(url: url)
            let _ = webView.load(request)
        }

        return true
    }

    /// Evaluate JavaScript in WebView - static method for swift-bridge
    nonisolated public static func evaluateJavaScript(webview_ptr: UInt, js: RustStr) -> Bool {
        let jsString = js.toString()

        os_log("LingXiaWebView.evaluateJavaScript: ptr=%{public}@", log: webViewLog, type: .info, String(webview_ptr))

        guard let webView = getWebView(from: webview_ptr) else {
            return false
        }

        DispatchQueue.main.async {
            webView.evaluateJavaScript(jsString) { result, error in
                if let error = error {
                    os_log("LingXiaWebView.evaluateJavaScript: Error: %{public}@", log: webViewLog, type: .error, error.localizedDescription)
                }
            }
        }

        return true
    }

    /// Clear browsing data in WebView - static method for swift-bridge
    nonisolated public static func clearBrowsingData(webview_ptr: UInt) -> Bool {
        guard let webView = getWebView(from: webview_ptr) else {
            return false
        }

        // Always dispatch to main thread for WebKit operations
        DispatchQueue.main.async {
            let dataStore = webView.configuration.websiteDataStore
            let dataTypes = WKWebsiteDataStore.allWebsiteDataTypes()
            dataStore.removeData(ofTypes: dataTypes, modifiedSince: Date.distantPast) {
                os_log("LingXiaWebView.clearBrowsingData: Browsing data cleared successfully", log: webViewLog, type: .info)
            }
        }

        return true
    }

    /// Set devtools enabled/disabled in WebView - static method for swift-bridge
    nonisolated public static func setDevtools(webview_ptr: UInt, enabled: Bool) -> Bool {

        guard let webView = getWebView(from: webview_ptr) else {
            return false
        }

        // Always dispatch to main thread for WebKit operations
        DispatchQueue.main.async {
            if #available(iOS 16.4, *) {
                webView.isInspectable = enabled
                os_log("LingXiaWebView.setDevtools: Devtools %{public}@ successfully", log: webViewLog, type: .info, enabled ? "enabled" : "disabled")
            }
        }

        return true
    }

    /// Set user agent in WebView - static method for swift-bridge
    nonisolated public static func setUserAgent(webview_ptr: UInt, ua: RustStr) -> Bool {
        let userAgent = ua.toString()

        guard let webView = getWebView(from: webview_ptr) else {
            return false
        }

        // Always dispatch to main thread for WebKit operations
        DispatchQueue.main.async {
            webView.customUserAgent = userAgent.isEmpty ? nil : userAgent
            os_log("setUserAgent: User agent set successfully", log: webViewLog, type: .info)
        }

        return true
    }

    /// Set scroll listener enabled/disabled in WebView - static method for swift-bridge
    nonisolated public static func setScrollListenerEnabled(webview_ptr: UInt, enabled: Bool, throttle_ms: UInt64) -> Bool {
        guard let webView = getWebView(from: webview_ptr) else {
            return false
        }

        // Always dispatch to main thread for WebKit operations
        DispatchQueue.main.async {
            webView.setScrollListenerEnabled(enabled: enabled, throttleMs: TimeInterval(throttle_ms) / 1000.0)
            os_log("setScrollListenerEnabled: Scroll listener %{public}@ successfully", log: webViewLog, type: .info, enabled ? "enabled" : "disabled")
        }

        return true
    }
}

// MARK: - WKNavigationDelegate
extension LingXiaWebView: WKNavigationDelegate {
    public func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {

        pageLoaded = false

        if let appId = appId, let currentPath = currentPath {
            let _ = lingxia.onPageStarted(appId, currentPath)
        }
    }

    public func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        let finalURL = webView.url?.absoluteString ?? "nil"
        os_log("WebView didFinish navigation: %{public}@", log: webViewLog, type: .info, finalURL)

        // Record that the page has finished loading
        pageLoaded = true

        // Update isFirstLoad flag after the first load completes
        if isFirstLoad {
            isFirstLoad = false
        }

        resetViewport()  // Reset viewport after page load

        // Setup bridge after page is fully loaded
        if !channelInitialized && superview != nil {
            ensureBridgeReady()
        }

        handlePageFinished(url: webView.url?.absoluteString)
    }

    public func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        os_log("WebView didFail navigation with error: %{public}@", log: webViewLog, type: .error, error.localizedDescription)
    }

    public func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        os_log("WebView didFailProvisionalNavigation with error: %{public}@", log: webViewLog, type: .error, error.localizedDescription)
    }

    public func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
        guard let url = navigationAction.request.url else {
            os_log("decidePolicyFor: URL is nil, allowing", log: webViewLog, type: .info)
            decisionHandler(.allow)
            return
        }

        // Skip interception for data URLs (generated by loadHTMLString)
        if url.scheme == "data" {
            os_log("Allowing data URL navigation (from loadHTMLString): %{public}@", log: webViewLog, type: .info, String(url.absoluteString.prefix(100)))
            decisionHandler(.allow)
            return
        }

        // Skip interception for about:blank and other special URLs
        if url.scheme == "about" || url.absoluteString == "about:blank" {
            os_log("Allowing about: URL navigation: %{public}@", log: webViewLog, type: .info, url.absoluteString)
            decisionHandler(.allow)
            return
        }

        // Prevent infinite loops
        if processingRequest {
            os_log("Already processing a request, allowing navigation to prevent loop", log: webViewLog, type: .info)
            decisionHandler(.allow)
            return
        }

        // For HTTPS and LX requests, check if we should intercept them
        if url.scheme == "https" || url.scheme == "lx" {
            processingRequest = true  // Set flag to prevent loops
            let request = navigationAction.request

            // Convert to HttpRequest for Rust processing
            if let httpRequest = convertToHttpRequest(request),
               let appId = appId {

                // Call Rust to check if we should handle this request
                if let httpResponse = appId.toRustStr({ appidRustStr in
                    lingxia.handleRequest(appidRustStr, httpRequest)
                }) {
                    // If Rust handled it, cancel the navigation
                    decisionHandler(.cancel)

                    // Reset processing flag after a delay to allow HTML injection to complete
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                        self.processingRequest = false
                    }

                    // Check if this is a blocked request (403 status)
                    if httpResponse.status_code == 403 {
                        os_log("Request blocked by Rust: %@", log: webViewLog, type: .default, url.absoluteString)
                        // Could show an error page or just silently block
                        return
                    }

                    // For successful responses, inject the content into WebView
                    if httpResponse.status_code == 200 {
                        let bodyData = Data(Array(httpResponse.body))
                        if let htmlString = String(data: bodyData, encoding: .utf8) {
                            // os_log("HTML content decoded successfully, length: %d chars", log: webViewLog, type: .info, htmlString.count)
                            // os_log("HTML preview: %{public}@", log: webViewLog, type: .debug, String(htmlString.prefix(200)))

                            // Inject the HTML content into WebView
                            DispatchQueue.main.async { [weak self] in
                                guard let self = self else { return }

                                // Load the actual Rust HTML content
                                let navigation = self.loadHTMLString(htmlString, baseURL: url)
                                os_log("loadHTMLString returned navigation: %{public}@", log: webViewLog, type: .info, navigation?.description ?? "nil")
                            }
                        } else {
                            os_log("Request handled by Rust but failed to decode HTML content as UTF-8", log: webViewLog, type: .error)
                            // Try to decode as raw bytes for debugging
                            let rawString = bodyData.map { String(format: "%02x", $0) }.joined(separator: " ")
                            os_log("Raw body bytes (first 100): %{public}@", log: webViewLog, type: .debug, String(rawString.prefix(200)))
                        }
                    }
                    return
                } else {
                    // Rust didn't handle the request
                    os_log("Rust did not handle request: %@, allowing normal navigation", log: webViewLog, type: .info, url.absoluteString)
                    processingRequest = false  // Reset flag
                }
            }
        }

        // Default URL override check (existing logic)
        if let appId = appId {
            let result = lingxia.shouldOverrideUrlLoading(appId, url.absoluteString)
            if result {
                decisionHandler(.cancel)
                return
            }
        }

        decisionHandler(.allow)
    }

    private func convertToHttpRequest(_ request: URLRequest) -> HttpRequest? {
        guard let url = request.url?.absoluteString else { return nil }

        let method = request.httpMethod ?? "GET"

        // Convert headers to RustVec efficiently
        var headerKeys: [RustString] = []
        var headerValues: [RustString] = []
        if let requestHeaders = request.allHTTPHeaderFields {
            for (key, value) in requestHeaders {
                headerKeys.append(RustString(key))
                headerValues.append(RustString(value))
            }
        }

        // Get request body
        let body = request.httpBody ?? Data()

        // Create high-efficiency HttpRequest struct
        let headerKeysVec = RustVec<RustString>()
        for key in headerKeys {
            headerKeysVec.push(value: key)
        }

        let headerValuesVec = RustVec<RustString>()
        for value in headerValues {
            headerValuesVec.push(value: value)
        }

        let bodyVec = RustVec<UInt8>()
        for byte in body {
            bodyVec.push(value: byte)
        }

        return HttpRequest(
            url: RustString(url),
            method: RustString(method),
            header_keys: headerKeysVec,
            header_values: headerValuesVec,
            body: bodyVec
        )
    }


}

// MARK: - WKUIDelegate
extension LingXiaWebView: WKUIDelegate {
    // WKUIDelegate methods can be added here if needed for alert/confirm/prompt handling
}

// MARK: - UIScrollViewDelegate
extension LingXiaWebView: UIScrollViewDelegate {
    public func scrollViewDidScroll(_ scrollView: UIScrollView) {
        let scrollX = scrollView.contentOffset.x
        let scrollY = scrollView.contentOffset.y

        handleScrollChange(scrollX: scrollX, scrollY: scrollY, oldScrollX: lastScrollX, oldScrollY: lastScrollY)

        lastScrollX = scrollX
        lastScrollY = scrollY
    }
}

// MARK: - Message Handler
private class WebViewMessageHandler: NSObject, WKScriptMessageHandler {
    private let messageHandler: (String) -> Void

    init(messageHandler: @escaping (String) -> Void) {
        self.messageHandler = messageHandler
        super.init()
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        var messageString: String?

        if let stringBody = message.body as? String {
            // Already a string
            messageString = stringBody
            // os_log("WebViewMessageHandler: Received string message from JavaScript", log: webViewLog, type: .info)
        } else if let dictBody = message.body as? [String: Any] {
            // Convert NSDictionary to JSON string
            do {
                let jsonData = try JSONSerialization.data(withJSONObject: dictBody, options: [])
                messageString = String(data: jsonData, encoding: .utf8)
                // os_log("WebViewMessageHandler: Converted NSDictionary to JSON string", log: webViewLog, type: .info)
            } catch {
                os_log("WebViewMessageHandler: Failed to convert NSDictionary to JSON: %{public}@", log: webViewLog, type: .error, error.localizedDescription)
                return
            }
        } else {
            os_log("WebViewMessageHandler: Received unsupported message type: %{public}@", log: webViewLog, type: .error, String(describing: type(of: message.body)))
            return
        }

        guard let finalMessageString = messageString else {
            os_log("WebViewMessageHandler: Failed to get message string", log: webViewLog, type: .error)
            return
        }

        // os_log("WebViewMessageHandler: Processing message: %{public}@", log: webViewLog, type: .info, String(finalMessageString.prefix(200)))
        messageHandler(finalMessageString)
    }
}
