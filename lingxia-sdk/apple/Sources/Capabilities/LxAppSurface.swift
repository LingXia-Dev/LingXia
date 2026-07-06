import Foundation
import CLingXiaRustAPI
import CLingXiaSwiftAPI

#if os(macOS)
import AppKit
import WebKit

@MainActor
enum LxAppSurface {
    /// View a controller-hosted host (the Runner's phone simulator) renders the
    /// lxapp into. Floats are bounded to it when there is no desktop shell, so they
    /// don't spill past the device frame. Weak — owned by the host view tree.
    static weak var hostAnchorView: NSView?
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
        /// Set for a phone-simulator full-screen surface (the runner's iPhone
        /// shape): an aside/window drilled in over the device screen, mirroring
        /// the real iOS phone. The reconciler dismisses it when the core drops it.
        let isFullScreen: Bool
        /// Set when the docked aside hosts a browser (a `{ url, as: 'aside' }`
        /// surface). Owns the browser tab + chrome; close() tears it down so the
        /// underlying Rust browser tab is destroyed with the aside.
        let dockedBrowser: DockedBrowser?

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
            isFloat: Bool = false,
            isFullScreen: Bool = false,
            dockedBrowser: DockedBrowser? = nil
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
            self.isFullScreen = isFullScreen
            self.dockedBrowser = dockedBrowser
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

    /// Back button for a phone full-screen surface; closes the surface (the
    /// drill-in "return to the page beneath" affordance).
    private final class SurfaceActionButton: NSButton {
        let id: String
        let appId: String

        init(id: String, appId: String) {
            self.id = id
            self.appId = appId
            super.init(frame: .zero)
            title = ""
            target = self
            action = #selector(closeSurface)
        }

        required init?(coder: NSCoder) { nil }

        @objc private func closeSurface() {
            _ = LxAppSurface.close(id: id, appId: appId, reason: "user")
        }
    }

    private final class WebNavigationDelegate: NSObject, WKNavigationDelegate {
        let initialURL: URL

        init(initialURL: URL) {
            self.initialURL = initialURL
        }

        func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping @MainActor @Sendable (WKNavigationActionPolicy) -> Void) {
            guard let url = navigationAction.request.url else {
                decisionHandler(.cancel)
                return
            }
            // A registered URL-callback sentinel (e.g. an auth handoff) is
            // consumed by the waiting Rust channel; cancel the load.
            if urlCallbackDispatch(url.absoluteString) {
                decisionHandler(.cancel)
                return
            }
            guard LxAppSurface.isSameOrigin(initialURL, url) else {
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
                // A docked aside's visibility is owned by the layout plan
                // reconciler; present only re-asserts that content exists.
            } else if existing.isFloat {
                // A float's visibility is owned by the layout plan reconciler;
                // present only re-asserts that content exists.
            } else {
                existing.window?.makeKeyAndOrderFront(nil)
            }
            return true
        }

        // Runner iPhone shape (controller host, no shell): a phone has no
        // side-by-side room, so a page aside/window drills in full-screen over the
        // device screen, mirroring a real iOS phone. URL asides degrade to the
        // in-app browser upstream, so only pages reach here; desktop is untouched.
        if LxAppActiveHost.activeShell == nil,
           content == contentPage,
           kind == kindWindow || (kind == kindPopup && role == roleAside),
           let context = phoneDeviceScreenContext() {
            return presentPhoneFullScreen(
                id: id,
                appId: appId,
                path: path,
                sessionId: sessionId,
                pageInstanceId: rawPageInstanceId,
                content: content,
                context: context
            )
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
            LXLog.error("unsupported surface kind=\(kind) id=\(id) app=\(appId)", category: "Surface", appId: appId)
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
            // Match the controller host's rounded screen (the Runner's phone
            // simulator) so the popup's bottom lands inside the device shape.
            let hostRadius = hostScreenCornerRadius()
            if hostRadius > 0 {
                windowContent.layer?.cornerRadius = hostRadius
                windowContent.layer?.masksToBounds = true
            }
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
                LXLog.error(
                    "present page requires path and pageInstanceId id=\(id) app=\(appId) path=\(path) pageInstanceId=\(pageInstanceId) content=\(content) kind=\(kind)",
                    category: "Surface",
                    appId: appId,
                    path: path
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
                    LXLog.error(
                        "mount failed id=\(id) app=\(appId) path=\(path) error=\(String(describing: error))",
                        category: "Surface",
                        appId: appId,
                        path: path
                    )
                    _ = close(id: id, appId: appId, reason: "failed")
                }
            }

