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

        init(
            id: String,
            appId: String,
            pageInstanceId: String,
            hostView: LxAppHostView?,
            webView: WKWebView?,
            navigationDelegate: WKNavigationDelegate?,
            window: NSWindow?,
            parentWindow: NSWindow?,
            delegate: WindowDelegate
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
        position: Int32
    ) -> Bool {
        if let existing = entries[id] {
            existing.window?.makeKeyAndOrderFront(nil)
            return true
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

        entries[id] = Entry(
            id: id,
            appId: appId,
            pageInstanceId: pageInstanceId,
            hostView: hostView,
            webView: webView,
            navigationDelegate: navigationDelegate,
            window: window,
            parentWindow: context.parentWindow,
            delegate: delegate
        )

        if kind != kindWindow, let parentWindow = context.parentWindow, let window {
            parentWindow.addChildWindow(window, ordered: .above)
        }
        window?.makeKeyAndOrderFront(nil)
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
        entry.window?.delegate = nil
        if let window = entry.window {
            entry.parentWindow?.removeChildWindow(window)
            window.contentView = nil
            window.orderOut(nil)
        }
        _ = onSurfaceClosed(appId, id, closeReason(reason))
        return true
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
            window.styleMask.insert(.fullSizeContentView)
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
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
        case "user", "programmatic", "owner_closed", "app_closed", "failed", "unknown":
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

        init(id: String, appId: String, contentFrame: CGRect) {
            self.id = id
            self.appId = appId
            self.contentFrame = contentFrame
            super.init(nibName: nil, bundle: nil)
            modalPresentationStyle = .overFullScreen
        }

        required init?(coder: NSCoder) {
            nil
        }

        override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = UIColor.black.withAlphaComponent(0.45)
            let tap = UITapGestureRecognizer(target: self, action: #selector(closeFromBackdrop))
            tap.delegate = self
            view.addGestureRecognizer(tap)

            contentView.frame = contentFrame
            contentView.backgroundColor = .white
            contentView.layer.cornerRadius = LxAppSurface.transientCornerRadius
            contentView.layer.masksToBounds = true
            contentView.isUserInteractionEnabled = true
            view.addSubview(contentView)
        }

        @objc private func closeFromBackdrop() {
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
        let controller = PopupViewController(id: id, appId: appId, contentFrame: contentFrame)
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
        case "user", "programmatic", "owner_closed", "app_closed", "failed", "unknown":
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
}

#endif
