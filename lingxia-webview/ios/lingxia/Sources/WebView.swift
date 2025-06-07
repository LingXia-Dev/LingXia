import UIKit
import WebKit
import Foundation
import os.log
import CLingXiaFFI

/// Manager class for WebView instances, providing centralized storage and retrieval
@MainActor
public class WebViewManager {
    private static let log = OSLog(subsystem: "LingXia", category: "WebViewManager")

    public static let shared = WebViewManager()
    private var webViewStorage: [String: LingXiaWebView] = [:]

    private init() {}

    private func makeKey(appId: String, path: String) -> String {
        return "\(appId):\(path)"
    }

    /// Stores a WebView instance
    public func storeWebView(_ webView: LingXiaWebView, appId: String, path: String) {
        let key = makeKey(appId: appId, path: path)
        webViewStorage[key] = webView
        os_log("Stored WebView for key: %@", log: Self.log, type: .info, key)
    }

    /// Retrieves a WebView instance
    public func getWebView(appId: String, path: String) -> LingXiaWebView? {
        let key = makeKey(appId: appId, path: path)
        return webViewStorage[key]
    }

    /// Removes a WebView instance from storage
    @discardableResult
    public func removeWebView(appId: String, path: String) -> LingXiaWebView? {
        let key = makeKey(appId: appId, path: path)
        let webView = webViewStorage.removeValue(forKey: key)
        if webView != nil {
            os_log("Removed WebView for key: %@", log: Self.log, type: .info, key)
        }
        return webView
    }

    /// Removes all WebView instances for a specific app
    public func removeAllWebViews(for appId: String) -> [LingXiaWebView] {
        let keysToRemove = webViewStorage.keys.filter { $0.hasPrefix("\(appId):") }
        var removedWebViews: [LingXiaWebView] = []

        for key in keysToRemove {
            if let webView = webViewStorage.removeValue(forKey: key) {
                removedWebViews.append(webView)
            }
        }

        os_log("Removed %d WebViews for appId: %@", log: Self.log, type: .info, removedWebViews.count, appId)
        return removedWebViews
    }

    /// Gets the total number of stored WebView instances
    public func getWebViewCount() -> Int {
        return webViewStorage.count
    }
}

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
/// - Note: All WebView instances are managed centrally through WebViewManager
public class LingXiaWebView: WKWebView {
    private static let log = OSLog(subsystem: "LingXia", category: "WebView")

    internal var appId: String?
    internal var currentPath: String?
    private var isRegistered = false
    private var isFirstLoad = true
    private var pageLoaded = false
    private var savedScrollX: CGFloat = 0
    private var savedScrollY: CGFloat = 0
    private var savedScale: CGFloat = 1.0
    private var savedUrl: String?
    private var showEventSent = false
    private var messageChannel: WKScriptMessageHandler?
    private var channelInitialized = false

    private var lastScrollX: CGFloat = 0
    private var lastScrollY: CGFloat = 0
    private var scrollEventThrottleMs: TimeInterval = 0.1
    private var lastScrollEventTime: TimeInterval = 0
    private var scrollEventEnabled: Bool = false

    private let config: WebViewConfig

    private static let WEBKIT_PORT_INIT_MESSAGE_DATA = "LingXia-port-init"

    /// Finds an existing WebView instance or creates a new one if not found
    /// This method manages WebView instances in Swift until native integration is complete
    /// - Parameters:
    ///   - appId: The mini app identifier
    ///   - path: The page path
    /// - Returns: Existing or newly created WebView instance
    public static func dummyNativeFindWebView(appId: String, path: String) -> LingXiaWebView? {
        if let existingWebView = WebViewManager.shared.getWebView(appId: appId, path: path) {
            os_log("Found existing WebView for appId=%@ path=%@", log: Self.log, type: .info, appId, path)
            return existingWebView
        }

        do {
            let newWebView = try createWebView(appId: appId, path: path)
            WebViewManager.shared.storeWebView(newWebView, appId: appId, path: path)
            os_log("Created and stored new WebView for appId=%@ path=%@", log: Self.log, type: .info, appId, path)
            return newWebView
        } catch {
            os_log("Failed to create WebView for appId=%@ path=%@: %@", log: Self.log, type: .error, appId, path, error.localizedDescription)
            return nil
        }
    }

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