        case contentUrl:
            guard let url = URL(string: path), let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else {
                LXLog.error("invalid web surface url id=\(id) url=\(path)", category: "Surface", appId: appId, path: path)
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
            LXLog.error("unsupported surface content=\(content) id=\(id) app=\(appId) path=\(path) kind=\(kind)", category: "Surface", appId: appId, path: path)
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
            // Do NOT order the popup front directly. Visibility is owned by the
            // layout plan commit that follows this content registration.
            return true
        }

        if kind != kindWindow, let parentWindow = context.parentWindow, let window {
            parentWindow.addChildWindow(window, ordered: .above)
        }
        window?.makeKeyAndOrderFront(nil)
        return true
    }

    /// Drill a page aside/window in full-screen over the runner's phone-simulator
    /// device screen, mirroring how a real iOS phone presents it. The surface is a
    /// borderless child window pinned to the whole device frame, clipped to the
    /// device's rounded corners, with a back affordance so the user can return.
    /// Shown eagerly (like iOS); the reconciler is the sole authority for dismiss.
    /// URL asides degrade to the in-app browser upstream, so this is page-only.
    private static func presentPhoneFullScreen(
        id: String,
        appId: String,
        path: String,
        sessionId: UInt64,
        pageInstanceId rawPageInstanceId: String,
        content: Int32,
        context: SurfaceContext
    ) -> Bool {
        let pageInstanceId = rawPageInstanceId.trimmingCharacters(in: .whitespacesAndNewlines)
        if path.isEmpty || pageInstanceId.isEmpty {
            LXLog.error(
                "fullscreen page requires path and pageInstanceId id=\(id) app=\(appId) path=\(path) pageInstanceId=\(pageInstanceId)",
                category: "Surface", appId: appId, path: path
            )
            return false
        }

        let window = makeWindow(kind: kindPopup, frame: context.frame)
        let windowContent = NSView(frame: NSRect(origin: .zero, size: context.frame.size))
        windowContent.wantsLayer = true
        windowContent.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        let hostRadius = hostScreenCornerRadius()
        if hostRadius > 0 {
            windowContent.layer?.cornerRadius = hostRadius
            windowContent.layer?.masksToBounds = true
        }

        let controller = LxAppActiveHost.activeController ?? LxAppController()
        let lxHostView = LxAppHostView(controller: controller)
        lxHostView.translatesAutoresizingMaskIntoConstraints = false
        lxHostView.wantsLayer = true
        lxHostView.layer?.backgroundColor = NSColor.clear.cgColor
        windowContent.addSubview(lxHostView)
        pinToEdges(lxHostView, in: windowContent)

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
        Task { @MainActor in
            do {
                try await lxHostView.mount(session, notifyVisibleOnMount: true)
            } catch {
                LXLog.error(
                    "fullscreen mount failed id=\(id) app=\(appId) path=\(path) error=\(String(describing: error))",
                    category: "Surface", appId: appId, path: path
                )
                _ = close(id: id, appId: appId, reason: "failed")
            }
        }

        addBackAffordance(to: windowContent, id: id, appId: appId)

        let delegate = WindowDelegate(id: id, appId: appId)
        window.contentView = windowContent
        window.delegate = delegate
        entries[id] = Entry(
            id: id,
            appId: appId,
            pageInstanceId: pageInstanceId,
            hostView: lxHostView,
            webView: nil,
            navigationDelegate: nil,
            window: window,
            parentWindow: context.parentWindow,
            delegate: delegate,
            isFullScreen: true
        )
        if let parentWindow = context.parentWindow {
            parentWindow.addChildWindow(window, ordered: .above)
        }
        window.makeKeyAndOrderFront(nil)
        return true
    }

    /// A drill-in back chevron pinned top-left, the phone affordance to dismiss a
    /// full-screen surface and return to the page beneath it.
    private static func addBackAffordance(to content: NSView, id: String, appId: String) {
        let button = SurfaceActionButton(id: id, appId: appId)
        button.translatesAutoresizingMaskIntoConstraints = false
        button.bezelStyle = .circular
        button.isBordered = false
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.black.withAlphaComponent(0.35).cgColor
        button.layer?.cornerRadius = 14
        button.image = NSImage(
            systemSymbolName: "chevron.left",
            accessibilityDescription: "Back"
        )
        button.contentTintColor = .white
        button.imageScaling = .scaleProportionallyDown
        content.addSubview(button)
        NSLayoutConstraint.activate([
            button.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 12),
            button.topAnchor.constraint(equalTo: content.topAnchor, constant: 12),
            button.widthAnchor.constraint(equalToConstant: 28),
            button.heightAnchor.constraint(equalToConstant: 28),
        ])
    }

    /// Phone full-screen surfaces currently on-screen. The reconciler reads this
    /// to dismiss any the core dropped, mirroring the iOS full-screen contract.
    static func presentedFullScreenIds() -> Set<String> {
        Set(
            entries.values
                .filter { $0.isFullScreen && ($0.window?.isVisible ?? false) }
                .map { $0.id }
        )
    }

    @discardableResult
    static func dismissFullScreen(id: String) -> Bool {
        guard let entry = entries[id], entry.isFullScreen else { return false }
        return close(id: id, appId: entry.appId, reason: "programmatic")
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
        let dockedBrowser: DockedBrowser?

        switch content {
        case contentPage:
            pageInstanceId = rawPageInstanceId.trimmingCharacters(in: .whitespacesAndNewlines)
            if path.isEmpty || pageInstanceId.isEmpty {
                LXLog.error(
                    "aside page requires path and pageInstanceId id=\(id) app=\(appId) path=\(path) pageInstanceId=\(pageInstanceId)",
                    category: "Surface", appId: appId, path: path
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
            dockedBrowser = nil
            Task { @MainActor in
                do {
                    try await lxHostView.mount(session, notifyVisibleOnMount: true)
                } catch {
                    LXLog.error(
                        "aside mount failed id=\(id) app=\(appId) path=\(path) error=\(String(describing: error))",
                        category: "Surface", appId: appId, path: path
                    )
                    _ = close(id: id, appId: appId, reason: "failed")
                }
            }

        case contentUrl:
            guard let url = URL(string: path), let scheme = url.scheme?.lowercased(),
                  scheme == "https" || scheme == "http" || scheme == "file" else {
                LXLog.error("invalid web aside url id=\(id) url=\(path)", category: "Surface", appId: appId, path: path)
                return false
            }
            // Every web-aside node is a tab in the single per-window browser
            // panel. The FIRST node anchors the panel (docks its container +
            // registers the shell slot); later nodes just add a tab (deduped by
            // URL) — no second dock. onCloseTab(sid) closes that one tab/node;
            // onCloseAside closes the anchor, which cascades to every tab.
            let onCloseTab: (String) -> Void = { sid in
                _ = LxAppSurface.close(id: sid, appId: appId, reason: "user")
            }
            let onCloseAside: () -> Void = {
                // Close every tab node, non-anchor first; the last close tears
                // the panel down.
                let ids = LxAppActiveHost.activeShell?.browserCoordinator
                    .activeDockedBrowser?.tabSurfaceIds ?? [id]
                for sid in ids.reversed() {
                    _ = LxAppSurface.close(id: sid, appId: appId, reason: "user")
                }
            }
            guard let opened = shell.browserCoordinator.openDockedAsideTab(
                surfaceId: id,
                url: url.absoluteString,
                onCloseTab: onCloseTab,
                onCloseAside: onCloseAside
            ) else {
                LXLog.error("failed to open docked aside tab id=\(id) url=\(path)", category: "Surface", appId: appId, path: path)
                return false
            }
            guard opened.isNew else {
                // Added as a tab to the existing panel — this node owns no dock
                // of its own. Track a lightweight entry so close() removes just
                // this tab (dockedContainer/position nil ⇒ no panel to hide).
                entries[id] = Entry(
                    id: id,
                    appId: appId,
                    pageInstanceId: "",
                    hostView: nil,
                    webView: nil,
                    navigationDelegate: nil,
                    window: nil,
                    parentWindow: nil,
                    delegate: WindowDelegate(id: id, appId: appId),
                    dockedBrowser: opened.browser
                )
                return true
            }
            pageInstanceId = ""
            hostView = nil
            webView = nil
            navigationDelegate = nil
            dockedBrowser = opened.browser
            container.addSubview(opened.browser.containerView)
            opened.browser.containerView.translatesAutoresizingMaskIntoConstraints = false
            pinToEdges(opened.browser.containerView, in: container)

        default:
            LXLog.error("unsupported aside content=\(content) id=\(id) app=\(appId)", category: "Surface", appId: appId, path: path)
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
            dockedContainer: container,
            dockedBrowser: dockedBrowser
        )
        // Only CREATE + REGISTER the aside content here (hidden). The Rust graph
        // commit that follows `present_surface` pushes the layout plan; the
        // reconciler is the sole authority for the aside's edge + visibility.
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
        // Multi-tab browser aside: a tab close removes just that tab. When the
        // closed node is the anchor (dockedContainer set) and other tabs
        // survive, the panel re-anchors to a survivor node; the LAST tab
        // closing tears the panel down.
        var reanchored = false
        if let browser = entry.dockedBrowser {
            if entry.dockedContainer != nil {
                _ = browser.removeTab(surfaceId: id)
                if let survivorId = browser.anchorSurfaceId, let survivor = entries[survivorId] {
                    entries[survivorId] = Entry(
                        id: survivorId,
                        appId: survivor.appId,
                        pageInstanceId: survivor.pageInstanceId,
                        hostView: nil,
                        webView: nil,
                        navigationDelegate: nil,
                        window: nil,
                        parentWindow: nil,
                        delegate: survivor.delegate,
                        dockedPosition: entry.dockedPosition,
                        dockedContainer: entry.dockedContainer,
                        dockedBrowser: browser
                    )
                    if let container = entry.dockedContainer,
                       let position = entry.dockedPosition,
                       let shell = LxAppActiveHost.activeShell {
                        shell.hidePanel(id: id)
                        shell.registerPanelWithNativeContent(
                            id: survivorId,
                            position: position,
                            contentView: container
                        )
                    }
                    reanchored = true
                } else {
                    browser.tearDown()
                    LxAppActiveHost.activeShell?.browserCoordinator.clearDockedBrowser()
                }
            } else if browser.removeTab(surfaceId: id) {
                // Last tab closed through a non-anchor node (anchor entry
                // missing): tear the panel down so no empty dock slot stays.
                browser.tearDown()
                LxAppActiveHost.activeShell?.browserCoordinator.clearDockedBrowser()
                if let (anchorId, anchor) = entries.first(where: {
                    $0.value.dockedBrowser === browser && $0.value.dockedContainer != nil
                }) {
                    entries.removeValue(forKey: anchorId)
                    LxAppActiveHost.activeShell?.hidePanel(id: anchorId)
                    anchor.dockedContainer?.removeFromSuperview()
                    _ = onSurfaceClosed(anchor.appId, anchorId, closeReason(reason))
                }
            }
        }
        if entry.dockedPosition != nil && !reanchored {
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
            LXLog.error("show: surface not found id=\(id) app=\(appId)", category: "Surface", appId: appId)
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
            LXLog.error("show: surface has no window id=\(id) app=\(appId)", category: "Surface", appId: appId)
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
            LXLog.error("hide: surface not found id=\(id) app=\(appId)", category: "Surface", appId: appId)
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
            LXLog.error("hide: surface has no window id=\(id) app=\(appId)", category: "Surface", appId: appId)
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
    static func dismissFloat(id: String) -> Bool {
        guard let entry = entries[id], entry.isFloat else { return false }
        return close(id: id, appId: entry.appId, reason: "programmatic")
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

    /// Rounded-screen corner radius of a controller host (the Runner's phone
    /// simulator), found by walking up from the lxapp render view. Returns 0 for a
    /// desktop shell or a square device shape, so the popup is clipped only when the
    /// host actually has rounded corners.
    private static func hostScreenCornerRadius() -> CGFloat {
        guard LxAppActiveHost.activeShell == nil else { return 0 }
        var view: NSView? = hostAnchorView
        while let current = view {
            if let layer = current.layer, layer.masksToBounds, layer.cornerRadius > 0 {
                return layer.cornerRadius
            }
            view = current.superview
        }
        return 0
    }

    /// Frame of the runner's whole device screen — the rounded screen container,
    /// the nearest masks-to-bounds rounded ancestor of the lxapp render view — so a
    /// phone full-screen surface covers the ENTIRE device frame (under the status /
    /// nav bars), not just the inset render view. Falls back to the render view.
    private static func phoneDeviceScreenContext() -> SurfaceContext? {
        var view: NSView? = hostAnchorView
        while let current = view {
            if let layer = current.layer, layer.masksToBounds, layer.cornerRadius > 0 {
                return contextFrame(for: current)
            }
            view = current.superview
        }
        return contextFrame(for: hostAnchorView)
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

        // Controller-hosted host (e.g. the Runner's phone simulator): bound floats
        // to the lxapp render view so they stay within the device frame.
        if let context = contextFrame(for: hostAnchorView) {
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
    private static let kindWindow: Int32 = 0
    private static let kindPopup: Int32 = 1
    private static let contentPage: Int32 = 0
    private static let contentUrl: Int32 = 1
    // Arbitrated role (mirrors lingxia_platform SurfaceRole). On a phone there is
    // no docking: an aside has no side-by-side room, so the core promotes it to a
    // main (kind Window) and an aside that survives as such (kind Overlay) is
    // still shown full-screen — the same way the primary lxapp page is shown. A
    // float keeps the positioned-popup treatment.
    private static let roleMain: Int32 = 0
    private static let roleAside: Int32 = 1
    private static let roleFloat: Int32 = 2
    private static let transientCornerRadius: CGFloat = 12
    private static var entries: [String: Entry] = [:]

    private final class Entry {
        let id: String
        let appId: String
        let pageInstanceId: String
        /// True when the surface presents full-screen (an aside, or a main the
        /// core promoted from an aside) rather than a positioned float popup.
        /// The layout reconciler tracks the full-screen set to dismiss asides the
        /// core dropped from the plan.
        let isFullScreenSurface: Bool
        /// A float popup (role Float). Floats are lxapp-owned overlays, never
        /// layout-plan members, so the full-screen reconciler must skip them even
        /// when a float fills the screen (size 100%) and so reads as full-screen.
        let isFloat: Bool
        let hostView: LxAppHostView?
        let webView: WKWebView?
        let navigationDelegate: WKNavigationDelegate?
        let window: UIWindow

        init(
            id: String,
            appId: String,
            pageInstanceId: String,
            isFullScreenSurface: Bool,
            isFloat: Bool,
            hostView: LxAppHostView?,
            webView: WKWebView?,
            navigationDelegate: WKNavigationDelegate?,
            window: UIWindow
        ) {
            self.id = id
            self.appId = appId
            self.pageInstanceId = pageInstanceId
            self.isFullScreenSurface = isFullScreenSurface
            self.isFloat = isFloat
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
            guard let url = navigationAction.request.url else {
                decisionHandler(.cancel)
                return
            }
            // A registered URL-callback sentinel (e.g. an auth handoff) is
            // consumed by the waiting Rust channel; cancel the load.
            if urlCallbackDispatch(url.absoluteString) {
                decisionHandler(.cancel)
                return
            }
            guard LxAppSurface.isSameOrigin(initialURL, url) else {
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
        /// True when the surface fills the whole window. This can be an adaptive
        /// aside/main drill-in or an explicitly full-screen float.
        private let fillsScreen: Bool
        /// True for adaptive asides (including asides promoted to mains on
        /// compact width). These should feel like an iOS drill-in page, not an
        /// immersive popup.
        private let usesDrillInChrome: Bool

        init(
            id: String,
            appId: String,
            contentFrame: CGRect,
            fillsScreen: Bool,
            usesDrillInChrome: Bool
        ) {
            self.id = id
            self.appId = appId
            self.contentFrame = contentFrame
            self.fillsScreen = fillsScreen
            self.usesDrillInChrome = usesDrillInChrome
            super.init(nibName: nil, bundle: nil)
            modalPresentationStyle = .overFullScreen
        }

        required init?(coder: NSCoder) {
            nil
        }

        // Adaptive asides are page-like drill-ins, so keep system chrome visible.
        // Only explicitly immersive full-screen floats hide/deflect system UI.
        override var prefersStatusBarHidden: Bool { fillsScreen && !usesDrillInChrome }
        override var prefersHomeIndicatorAutoHidden: Bool { fillsScreen && !usesDrillInChrome }
        override var preferredScreenEdgesDeferringSystemGestures: UIRectEdge {
            fillsScreen && !usesDrillInChrome ? .all : []
        }

        override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = fillsScreen
                ? (usesDrillInChrome ? UIColor.systemBackground : UIColor.clear)
                : UIColor.black.withAlphaComponent(0.45)
            let tap = UITapGestureRecognizer(target: self, action: #selector(closeFromBackdrop))
            tap.delegate = self
            view.addGestureRecognizer(tap)

            // A drill-in aside has an iOS-style way back (the visible Back
            // affordance below, plus this left-edge swipe). An immersive
            // full-screen float draws no SDK chrome, but keeps the same silent
            // left-edge swipe as a last-resort escape — iOS has no system Back
            // and backdrop tap is disabled when full-screen, so without it a
            // float that forgot to draw its own close would trap the user.
            if fillsScreen {
                let edge = UIScreenEdgePanGestureRecognizer(target: self, action: #selector(closeFromEdgeSwipe(_:)))
                edge.edges = .left
                view.addGestureRecognizer(edge)
            }

            contentView.frame = contentFrame
            contentView.backgroundColor = .white
            contentView.layer.cornerRadius = fillsScreen ? 0 : LxAppSurface.transientCornerRadius
            contentView.layer.masksToBounds = true
            contentView.isUserInteractionEnabled = true
            view.addSubview(contentView)

            // Full-screen surfaces have no host chrome. Adaptive asides get a
            // page-like Back affordance; immersive floats draw their own close
            // (the SDK injects none — see the left-edge swipe safety net above).
            if usesDrillInChrome {
                let action = UIButton(type: .system)
                action.translatesAutoresizingMaskIntoConstraints = false
                action.setImage(UIImage(systemName: "chevron.left"), for: .normal)
                action.tintColor = UIColor.label
                action.backgroundColor = UIColor.systemBackground.withAlphaComponent(0.86)
                action.layer.cornerRadius = 18
                action.layer.shadowColor = UIColor.black.cgColor
                action.layer.shadowOpacity = 0.12
                action.layer.shadowRadius = 8
                action.layer.shadowOffset = CGSize(width: 0, height: 2)
                action.accessibilityLabel = "Back"
                action.addTarget(self, action: #selector(closeFullScreen), for: .touchUpInside)
                view.addSubview(action)
                NSLayoutConstraint.activate([
                    action.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 10),
                    action.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: 12),
                    action.widthAnchor.constraint(equalToConstant: 36),
                    action.heightAnchor.constraint(equalToConstant: 36),
                ])
            }
        }

        @objc private func closeFullScreen() {
            _ = LxAppSurface.close(id: id, appId: appId, reason: "user")
        }

        @objc private func closeFromBackdrop() {
            // Full-screen surfaces have no exposed backdrop — disable the
            // tap-to-close affordance so users don't accidentally dismiss by
            // tapping anywhere on the page.
            if fillsScreen { return }
            _ = LxAppSurface.close(id: id, appId: appId, reason: "user")
        }

        @objc private func closeFromEdgeSwipe(_ gesture: UIScreenEdgePanGestureRecognizer) {
            guard gesture.state == .ended else { return }
            let translation = gesture.translation(in: view).x
            let velocity = gesture.velocity(in: view).x
            if translation > max(view.bounds.width * 0.2, 80) || velocity > 700 {
                _ = LxAppSurface.close(id: id, appId: appId, reason: "user")
            }
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
        position: Int32,
        role: Int32
    ) -> Bool {
        // A phone has no side-by-side room, so the core promotes an aside to a
        // main and presents it as a window (kind Window); an aside that survives
        // the arbitration (kind Overlay, role Aside) is likewise shown
        // full-screen — the same way the primary lxapp page is shown. A float
        // (kind Overlay, role Float) keeps the positioned-popup treatment below.
        guard kind == kindPopup || kind == kindWindow else {
            LXLog.error("unsupported mobile surface kind=\(kind) id=\(id) app=\(appId)", category: "Surface", appId: appId, path: path)
            return false
        }
        guard content == contentPage || content == contentUrl else {
            LXLog.error("unsupported surface content=\(content) id=\(id) app=\(appId)", category: "Surface", appId: appId, path: path)
            return false
        }
        guard let windowScene = activeWindowScene() else {
            LXLog.error("no active window scene for surface id=\(id) app=\(appId)", category: "Surface", appId: appId, path: path)
            return false
        }

        if entries[id] != nil {
            entries[id]?.window.makeKeyAndVisible()
            return true
        }

        // An aside (or an aside the core promoted to a main) covers the whole
        // screen, pushed over the primary page like any other full-screen
        // surface. A float popup keeps its requested size/position, and still
        // fills the screen when both ratios reach ~1.0 (matching Android).
        let usesDrillInChrome = kind == kindWindow || role == roleAside
        let isFullScreenSurface = kind == kindWindow
            || role == roleAside
            || (widthRatio.isFinite
                && heightRatio.isFinite
                && widthRatio >= 0.999
                && heightRatio >= 0.999)

        let containerFrame = windowScene.screen.bounds
        let contentFrame = isFullScreenSurface
            ? containerFrame
            : popupFrame(
                width: width,
                height: height,
                widthRatio: widthRatio,
                heightRatio: heightRatio,
                position: position,
                containerFrame: containerFrame
            )
        let controller = PopupViewController(
            id: id,
            appId: appId,
            contentFrame: contentFrame,
            fillsScreen: isFullScreenSurface,
            usesDrillInChrome: usesDrillInChrome
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
                LXLog.error("present page requires path and pageInstanceId id=\(id) app=\(appId)", category: "Surface", appId: appId, path: path)
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
                    LXLog.error("mount failed id=\(id) app=\(appId) path=\(path) error=\(String(describing: error))", category: "Surface", appId: appId, path: path)
                    _ = close(id: id, appId: appId, reason: "failed")
                }
            }

        case contentUrl:
            guard let url = URL(string: path), let scheme = url.scheme?.lowercased(), scheme == "https" || scheme == "http" else {
                LXLog.error("invalid web surface url id=\(id) url=\(path)", category: "Surface", appId: appId, path: path)
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
            isFullScreenSurface: isFullScreenSurface,
            isFloat: role == roleFloat,
            hostView: hostView,
            webView: webView,
            navigationDelegate: navigationDelegate,
            window: window
        )
        window.makeKeyAndVisible()
        return true
    }

    /// Surfaces presented full-screen (asides, and asides the core promoted to a
    /// main) that are currently on-screen. The layout reconciler reads this to
    /// dismiss any full-screen surface the core dropped from the plan — mirroring
    /// the macOS reconciler's desired-set vs presented-set contract.
    static func presentedFullScreenIds() -> Set<String> {
        Set(
            entries.values
                .filter { $0.isFullScreenSurface && !$0.isFloat && !$0.window.isHidden }
                .map { $0.id }
        )
    }

    @discardableResult
    static func dismissFullScreen(id: String) -> Bool {
        guard let entry = entries[id], entry.isFullScreenSurface else { return false }
        return close(id: id, appId: entry.appId, reason: "programmatic")
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
            LXLog.error("show: surface not found id=\(id) app=\(appId)", category: "Surface", appId: appId)
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
            LXLog.error("hide: surface not found id=\(id) app=\(appId)", category: "Surface", appId: appId)
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
        position: Int32,
        role: Int32
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
        _ = role
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
