import Foundation
import OSLog
import CLingXiaRustAPI
import CLingXiaSwiftAPI

#if os(macOS)
import AppKit
import WebKit

@MainActor
enum LxAppSurface {
    private static let log = OSLog(subsystem: "LingXia", category: "Surface")
    private static let kindWindow: Int32 = 0
    private static let kindPopup: Int32 = 1
    // Arbitrated role (mirrors lingxia_platform SurfaceRole): only an aside docks.
    private static let roleMain: Int32 = 0
    private static let roleAside: Int32 = 1
    private static let roleFloat: Int32 = 2
    private static let contentPage: Int32 = 0
    private static let contentUrl: Int32 = 1
    private static let transientCornerRadius: CGFloat = 12
    private static var entries: [String: Entry] = [:]

    private final class Entry {
        let id: String
        let appId: String
        let pageInstanceId: String
        let hostView: LxAppHostView?
        let webView: WKWebView?
        let navigationDelegate: WKNavigationDelegate?
        let window: NSWindow?
        weak var parentWindow: NSWindow?
        let delegate: WindowDelegate
        /// Set when the surface is an edge-aside docked in-window via the shell's
        /// workspace panels (Adaptive Surface Layout split pane) rather than a
        /// standalone window. `nil` for window/float surfaces.
        let dockedPosition: PanelPosition?
        /// The container view handed to the shell's panel dock; held so close()
        /// can detach it from the panel slot.
        let dockedContainer: NSView?
        /// Set for float popups (popups above the layout). A float is created +
        /// registered hidden here; the reconciler is the single authority for
        /// its visibility, showing/dismissing it from `plan.floats`.
        let isFloat: Bool

        init(
            id: String,
            appId: String,
            pageInstanceId: String,
            hostView: LxAppHostView?,
            webView: WKWebView?,
            navigationDelegate: WKNavigationDelegate?,
            window: NSWindow?,
            parentWindow: NSWindow?,
            delegate: WindowDelegate,
            dockedPosition: PanelPosition? = nil,
            dockedContainer: NSView? = nil,
            isFloat: Bool = false
        ) {
            self.id = id
            self.appId = appId
            self.pageInstanceId = pageInstanceId
            self.hostView = hostView
            self.webView = webView
            self.navigationDelegate = navigationDelegate
            self.window = window
            self.parentWindow = parentWindow
            self.delegate = delegate
            self.dockedPosition = dockedPosition
            self.dockedContainer = dockedContainer
            self.isFloat = isFloat
        }
    }

    private struct SurfaceContext {
        let frame: NSRect
        let anchorView: NSView?
        let parentWindow: NSWindow?
    }

    private final class WindowDelegate: NSObject, NSWindowDelegate {
        let id: String
        let appId: String

        init(id: String, appId: String) {
            self.id = id
            self.appId = appId
        }

        func windowShouldClose(_ sender: NSWindow) -> Bool {
            _ = LxAppSurface.close(id: id, appId: appId, reason: "user")
            return false
        }
    }

    private final class PopupWindow: NSPanel {
        override var canBecomeKey: Bool { true }
        override var canBecomeMain: Bool { true }
    }

    private final class BackdropView: NSView {
        let id: String
        let appId: String

        init(id: String, appId: String) {
            self.id = id
            self.appId = appId
            super.init(frame: .zero)
            wantsLayer = true
            layer?.backgroundColor = NSColor.black.withAlphaComponent(0.45).cgColor
        }

        required init?(coder: NSCoder) {
            nil
        }

        override func mouseDown(with event: NSEvent) {
            _ = LxAppSurface.close(id: id, appId: appId, reason: "user")
        }
    }

    private final class WebNavigationDelegate: NSObject, WKNavigationDelegate {
        let initialURL: URL