        os_log("Applied layout: %fx%f", log: Self.log, type: .info, width, height)
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
    public static func createWebView(
        appId: String,
        path: String,
        enableJavaScript: Bool = true,
        enableDomStorage: Bool = false
    ) throws -> LingXiaWebView {
        // CRITICAL: WebView creation must happen on main thread
        // Throw error instead of blocking to prevent deadlocks
        guard Thread.isMainThread else {
            throw NSError(
                domain: "WebViewCreation",
                code: -2,
                userInfo: [
                    NSLocalizedDescriptionKey: "WebView creation must be called on the main thread",
                    NSLocalizedRecoverySuggestionErrorKey: "Dispatch to main thread before calling createWebView"
                ]
            )
        }

        return try createWebViewOnMainThread(
            appId: appId,
            path: path,
            enableJavaScript: enableJavaScript,
            enableDomStorage: enableDomStorage
        )
    }

    /// Internal method to create WebView on main thread
    /// - Warning: This method assumes it's being called on the main thread
    private static func createWebViewOnMainThread(
        appId: String,
        path: String,
        enableJavaScript: Bool,
        enableDomStorage: Bool
    ) throws -> LingXiaWebView {
        assert(Thread.isMainThread, "createWebViewOnMainThread must be called on main thread")

        let config = WebViewConfig(enableJavaScript: enableJavaScript, enableDomStorage: enableDomStorage)
        let webView = LingXiaWebView(config: config)

        // Set appId and path directly
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
        // Add URL scheme handler for lx://
        if let appId = appId {
            let schemeHandler = SchemeHandler(appId: appId)
            configuration.setURLSchemeHandler(schemeHandler, forURLScheme: "lx")
            os_log("Registered lx:// scheme handler for appId: %@", log: Self.log, type: .info, appId)
        }

        applyWebViewSettings()
        setupWebViewDelegates()
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

            // Send scroll event to native layer (dummy implementation)
            let _ = dummyNativeOnScrollChanged(
                appId: appId,
                path: currentPath,
                scrollX: Int(scrollX),
                scrollY: Int(scrollY),
                maxScrollX: Int(maxScrollX),
                maxScrollY: Int(maxScrollY)
            )
        }
    }

    /// Ensures the bridge.js compatibility is set up
    /// This method sets up the message handler for bridge.js to communicate with native layer
    /// - Note: Called from WebView lifecycle methods which are already on main thread
    public func ensureBridgeReady() {
        // If channel is already initialized, don't recreate
        if channelInitialized && messageChannel != nil {
            return
        }

        // Clean up existing channel if any
        if messageChannel != nil {
            configuration.userContentController.removeScriptMessageHandler(forName: "LingXia")
            messageChannel = nil
        }
        channelInitialized = false

        // Setup message handler for bridge.js communication
        let messageHandler = WebViewMessageHandler { [weak self] message in
            guard let self = self else { return }

            // Forward message to native layer (dummy implementation)
            guard let appId = self.appId, let currentPath = self.currentPath else { return }
            let _ = self.dummyNativeHandlePostMessage(appId: appId, path: currentPath, message: message)
        }

        messageChannel = messageHandler
        configuration.userContentController.add(messageHandler, name: "LingXia")
        channelInitialized = true

        os_log("WebView bridge ready for appId=%@ path=%@", log: Self.log, type: .info,
               appId ?? "nil", currentPath ?? "nil")
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
            os_log("resumeWebView called from background thread, dispatching to main", log: Self.log, type: .debug)
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
        os_log("Resuming WebView operations, appId=%{public}@ path=%{public}@ isFirstLoad=%{public}@ pageLoaded=%{public}@ showEventSent=%{public}@", log: Self.log, type: .debug, self.appId ?? "nil", self.currentPath ?? "nil", String(self.isFirstLoad), String(self.pageLoaded), String(self.showEventSent))

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

                        let _ = self.dummyNativeOnPageShow(appId: appId, path: currentPath)
                        self.showEventSent = true  // Mark that we've sent the event
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
            // Register with native layer if not already registered and we have appId/path
            if !isRegistered, let appId = appId, let currentPath = currentPath {

                let result = dummyNativeOnWebViewAttached(appId: appId, path: currentPath)
                if result == 0 {
                    isRegistered = true
                }
            }
        } else {
            // Clean up resources when being removed from superview
            if messageChannel != nil {
                configuration.userContentController.removeScriptMessageHandler(forName: "lingxiaMessageHandler")
                messageChannel = nil
            }
            channelInitialized = false  // Reset the flag when detached
            pauseWebView()
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
        let configJson = dummyNativeGetPageConfig(appId: appId, path: currentPath)
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
                   log: Self.log, type: .info, String(enabled), throttleMs)

            if enabled {
                // Enable scroll delegate if not already set
                if self.scrollView.delegate == nil {
                    self.scrollView.delegate = self
                }
                self.lastScrollEventTime = 0 // Reset throttle timer
            }
        }
    }

    // Dummy native functions - replace with actual native calls
    private func dummyNativeOnWebViewAttached(appId: String, path: String) -> Int32 {
        os_log("[DUMMY] Webview attached for %{public}@ at %{public}@", log: Self.log, type: .debug, appId, path)

        // Move the page creation logic here from dummyNativeOnPageCreated
        os_log("LingXia: [DUMMY] %{public}@ called - appId: %{public}@, path: %{public}@", log: Self.log, type: .debug, #function, appId, path)

        // Load bing.com in the current webview for demo purposes
        DispatchQueue.main.async { [weak self] in
            if let url = URL(string: "https://www.bing.com") {
                let request = URLRequest(url: url)
                let _ = self?.load(request)
                os_log("LingXia: [DUMMY] Loading bing.com in current WebView for appId: %{public}@, path: %{public}@", log: Self.log, type: .debug, appId, path)
            }
        }

        // Pre-create other tabBar pages when home page is attached
        if path.contains("home/index.html") {
            os_log("LingXia: [DUMMY] Home page attached, pre-creating other tabBar pages", log: Self.log, type: .info)

            let tabBarPages = [
                "pages/API/index.html",
                "pages/todo/index.html"
            ]

            DispatchQueue.main.async {
                for tabPage in tabBarPages {
                    // Check if WebView already exists
                    if WebViewManager.shared.getWebView(appId: appId, path: tabPage) == nil {
                        do {
                            let preCreatedWebView = try LingXiaWebView.createWebView(appId: appId, path: tabPage)
                            WebViewManager.shared.storeWebView(preCreatedWebView, appId: appId, path: tabPage)
                            os_log("LingXia: [DUMMY] Pre-created WebView for %{public}@", log: Self.log, type: .info, tabPage)

                            if let url = URL(string: "https://www.baidu.com") {
                                let request = URLRequest(url: url)
                                let _ = preCreatedWebView.load(request)
                                os_log("LingXia: [DUMMY] Pre-loading content for %{public}@", log: Self.log, type: .info, tabPage)
                            }
                        } catch {
                            os_log("LingXia: [DUMMY] Failed to pre-create WebView for %{public}@: %{public}@", log: Self.log, type: .error, tabPage, error.localizedDescription)
                        }
                    } else {
                        os_log("LingXia: [DUMMY] WebView for %{public}@ already exists, skipping pre-creation", log: Self.log, type: .debug, tabPage)
                    }
                }
            }
        }

        return 0
    }

    private func dummyNativeHandlePostMessage(appId: String, path: String, message: String) -> Int32 {
        os_log("[DUMMY] Page handledPost for %{public}@ at %{public}@", log: Self.log, type: .debug, appId, path)
        return 0
    }

    private func dummyNativeOnPageStarted(appId: String, path: String) -> Int32 {
        os_log("[DUMMY] Page started for %{public}@ at %{public}@", log: Self.log, type: .debug, appId, path)
        return 0
    }

    private func dummyNativeOnPageFinished(appId: String, path: String) -> Int32 {
        os_log("[DUMMY] Page finished for %{public}@ at %{public}@", log: Self.log, type: .debug, appId, path)
        return 0
    }

    private func dummyNativeOnPageShow(appId: String, path: String) {
        os_log("[DUMMY] Page show for %{public}@ at %{public}@", log: Self.log, type: .debug, appId, path)
    }

    private func dummyNativeShouldOverrideUrlLoading(appId: String, url: String) -> Int32 {
        os_log("[DUMMY] Should override URL loading for %{public}@: %{public}@", log: Self.log, type: .debug, appId, url)
        return 0
    }

    private func dummyNativeGetPageConfig(appId: String, path: String) -> String? {
        os_log("LingXia: [DUMMY] %{public}@ called - appId: %{public}@, path: %{public}@", log: Self.log, type: .debug, #function, appId, path)

        // Different configs for different pages
        let result: String
        let isHomePage = path.contains("home/index.html")
        os_log("LingXia: [DUMMY] Path matching - path: '%{public}@', contains 'home/index.html': %{public}@", log: Self.log, type: .debug, path, String(isHomePage))

        if isHomePage {
            // Home page - hidden navigation bar (backgroundColor ignored when hidden)
            result = """
            {
                "hidden": true
            }
            """
            os_log("LingXia: [DUMMY] Returning HOME page config with hidden=true", log: Self.log, type: .debug)
        } else {
            // Other pages - show navigation bar with white background
            result = """
            {
                "hidden": false,
                "navigationBarTitleText": "LingXia Demo",
                "navigationBarBackgroundColor": "#ffffff",
                "navigationBarTextStyle": "black",
                "backgroundColor": "#ffffff"
            }
            """
            os_log("LingXia: [DUMMY] Returning OTHER page config with navigationBar visible", log: Self.log, type: .debug)
        }

        os_log("LingXia: [DUMMY] %{public}@ returning: %{public}@", log: Self.log, type: .debug, #function, result)
        return result
    }

    private func dummyNativeOnScrollChanged(appId: String, path: String, scrollX: Int, scrollY: Int, maxScrollX: Int, maxScrollY: Int) -> Int32 {
        os_log("[DUMMY] Scroll changed for %{public}@ at %{public}@: (%d,%d)", log: Self.log, type: .debug, appId, path, scrollX, scrollY)
        return 0
    }

    private func handlePageFinished(url: String?) {
        guard let appId = appId, let currentPath = currentPath else { return }

        let _ = dummyNativeOnPageFinished(appId: appId, path: currentPath)

        // If page is loaded and attached to superview, and we haven't sent PageShow yet
        if superview != nil && url != nil && !showEventSent {
            dummyNativeOnPageShow(appId: appId, path: currentPath)
            showEventSent = true
        }
    }
}

// MARK: - WKNavigationDelegate
extension LingXiaWebView: WKNavigationDelegate {
    public func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {

        pageLoaded = false

        if let appId = appId, let currentPath = currentPath {
            let _ = dummyNativeOnPageStarted(appId: appId, path: currentPath)
        }
    }

    public func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {


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

    public func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
        guard let url = navigationAction.request.url else {
            decisionHandler(.allow)
            return
        }

        // For HTTPS requests, check if we should intercept them
        if url.scheme == "https" {
            let request = navigationAction.request

            // Convert to HttpRequest for Rust processing
            if let httpRequest = convertToHttpRequest(request),
               let appId = appId {

                // Call Rust to check if we should handle this request
                if let httpResponse = dummyNativehandleRequest(appId: appId, httpRequest: httpRequest) {
                    // If Rust handled it, cancel the navigation
                    decisionHandler(.cancel)

                    // Check if this is a blocked request (403 status)
                    if httpResponse.status_code == 403 {
                        os_log("HTTPS request blocked by Rust: %@", log: Self.log, type: .default, url.absoluteString)
                        // Could show an error page or just silently block
                        return
                    }

                    // For other responses, we could inject custom content
                    // This is more complex and might require custom handling
                    os_log("HTTPS request handled by Rust: %@ (status: %d)", log: Self.log, type: .info, url.absoluteString, httpResponse.status_code)
                    return
                }
            }
        }

        // Default URL override check (existing logic)
        if let appId = appId {
            let result = dummyNativeShouldOverrideUrlLoading(appId: appId, url: url.absoluteString)
            if result == 1 {
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
    // Handle console messages and other UI delegate methods
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
        if let messageBody = message.body as? String {
            messageHandler(messageBody)
        }
    }
}

private func dummyNativehandleRequest(appId: String, httpRequest: HttpRequest) -> HttpResponse? {
    return nil
}