        init(initialURL: URL) {
            self.initialURL = initialURL
        }

        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
            guard let url = navigationAction.request.url, LxAppSurface.isSameOrigin(initialURL, url) else {
                decisionHandler(.cancel)
                return
            }
            decisionHandler(.allow)
        }
    }

    static func present(
        id: String,
        appId: String,
        path: String,
        sessionId: UInt64,
        pageInstanceId rawPageInstanceId: String,
        content: Int32,
        kind: Int32,
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double,
        position: Int32,
        role: Int32
    ) -> Bool {
        if let existing = entries[id] {
            if existing.dockedPosition != nil {
                LxAppActiveHost.activeShell?.showPanel(id: id)
            } else if existing.isFloat {
                // A float's visibility is owned by the reconciler (plan.floats);
                // present_surface only re-asserts existence. The commit firing
                // present_layout right after will show it.
            } else {
                existing.window?.makeKeyAndOrderFront(nil)
            }
            return true
        }

        // Adaptive Surface Layout: only an arbitrated aside (overlay on a
        // dockable edge) renders as an in-window split pane via the shell's
        // workspace dock — the same mechanism the terminal uses. Floats (and any
        // other overlay) stay on the positioned popup path below.
        if kind == kindPopup,
           role == roleAside,
           let panelPosition = panelPosition(for: position),
           let shell = LxAppActiveHost.activeShell {
            return presentDockedAside(
                id: id,
                appId: appId,
                path: path,
                sessionId: sessionId,
                pageInstanceId: rawPageInstanceId,
                content: content,
                panelPosition: panelPosition,
                width: width,
                height: height,
                widthRatio: widthRatio,
                heightRatio: heightRatio,
                shell: shell
            )
        }

        let context = surfaceContext(kind: kind)
        if kind != kindWindow && kind != kindPopup {
            os_log("unsupported surface kind=%{public}d id=%{public}@ app=%{public}@", log: log, type: .error, kind, id, appId)
            return false
        }

        let surfaceFrame = windowFrame(
            kind: kind,
            width: width,
            height: height,
            widthRatio: widthRatio,
            heightRatio: heightRatio,
            position: position,
            containerFrame: context.frame
        )
        let windowFrame = kind == kindPopup ? context.frame : surfaceFrame
        let window: NSWindow? = makeWindow(kind: kind, frame: windowFrame)
        let windowContent = NSView(frame: NSRect(origin: .zero, size: windowFrame.size))
        let contentHost: NSView
        if kind == kindPopup {
            windowContent.wantsLayer = true
            windowContent.layer?.backgroundColor = NSColor.clear.cgColor
            let backdrop = BackdropView(id: id, appId: appId)
            backdrop.frame = windowContent.bounds
            backdrop.autoresizingMask = [.width, .height]
            windowContent.addSubview(backdrop)

            let cardFrame = NSRect(
                x: surfaceFrame.minX - windowFrame.minX,
                y: surfaceFrame.minY - windowFrame.minY,
                width: surfaceFrame.width,
                height: surfaceFrame.height
            )
            let card = NSView(frame: cardFrame)
            configureContentChrome(card, kind: kind)
            windowContent.addSubview(card)
            contentHost = card
        } else {
            configureContentChrome(windowContent, kind: kind)
            contentHost = windowContent
        }

        let delegate = WindowDelegate(id: id, appId: appId)
        window?.contentView = windowContent
        window?.delegate = delegate

        let pageInstanceId: String
        let hostView: LxAppHostView?
        let webView: WKWebView?
        let navigationDelegate: WKNavigationDelegate?

        switch content {
        case contentPage:
            pageInstanceId = rawPageInstanceId.trimmingCharacters(in: .whitespacesAndNewlines)
            if path.isEmpty || pageInstanceId.isEmpty {
                os_log(
                    "present page requires path and pageInstanceId id=%{public}@ app=%{public}@ path=%{public}@ pageInstanceId=%{public}@ content=%{public}d kind=%{public}d",
                    log: log,
                    type: .error,
                    id,
                    appId,
                    path,
                    pageInstanceId,
                    content,
                    kind
                )
                return false
            }

            let controller = LxAppActiveHost.activeController ?? LxAppController()
            let lxHostView = LxAppHostView(controller: controller)
            lxHostView.translatesAutoresizingMaskIntoConstraints = false
            lxHostView.wantsLayer = true
            lxHostView.layer?.backgroundColor = NSColor.clear.cgColor
            contentHost.addSubview(lxHostView)
            pinToEdges(lxHostView, in: contentHost)

            let session = LxAppSession(
                id: LxAppSessionID(rawValue: sessionId),
                appId: appId,
                path: path,
                presentation: .normal,
                userInfo: [
                    "pageInstanceId": .string(pageInstanceId),
                    "dynamicSurfaceId": .string(id),
                ]
            )

            hostView = lxHostView
            webView = nil
            navigationDelegate = nil

            Task { @MainActor in
                do {
                    try await lxHostView.mount(session, notifyVisibleOnMount: true)
                } catch {
                    os_log(
                        "mount failed id=%{public}@ app=%{public}@ path=%{public}@ error=%{public}@",
                        log: log,
                        type: .error,
                        id,
                        appId,
                        path,
                        String(describing: error)
                    )
                    _ = close(id: id, appId: appId, reason: "failed")
                }
            }

        case contentUrl:
            guard let url = URL(string: path), let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else {
                os_log("invalid web surface url id=%{public}@ url=%{public}@", log: log, type: .error, id, path)
                return false
            }
            pageInstanceId = ""
            hostView = nil
            let configuration = WKWebViewConfiguration()
            let wkWebView = WKWebView(frame: contentHost.bounds, configuration: configuration)
            let delegate = WebNavigationDelegate(initialURL: url)
            wkWebView.navigationDelegate = delegate
            wkWebView.translatesAutoresizingMaskIntoConstraints = false
            contentHost.addSubview(wkWebView)
            pinToEdges(wkWebView, in: contentHost)
            wkWebView.load(URLRequest(url: url))
            webView = wkWebView
            navigationDelegate = delegate

        default:
            os_log("unsupported surface content=%{public}d id=%{public}@ app=%{public}@ path=%{public}@ kind=%{public}d", log: log, type: .error, content, id, appId, path, kind)
            return false
        }

        // A popup (kind == kindPopup, non-aside) is a float: created + registered
        // hidden here, then shown/positioned/dismissed by the reconciler from
        // plan.floats — the single authority for float visibility. A bare window
        // (kind == kindWindow) is shown immediately, as before.
        let isFloat = kind == kindPopup
        entries[id] = Entry(
            id: id,
            appId: appId,
            pageInstanceId: pageInstanceId,
            hostView: hostView,
            webView: webView,
            navigationDelegate: navigationDelegate,
            window: window,
            parentWindow: context.parentWindow,
            delegate: delegate,
            isFloat: isFloat
        )

        if isFloat {
            // Do NOT order the popup front at create time. The commit firing
            // present_layout right after this present_surface returns drives the
            // reconciler, which shows the float (showFloat) from plan.floats.
            return true
        }

        if kind != kindWindow, let parentWindow = context.parentWindow, let window {
            parentWindow.addChildWindow(window, ordered: .above)
        }
        window?.makeKeyAndOrderFront(nil)
        return true
    }

    /// Render an edge-aside as an in-window split pane docked into the shell's
    /// workspace. The aside's content (page host or web view) is mounted into a
    /// plain container that the shell pins inside a panel slot; the main content
    /// card shrinks to make room, producing a real split rather than a floating
    /// window.
    private static func presentDockedAside(
        id: String,
        appId: String,
        path: String,
        sessionId: UInt64,
        pageInstanceId rawPageInstanceId: String,
        content: Int32,
        panelPosition: PanelPosition,
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double,
        shell: LxAppShell
    ) -> Bool {
        let container = NSView()
        container.wantsLayer = true
        container.layer?.backgroundColor = NSColor.clear.cgColor

        let pageInstanceId: String
        let hostView: LxAppHostView?
        let webView: WKWebView?
        let navigationDelegate: WKNavigationDelegate?

        switch content {
        case contentPage:
            pageInstanceId = rawPageInstanceId.trimmingCharacters(in: .whitespacesAndNewlines)
            if path.isEmpty || pageInstanceId.isEmpty {
                os_log(
                    "aside page requires path and pageInstanceId id=%{public}@ app=%{public}@ path=%{public}@ pageInstanceId=%{public}@",
                    log: log, type: .error, id, appId, path, pageInstanceId
                )
                return false
            }
            let controller = LxAppActiveHost.activeController ?? LxAppController()
            let lxHostView = LxAppHostView(controller: controller)
            lxHostView.translatesAutoresizingMaskIntoConstraints = false
            lxHostView.wantsLayer = true
            lxHostView.layer?.backgroundColor = NSColor.clear.cgColor
            container.addSubview(lxHostView)
            pinToEdges(lxHostView, in: container)

            let session = LxAppSession(
                id: LxAppSessionID(rawValue: sessionId),
                appId: appId,
                path: path,
                presentation: .normal,
                userInfo: [
                    "pageInstanceId": .string(pageInstanceId),
                    "dynamicSurfaceId": .string(id),
                ]
            )
            hostView = lxHostView
            webView = nil
            navigationDelegate = nil
            Task { @MainActor in
                do {
                    try await lxHostView.mount(session, notifyVisibleOnMount: true)
                } catch {
                    os_log(
                        "aside mount failed id=%{public}@ app=%{public}@ path=%{public}@ error=%{public}@",
                        log: log, type: .error, id, appId, path, String(describing: error)
                    )
                    _ = close(id: id, appId: appId, reason: "failed")
                }
            }

        case contentUrl:
            guard let url = URL(string: path), let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else {
                os_log("invalid web aside url id=%{public}@ url=%{public}@", log: log, type: .error, id, path)
                return false
            }
            pageInstanceId = ""
            hostView = nil
            let configuration = WKWebViewConfiguration()
            let wkWebView = WKWebView(frame: .zero, configuration: configuration)
            let navDelegate = WebNavigationDelegate(initialURL: url)
            wkWebView.navigationDelegate = navDelegate
            wkWebView.translatesAutoresizingMaskIntoConstraints = false
            container.addSubview(wkWebView)
            pinToEdges(wkWebView, in: container)
            wkWebView.load(URLRequest(url: url))
            webView = wkWebView
            navigationDelegate = navDelegate

        default:
            os_log("unsupported aside content=%{public}d id=%{public}@ app=%{public}@", log: log, type: .error, content, id, appId)
            return false
        }

        let defaultSize = dockDefaultSize(
            position: panelPosition,
            width: width,
            height: height,
            widthRatio: widthRatio,
            heightRatio: heightRatio
        )
        entries[id] = Entry(
            id: id,
            appId: appId,
            pageInstanceId: pageInstanceId,
            hostView: hostView,
            webView: webView,
            navigationDelegate: navigationDelegate,
            window: nil,
            parentWindow: nil,
            delegate: WindowDelegate(id: id, appId: appId),
            dockedPosition: panelPosition,
            dockedContainer: container
        )
        // Only CREATE + REGISTER the aside content here (hidden). The core's
        // `present_layout` (fired right after this present_surface returns)
        // drives the aside-layout reconciler, which is the sole authority for the
        // aside's edge + visibility.
        shell.registerPanelWithNativeContent(
            id: id,
            position: panelPosition,
            contentView: container,
            defaultSize: defaultSize
        )
        return true
    }

    /// Map a `SurfacePosition` integer to a dockable workspace edge. Center (0)
    /// is a float and has no dock slot — returns nil so the caller treats it as
    /// a popup.
    private static func panelPosition(for position: Int32) -> PanelPosition? {
        switch position {
        case 1: return .bottom
        case 2: return .left
        case 3: return .right
        case 4: return .top
        default: return nil
        }
    }

    private static func dockDefaultSize(
        position: PanelPosition,
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double
    ) -> CGFloat {
        switch position {
        case .left, .right:
            return finitePositive(width)
                ?? ratioSize(widthRatio, base: NSScreen.main?.visibleFrame.width ?? 1280)
                ?? 360
        case .bottom, .top:
            return finitePositive(height)
                ?? ratioSize(heightRatio, base: NSScreen.main?.visibleFrame.height ?? 800)
                ?? 320
        }
    }

    static func close(id: String, appId: String, reason: String) -> Bool {
        guard let entry = entries.removeValue(forKey: id) else {
            _ = onSurfaceClosed(appId, id, closeReason(reason))
            return true
        }
        guard entry.appId == appId else {
            entries[id] = entry
            return false
        }

        entry.hostView?.unmount(reason: closeReason(reason))
        entry.webView?.stopLoading()
        entry.webView?.navigationDelegate = nil
        if entry.dockedPosition != nil {
            // In-window aside: hide the panel slot and detach its content.
            LxAppActiveHost.activeShell?.hidePanel(id: id)
            entry.dockedContainer?.removeFromSuperview()
        }
        entry.window?.delegate = nil
        if let window = entry.window {
            entry.parentWindow?.removeChildWindow(window)
            window.contentView = nil
            window.orderOut(nil)
        }
        _ = onSurfaceClosed(appId, id, closeReason(reason))
        return true
    }

    static func show(id: String, appId: String) -> Bool {
        guard let entry = entries[id], entry.appId == appId else {
            os_log("show: surface not found id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
            return false
        }
        if entry.dockedPosition != nil {
            LxAppActiveHost.activeShell?.showPanel(id: id)
            if let webView = entry.hostView?.webView {
                MacNativeBridge.notifyPageActive(for: webView)
            }
            if !entry.pageInstanceId.isEmpty {
                _ = notifyPageInstanceVisible(entry.pageInstanceId)
            }
            return true
        }
        guard let window = entry.window else {
            os_log("show: surface has no window id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
            return false
        }
        // Defense in depth — JS-side already short-circuits on no-op.
        if window.isVisible { return true }
        if let parentWindow = entry.parentWindow, window.parent == nil {
            parentWindow.addChildWindow(window, ordered: .above)
        }
        window.makeKeyAndOrderFront(nil)
        // Wake any native overlay components on this page (video player,
        // media swiper, ...) — their views were hidden and playback paused
        // when hide() ran; this routes the active lifecycle so they re-show
        // and the components that were playing auto-resume.
        if let webView = entry.hostView?.webView {
            MacNativeBridge.notifyPageActive(for: webView)
        }
        // Fire the page-side onShow lifecycle so JS observes the visibility
        // transition. Skipped for URL surfaces (no page instance).
        if !entry.pageInstanceId.isEmpty {
            _ = notifyPageInstanceVisible(entry.pageInstanceId)
        }
        return true
    }

    static func hide(id: String, appId: String) -> Bool {
        guard let entry = entries[id], entry.appId == appId else {
            os_log("hide: surface not found id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
            return false
        }
        if entry.dockedPosition != nil {
            LxAppActiveHost.activeShell?.hidePanel(id: id)
            if let webView = entry.hostView?.webView {
                MacNativeBridge.notifyPageInactive(for: webView)
            }
            if !entry.pageInstanceId.isEmpty {
                _ = notifyPageInstanceHidden(entry.pageInstanceId, "hidden")
            }
            return true
        }
        guard let window = entry.window else {
            os_log("hide: surface has no window id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
            return false
        }
        if !window.isVisible { return true }
        // orderOut keeps the window's contentView/web view/page mount intact;
        // a subsequent show() simply reorders it back to the front. We still
        // fire the page-side onHide lifecycle so JS can pause timers etc.
        entry.parentWindow?.removeChildWindow(window)
        window.orderOut(nil)
        // Pause and visually hide any native overlay components on this page.
        // The NativeComponentManager records playback intent so the matching
        // show() can auto-resume what was playing.
        if let webView = entry.hostView?.webView {
            MacNativeBridge.notifyPageInactive(for: webView)
        }
        if !entry.pageInstanceId.isEmpty {
            _ = notifyPageInstanceHidden(entry.pageInstanceId, "hidden")
        }
        return true
    }

    /// Float popups currently shown. A float is created + registered hidden by
    /// `present`; this is the set the reconciler already brought on-screen, so it
    /// can leave them untouched (idempotent) and dismiss any no longer desired.
    static func visibleFloatIds() -> Set<String> {
        Set(
            entries.values
                .filter { $0.isFloat && ($0.window?.isVisible ?? false) }
                .map { $0.id }
        )
    }

    /// Order a registered (hidden) float popup on-screen. Idempotent: a float
    /// already visible is left exactly as is (no flicker). Driven solely by the
    /// reconciler from `plan.floats`.
    @discardableResult
    static func showFloat(id: String) -> Bool {
        guard let entry = entries[id], entry.isFloat, let window = entry.window else {
            return false
        }
        if window.isVisible { return true }
        if let parentWindow = entry.parentWindow, window.parent == nil {
            parentWindow.addChildWindow(window, ordered: .above)
        }
        window.makeKeyAndOrderFront(nil)
        return true
    }

    /// Dismiss a float popup the core no longer lists in `plan.floats`. This is
    /// the existing popup teardown (close), which unmounts the page, detaches the
    /// child window, and fires the close observer so the modal-focus stack pops.
    @discardableResult
    static func dismissFloat(id: String, appId: String) -> Bool {
        guard let entry = entries[id], entry.isFloat else { return false }
        return close(id: id, appId: appId, reason: "programmatic")
    }

    private static func pinToEdges(_ child: NSView, in parent: NSView) {
        NSLayoutConstraint.activate([
            child.leadingAnchor.constraint(equalTo: parent.leadingAnchor),
            child.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
            child.topAnchor.constraint(equalTo: parent.topAnchor),
            child.bottomAnchor.constraint(equalTo: parent.bottomAnchor),
        ])
    }

    private static func makeWindow(kind: Int32, frame: NSRect) -> NSWindow {
        var style: NSWindow.StyleMask
        switch kind {
        case kindWindow:
            style = [.titled, .closable, .miniaturizable, .resizable]
        default:
            style = [.borderless]
        }

        let window: NSWindow
        if kind == kindPopup {
            let popupWindow = PopupWindow(contentRect: frame, styleMask: style, backing: .buffered, defer: false)
            popupWindow.isFloatingPanel = true
            popupWindow.hidesOnDeactivate = false
            popupWindow.becomesKeyOnlyIfNeeded = false
            window = popupWindow
        } else {
            window = NSWindow(contentRect: frame, styleMask: style, backing: .buffered, defer: false)
        }
        window.title = ""
        window.isReleasedWhenClosed = false
        if kind == kindWindow {
            // Use a standard NSWindow chrome (title bar + traffic lights)
            // rather than the previous "fullSizeContentView + transparent
            // title bar + hidden title" combination. The chromeless look was
            // pretty but left users with no visible affordance to grab the
            // window for drag/resize: WebView covers the entire content view
            // and captures the mouse events that would otherwise reach the
            // (invisible) title bar. A standard title bar gives back drag,
            // close/minimize/zoom, and corner resize at the cost of a
            // ~28-pt header strip.
            //
            // Note: deliberately NOT setting `isMovableByWindowBackground =
            // true` — the WebView intercepts text selection, scroll, and
            // drag-and-drop, so a "drag anywhere on background" policy
            // would steal those gestures from the page. The visible title
            // bar is the drag affordance.
            window.titlebarAppearsTransparent = false
            window.titleVisibility = .visible
            window.backgroundColor = .windowBackgroundColor
        } else if kind == kindPopup {
            window.backgroundColor = .clear
            window.isOpaque = false
            window.hasShadow = false
        } else {
            window.backgroundColor = .windowBackgroundColor
        }
        return window
    }

    private static func configureContentChrome(_ content: NSView, kind: Int32) {
        guard kind == kindPopup else { return }
        content.wantsLayer = true
        content.layer?.backgroundColor = NSColor.white.cgColor
        content.layer?.cornerRadius = transientCornerRadius
        content.layer?.masksToBounds = true
        content.layer?.borderWidth = 0
        content.layer?.edgeAntialiasingMask = [
            .layerLeftEdge,
            .layerRightEdge,
            .layerBottomEdge,
            .layerTopEdge,
        ]
    }

    private static func surfaceContext(kind: Int32) -> SurfaceContext {
        let screenFrame = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1280, height: 800)
        guard kind != kindWindow else {
            return SurfaceContext(frame: screenFrame, anchorView: nil, parentWindow: nil)
        }

        if let shell = LxAppActiveHost.activeShell,
           let context = contextFrame(for: shell.contentPanelView ?? shell.window?.contentView) {
            return context
        }

        if let context = contextFrame(for: NSApp.keyWindow?.contentView ?? NSApp.mainWindow?.contentView) {
            return context
        }

        return SurfaceContext(frame: screenFrame, anchorView: nil, parentWindow: nil)
    }

    private static func contextFrame(for view: NSView?) -> SurfaceContext? {
        guard let view, let window = view.window else { return nil }
        view.layoutSubtreeIfNeeded()
        let rectInWindow = view.convert(view.bounds, to: nil)
        var frame = window.convertToScreen(rectInWindow)
        if let screenFrame = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame {
            frame = frame.intersection(screenFrame)
        }
        guard frame.width > 0, frame.height > 0 else { return nil }
        return SurfaceContext(frame: frame, anchorView: view, parentWindow: window)
    }

    private static func windowFrame(
        kind: Int32,
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double,
        position: Int32,
        containerFrame: NSRect
    ) -> NSRect {
        let defaultSize: NSSize = {
            switch kind {
            case kindWindow:
                return NSSize(width: 960, height: 720)
            default:
                return NSSize(width: 360, height: 420)
            }
        }()

        let resolvedWidth = finitePositive(width)
            ?? ratioSize(widthRatio, base: containerFrame.width)
            ?? defaultSize.width
        let resolvedHeight = finitePositive(height)
            ?? ratioSize(heightRatio, base: containerFrame.height)
            ?? defaultSize.height
        let size = NSSize(
            width: min(max(resolvedWidth, 240), containerFrame.width),
            height: min(max(resolvedHeight, 160), containerFrame.height)
        )

        let origin = kind == kindWindow
            ? NSPoint(x: containerFrame.midX - size.width / 2, y: containerFrame.midY - size.height / 2)
            : positionedOrigin(position: position, size: size, containerFrame: containerFrame)

        return NSRect(origin: origin, size: size)
    }

    private static func positionedOrigin(position: Int32, size: NSSize, containerFrame: NSRect) -> NSPoint {
        switch position {
        case 1:
            return NSPoint(x: containerFrame.midX - size.width / 2, y: containerFrame.minY)
        case 2:
            return NSPoint(x: containerFrame.minX, y: containerFrame.midY - size.height / 2)
        case 3:
            return NSPoint(x: containerFrame.maxX - size.width, y: containerFrame.midY - size.height / 2)
        case 4:
            return NSPoint(x: containerFrame.midX - size.width / 2, y: containerFrame.maxY - size.height)
        default:
            return NSPoint(x: containerFrame.midX - size.width / 2, y: containerFrame.midY - size.height / 2)
        }
    }

    private static func finitePositive(_ value: Double) -> CGFloat? {
        guard value.isFinite, value > 0 else { return nil }
        return CGFloat(value)
    }

    private static func ratioSize(_ value: Double, base: CGFloat) -> CGFloat? {
        guard value.isFinite, value > 0 else { return nil }
        return CGFloat(min(value, 1.0)) * base
    }

    private static func closeReason(_ reason: String) -> String {
        switch reason {
        case "user", "programmatic", "owner_closed", "app_closed", "reclaimed", "failed", "unknown":
            return reason
        default:
            return "unknown"
        }
    }

    private static func isSameOrigin(_ initial: URL, _ next: URL) -> Bool {
        guard let initialScheme = initial.scheme?.lowercased(),
              let nextScheme = next.scheme?.lowercased(),
              initialScheme == nextScheme,
              let initialHost = initial.host?.lowercased(),
              let nextHost = next.host?.lowercased(),
              initialHost == nextHost else {
            return false
        }
        return effectivePort(initial) == effectivePort(next)
    }

    private static func effectivePort(_ url: URL) -> Int {
        if let port = url.port {
            return port
        }
        switch url.scheme?.lowercased() {
        case "http": return 80
        case "https": return 443
        default: return -1
        }
    }
}

#elseif os(iOS)
import UIKit
import WebKit

@MainActor
enum LxAppSurface {
    private static let log = OSLog(subsystem: "LingXia", category: "Surface")
    private static let kindPopup: Int32 = 1
    private static let contentPage: Int32 = 0
    private static let contentUrl: Int32 = 1
    private static let transientCornerRadius: CGFloat = 12
    private static var entries: [String: Entry] = [:]

    private final class Entry {
        let id: String
        let appId: String
        let pageInstanceId: String
        let hostView: LxAppHostView?
        let webView: WKWebView?
        let navigationDelegate: WKNavigationDelegate?
        let window: UIWindow

        init(
            id: String,
            appId: String,
            pageInstanceId: String,
            hostView: LxAppHostView?,
            webView: WKWebView?,
            navigationDelegate: WKNavigationDelegate?,
            window: UIWindow
        ) {
            self.id = id
            self.appId = appId
            self.pageInstanceId = pageInstanceId
            self.hostView = hostView
            self.webView = webView
            self.navigationDelegate = navigationDelegate
            self.window = window
        }
    }

    private final class WebNavigationDelegate: NSObject, WKNavigationDelegate {
        let initialURL: URL

        init(initialURL: URL) {
            self.initialURL = initialURL
        }

        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
            guard let url = navigationAction.request.url, LxAppSurface.isSameOrigin(initialURL, url) else {
                decisionHandler(.cancel)
                return
            }
            decisionHandler(.allow)
        }
    }

    private final class PopupViewController: UIViewController, UIGestureRecognizerDelegate {
        let id: String
        let appId: String
        let contentView = UIView()
        private let contentFrame: CGRect
        /// True when the surface was opened with both ratios ≈ 1.0. Drives
        /// status-bar hiding + corner-radius / backdrop adjustments so the
        /// surface visually covers the system status area, matching the
        /// Android immersive treatment.
        private let isFullScreen: Bool

        init(id: String, appId: String, contentFrame: CGRect, isFullScreen: Bool) {
            self.id = id
            self.appId = appId
            self.contentFrame = contentFrame
            self.isFullScreen = isFullScreen
            super.init(nibName: nil, bundle: nil)
            modalPresentationStyle = .overFullScreen
        }

        required init?(coder: NSCoder) {
            nil
        }

        // Hide the iOS status bar (time, signal, battery) when the surface
        // is full-screen so its content actually covers the area, instead of
        // showing through behind the system glyphs.
        override var prefersStatusBarHidden: Bool { isFullScreen }
        override var prefersHomeIndicatorAutoHidden: Bool { isFullScreen }
        override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge {
            isFullScreen ? .all : []
        }

        override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = isFullScreen
                ? UIColor.clear
                : UIColor.black.withAlphaComponent(0.45)
            let tap = UITapGestureRecognizer(target: self, action: #selector(closeFromBackdrop))
            tap.delegate = self
            view.addGestureRecognizer(tap)

            contentView.frame = contentFrame
            contentView.backgroundColor = .white
            contentView.layer.cornerRadius = isFullScreen ? 0 : LxAppSurface.transientCornerRadius
            contentView.layer.masksToBounds = true
            contentView.isUserInteractionEnabled = true
            view.addSubview(contentView)
        }

        @objc private func closeFromBackdrop() {
            // Full-screen surfaces have no exposed backdrop — disable the
            // tap-to-close affordance so users don't accidentally dismiss by
            // tapping anywhere on the page.
            if isFullScreen { return }
            _ = LxAppSurface.close(id: id, appId: appId, reason: "user")
        }

        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
            guard let touchedView = touch.view else { return true }
            return !touchedView.isDescendant(of: contentView)
        }
    }

    static func present(
        id: String,
        appId: String,
        path: String,
        sessionId: UInt64,
        pageInstanceId rawPageInstanceId: String,
        content: Int32,
        kind: Int32,
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double,
        position: Int32
    ) -> Bool {
        guard kind == kindPopup else {
            os_log("unsupported mobile surface kind=%{public}d id=%{public}@ app=%{public}@", log: log, type: .error, kind, id, appId)
            return false
        }
        guard content == contentPage || content == contentUrl else {
            os_log("unsupported surface content=%{public}d id=%{public}@ app=%{public}@", log: log, type: .error, content, id, appId)
            return false
        }
        guard let windowScene = activeWindowScene() else {
            os_log("no active window scene for surface id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
            return false
        }

        if entries[id] != nil {
            entries[id]?.window.makeKeyAndVisible()
            return true
        }

        let containerFrame = windowScene.screen.bounds
        let contentFrame = popupFrame(
            width: width,
            height: height,
            widthRatio: widthRatio,
            heightRatio: heightRatio,
            position: position,
            containerFrame: containerFrame
        )
        // Same criterion as Android: both ratios at (near) 1.0 means "fill the
        // whole screen including the status bar area".
        let isFullScreen = widthRatio.isFinite
            && heightRatio.isFinite
            && widthRatio >= 0.999
            && heightRatio >= 0.999
        let controller = PopupViewController(
            id: id,
            appId: appId,
            contentFrame: contentFrame,
            isFullScreen: isFullScreen
        )
        let window = UIWindow(windowScene: windowScene)
        window.frame = containerFrame
        window.windowLevel = .alert + 1
        window.backgroundColor = .clear
        window.rootViewController = controller

        let pageInstanceId: String
        let hostView: LxAppHostView?
        let webView: WKWebView?
        let navigationDelegate: WKNavigationDelegate?

        switch content {
        case contentPage:
            pageInstanceId = rawPageInstanceId.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !path.isEmpty, !pageInstanceId.isEmpty else {
                os_log("present page requires path and pageInstanceId id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
                return false
            }

            let activeController = LxAppActiveHost.activeController ?? LxAppController()
            let lxHostView = LxAppHostView(controller: activeController)
            lxHostView.translatesAutoresizingMaskIntoConstraints = false
            controller.contentView.addSubview(lxHostView)
            pinToEdges(lxHostView, in: controller.contentView)

            let session = LxAppSession(
                id: LxAppSessionID(rawValue: sessionId),
                appId: appId,
                path: path,
                presentation: .normal,
                userInfo: [
                    "pageInstanceId": .string(pageInstanceId),
                    "dynamicSurfaceId": .string(id),
                ]
            )
            hostView = lxHostView
            webView = nil
            navigationDelegate = nil
            Task { @MainActor in
                do {
                    try await lxHostView.mount(session, notifyVisibleOnMount: true)
                } catch {
                    os_log("mount failed id=%{public}@ app=%{public}@ path=%{public}@ error=%{public}@", log: log, type: .error, id, appId, path, String(describing: error))
                    _ = close(id: id, appId: appId, reason: "failed")
                }
            }

        case contentUrl:
            guard let url = URL(string: path), let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else {
                os_log("invalid web surface url id=%{public}@ url=%{public}@", log: log, type: .error, id, path)
                return false
            }
            pageInstanceId = ""
            hostView = nil
            let wkWebView = WKWebView(frame: controller.contentView.bounds, configuration: WKWebViewConfiguration())
            let delegate = WebNavigationDelegate(initialURL: url)
            wkWebView.navigationDelegate = delegate
            wkWebView.translatesAutoresizingMaskIntoConstraints = false
            controller.contentView.addSubview(wkWebView)
            pinToEdges(wkWebView, in: controller.contentView)
            wkWebView.load(URLRequest(url: url))
            webView = wkWebView
            navigationDelegate = delegate

        default:
            return false
        }

        entries[id] = Entry(
            id: id,
            appId: appId,
            pageInstanceId: pageInstanceId,
            hostView: hostView,
            webView: webView,
            navigationDelegate: navigationDelegate,
            window: window
        )
        window.makeKeyAndVisible()
        return true
    }

    static func close(id: String, appId: String, reason: String) -> Bool {
        guard let entry = entries.removeValue(forKey: id) else {
            _ = onSurfaceClosed(appId, id, closeReason(reason))
            return true
        }
        guard entry.appId == appId else {
            entries[id] = entry
            return false
        }
        entry.hostView?.unmount(reason: closeReason(reason))
        entry.webView?.stopLoading()
        entry.webView?.navigationDelegate = nil
        entry.window.rootViewController = nil
        entry.window.isHidden = true
        _ = onSurfaceClosed(appId, id, closeReason(reason))
        return true
    }

    static func show(id: String, appId: String) -> Bool {
        guard let entry = entries[id], entry.appId == appId else {
            os_log("show: surface not found id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
            return false
        }
        // Defense in depth — the Rust JS-side closure already short-circuits
        // on no-op show/hide so this is currently unreachable from JS callers,
        // but guard here so future SDK-internal callers don't double-fire the
        // page lifecycle.
        if !entry.window.isHidden { return true }
        entry.window.isHidden = false
        entry.window.makeKeyAndVisible()
        // Wake any native overlay components on this page (video player,
        // media swiper, ...) — their views were hidden and playback paused
        // when hide() ran; this routes the active lifecycle so they re-show
        // and components that were playing auto-resume.
        if let webView = entry.hostView?.webView {
            NativeBridge.notifyPageActive(for: webView)
        }
        // Fire the page-side onShow lifecycle so JS observes the visibility
        // transition. Skipped for URL surfaces (no page instance).
        if !entry.pageInstanceId.isEmpty {
            _ = notifyPageInstanceVisible(entry.pageInstanceId)
        }
        return true
    }

    static func hide(id: String, appId: String) -> Bool {
        guard let entry = entries[id], entry.appId == appId else {
            os_log("hide: surface not found id=%{public}@ app=%{public}@", log: log, type: .error, id, appId)
            return false
        }
        if entry.window.isHidden { return true }
        // isHidden=true keeps the rootViewController/page mount alive so a
        // subsequent show() restores the same state instead of remounting.
        // We still fire onHide so JS can pause timers / save state, and route
        // an inactive page lifecycle to NativeBridge so video / swiper / etc.
        // overlay components pause and hide their views.
        entry.window.isHidden = true
        if let webView = entry.hostView?.webView {
            NativeBridge.notifyPageInactive(for: webView)
        }
        if !entry.pageInstanceId.isEmpty {
            _ = notifyPageInstanceHidden(entry.pageInstanceId, "hidden")
        }
        return true
    }

    private static func activeWindowScene() -> UIWindowScene? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }
            ?? UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
    }

    private static func pinToEdges(_ child: UIView, in parent: UIView) {
        NSLayoutConstraint.activate([
            child.leadingAnchor.constraint(equalTo: parent.leadingAnchor),
            child.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
            child.topAnchor.constraint(equalTo: parent.topAnchor),
            child.bottomAnchor.constraint(equalTo: parent.bottomAnchor),
        ])
    }

    private static func popupFrame(
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double,
        position: Int32,
        containerFrame: CGRect
    ) -> CGRect {
        let resolvedWidth = finitePositive(width)
            ?? ratioSize(widthRatio, base: containerFrame.width)
            ?? containerFrame.width * 0.9
        let resolvedHeight = finitePositive(height)
            ?? ratioSize(heightRatio, base: containerFrame.height)
            ?? containerFrame.height * 0.55
        let size = CGSize(
            width: min(max(resolvedWidth, 160), containerFrame.width),
            height: min(max(resolvedHeight, 160), containerFrame.height)
        )
        let origin: CGPoint
        switch position {
        case 1:
            origin = CGPoint(x: containerFrame.midX - size.width / 2, y: containerFrame.maxY - size.height)
        case 2:
            origin = CGPoint(x: containerFrame.minX, y: containerFrame.midY - size.height / 2)
        case 3:
            origin = CGPoint(x: containerFrame.maxX - size.width, y: containerFrame.midY - size.height / 2)
        case 4:
            origin = CGPoint(x: containerFrame.midX - size.width / 2, y: containerFrame.minY)
        default:
            origin = CGPoint(x: containerFrame.midX - size.width / 2, y: containerFrame.midY - size.height / 2)
        }
        return CGRect(origin: origin, size: size)
    }

    private static func finitePositive(_ value: Double) -> CGFloat? {
        guard value.isFinite, value > 0 else { return nil }
        return CGFloat(value)
    }

    private static func ratioSize(_ value: Double, base: CGFloat) -> CGFloat? {
        guard value.isFinite, value > 0 else { return nil }
        return CGFloat(min(value, 1.0)) * base
    }

    private static func closeReason(_ reason: String) -> String {
        switch reason {
        case "user", "programmatic", "owner_closed", "app_closed", "reclaimed", "failed", "unknown":
            return reason
        default:
            return "unknown"
        }
    }

    private static func isSameOrigin(_ initial: URL, _ next: URL) -> Bool {
        guard let initialScheme = initial.scheme?.lowercased(),
              let nextScheme = next.scheme?.lowercased(),
              initialScheme == nextScheme,
              let initialHost = initial.host?.lowercased(),
              let nextHost = next.host?.lowercased(),
              initialHost == nextHost else {
            return false
        }
        return effectivePort(initial) == effectivePort(next)
    }

    private static func effectivePort(_ url: URL) -> Int {
        if let port = url.port {
            return port
        }
        switch url.scheme?.lowercased() {
        case "http": return 80
        case "https": return 443
        default: return -1
        }
    }
}

#else

@MainActor
enum LxAppSurface {
    static func present(
        id: String,
        appId: String,
        path: String,
        sessionId: UInt64,
        pageInstanceId: String,
        content: Int32,
        kind: Int32,
        width: Double,
        height: Double,
        widthRatio: Double,
        heightRatio: Double,
        position: Int32
    ) -> Bool {
        _ = id
        _ = appId
        _ = path
        _ = sessionId
        _ = pageInstanceId
        _ = content
        _ = kind
        _ = width
        _ = height
        _ = widthRatio
        _ = heightRatio
        _ = position
        return false
    }

    static func close(id: String, appId: String, reason: String) -> Bool {
        _ = id
        _ = appId
        _ = reason
        return false
    }

    static func show(id: String, appId: String) -> Bool {
        _ = id
        _ = appId
        return false
    }

    static func hide(id: String, appId: String) -> Bool {
        _ = id
        _ = appId
        return false
    }
}

#endif
