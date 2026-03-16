#if os(iOS)
import UIKit
import WebKit
import OSLog
import CLingXiaRustAPI
import CLingXiaSwiftAPI

@MainActor
public enum PopupDisplayPosition {
    case center
    case bottom
    case left
    case right
}

@MainActor
public final class LxAppPopup {
    private static let log = OSLog(subsystem: "LingXia", category: "Popup")

    private static var overlayView: UIView?
    private static var popupContainer: UIView?
    private static var popupWebView: WKWebView?
    private static var currentAppId: String?
    private static var currentPath: String?

    private struct LayoutResult {
        let width: CGFloat
        let height: CGFloat
        let isFullWidth: Bool
        let isFullHeight: Bool
    }

    public static func showPopup(
        appId: String,
        path: String,
        widthRatio: Double,
        heightRatio: Double,
        position: PopupDisplayPosition
    ) -> Bool {
        if let existingOverlay = overlayView {
            existingOverlay.removeFromSuperview()
            cleanupPopup()
        }

        guard let manager = iOSLxApp.getInstance().currentLxAppManager else {
            os_log("showPopup failed: no active LxAppViewController", log: log, type: .error)
            return false
        }

        manager.view.layoutIfNeeded()
        guard let rootContainer = manager.rootContainer else {
            os_log("showPopup failed: root container not available", log: log, type: .error)
            return false
        }

        guard let webView = WebViewManager.findWebView(appId: appId, path: path) else {
            return false
        }

        let layout = resolveLayout(
            widthRatio: widthRatio,
            heightRatio: heightRatio,
            position: position,
            container: rootContainer
        )

        let overlay = UIView()
        overlay.translatesAutoresizingMaskIntoConstraints = false
        overlay.backgroundColor = .clear
        overlay.accessibilityViewIsModal = true

        let maskView = UIControl()
        maskView.translatesAutoresizingMaskIntoConstraints = false
        maskView.backgroundColor = UIColor(white: 0, alpha: 0.45)
        maskView.addTarget(self, action: #selector(maskTapped), for: .touchUpInside)
        overlay.addSubview(maskView)

        let container = UIView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.backgroundColor = .clear
        overlay.addSubview(container)

        rootContainer.addSubview(overlay)

        NSLayoutConstraint.activate([
            overlay.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            overlay.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            overlay.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            overlay.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor),

            maskView.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
            maskView.trailingAnchor.constraint(equalTo: overlay.trailingAnchor),
            maskView.topAnchor.constraint(equalTo: overlay.topAnchor),
            maskView.bottomAnchor.constraint(equalTo: overlay.bottomAnchor)
        ])

        let safeArea = overlay.safeAreaLayoutGuide
        if layout.isFullWidth {
            NSLayoutConstraint.activate([
                container.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
                container.trailingAnchor.constraint(equalTo: overlay.trailingAnchor)
            ])
        } else {
            NSLayoutConstraint.activate([
                container.widthAnchor.constraint(equalToConstant: layout.width)
            ])
            switch position {
            case .left:
                NSLayoutConstraint.activate([
                    container.leadingAnchor.constraint(equalTo: overlay.leadingAnchor)
                ])
            case .right:
                NSLayoutConstraint.activate([
                    container.trailingAnchor.constraint(equalTo: overlay.trailingAnchor)
                ])
            default:
                NSLayoutConstraint.activate([
                    container.centerXAnchor.constraint(equalTo: safeArea.centerXAnchor)
                ])
            }
        }

        let containerHeight = layout.height
        if layout.isFullHeight {
            NSLayoutConstraint.activate([
                container.topAnchor.constraint(equalTo: overlay.topAnchor),
                container.bottomAnchor.constraint(equalTo: overlay.bottomAnchor)
            ])
        } else {
            NSLayoutConstraint.activate([
                container.heightAnchor.constraint(equalToConstant: containerHeight)
            ])
        }

        switch position {
        case .bottom:
            NSLayoutConstraint.activate([
                container.bottomAnchor.constraint(equalTo: overlay.bottomAnchor)
            ])
        case .center:
            NSLayoutConstraint.activate([
                container.centerYAnchor.constraint(equalTo: safeArea.centerYAnchor)
            ])
        case .left, .right:
            NSLayoutConstraint.activate([
                container.centerYAnchor.constraint(equalTo: safeArea.centerYAnchor)
            ])
            if !layout.isFullHeight {
                NSLayoutConstraint.activate([
                    container.topAnchor.constraint(greaterThanOrEqualTo: safeArea.topAnchor, constant: 16),
                    container.bottomAnchor.constraint(lessThanOrEqualTo: safeArea.bottomAnchor, constant: -16)
                ])
            }
        }

        let sheetView = UIView()
        sheetView.translatesAutoresizingMaskIntoConstraints = false
        sheetView.backgroundColor = UIColor.white
        applyCornerStyle(to: sheetView, position: position, isFullHeight: layout.isFullHeight)
        sheetView.clipsToBounds = true
        container.addSubview(sheetView)

        NSLayoutConstraint.activate([
            sheetView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            sheetView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            sheetView.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])

        sheetView.topAnchor.constraint(equalTo: container.topAnchor).isActive = true

        webView.removeFromSuperview()
        webView.translatesAutoresizingMaskIntoConstraints = false
        WebViewManager.configureWebViewTransparency(webView, transparent: false)
        sheetView.addSubview(webView)
        NSLayoutConstraint.activate([
            webView.leadingAnchor.constraint(equalTo: sheetView.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: sheetView.trailingAnchor),
            webView.topAnchor.constraint(equalTo: sheetView.topAnchor),
            webView.bottomAnchor.constraint(equalTo: sheetView.bottomAnchor)
        ])
        webView.resumeWebView()

        overlay.layoutIfNeeded()

        popupWebView = webView
        overlayView = overlay
        popupContainer = container
        currentAppId = appId
        currentPath = path

        lingxia.onPageShow(appId, path)
        return true
    }

    public static func hidePopup(appId: String) -> Bool {
        if let activeAppId = currentAppId, !activeAppId.isEmpty, activeAppId != appId {
            os_log("hidePopup called with mismatched appId (expected %{public}@, got %{public}@)", log: log, type: .info, activeAppId, appId)
        }

        guard let overlay = overlayView else {
            return true
        }

        overlay.removeFromSuperview()
        cleanupPopup()
        return true
    }

    private static func cleanupPopup() {
        popupWebView?.pauseWebView()
        popupWebView?.removeFromSuperview()
        popupWebView = nil
        popupContainer = nil
        overlayView = nil
        currentAppId = nil
        currentPath = nil
    }

    private static func resolveLayout(
        widthRatio: Double,
        heightRatio: Double,
        position: PopupDisplayPosition,
        container: UIView
    ) -> LayoutResult {
        container.layoutIfNeeded()
        var containerSize = container.bounds.size
        if containerSize.width <= 0 || containerSize.height <= 0 {
            containerSize = UIScreen.main.bounds.size
        }
        let sanitizedWidth = sanitizeFraction(widthRatio)
        let sanitizedHeight = sanitizeFraction(heightRatio)

        let availableWidth = containerSize.width
        let availableHeight = containerSize.height

        let width: CGFloat
        if sanitizedWidth >= 0.999 {
            width = availableWidth
        } else {
            let computed = CGFloat(sanitizedWidth) * availableWidth
            let bounded = min(computed, max(availableWidth - 32, 0))
            width = min(availableWidth, max(160, bounded))
        }

        let height: CGFloat
        if sanitizedHeight >= 0.999 {
            height = availableHeight
        } else {
            let computed = CGFloat(sanitizedHeight) * availableHeight
            let bounded = min(computed, availableHeight)
            height = min(availableHeight, max(160, bounded))
        }

        return LayoutResult(
            width: width,
            height: height,
            isFullWidth: sanitizedWidth >= 0.999,
            isFullHeight: sanitizedHeight >= 0.999
        )
    }

    private static func sanitizeFraction(_ value: Double) -> Double {
        if value.isNaN || !value.isFinite {
            return 1.0
        }
        return min(max(value, 0.0), 1.0)
    }

    private static func applyCornerStyle(
        to view: UIView,
        position: PopupDisplayPosition,
        isFullHeight: Bool
    ) {
        let radius: CGFloat = 16

        if radius <= 0 {
            view.layer.cornerRadius = 0
            if #available(iOS 11.0, *) {
                view.layer.maskedCorners = []
            }
            view.layer.masksToBounds = false
            return
        }

        let shouldClip = !(isFullHeight && position == .bottom)
        view.layer.cornerRadius = shouldClip ? radius : 0
        if #available(iOS 11.0, *) {
            if shouldClip {
                switch position {
                case .bottom:
                    view.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
                case .center:
                    view.layer.maskedCorners = [
                        .layerMinXMinYCorner,
                        .layerMaxXMinYCorner,
                        .layerMinXMaxYCorner,
                        .layerMaxXMaxYCorner
                    ]
                case .left:
                    view.layer.maskedCorners = [
                        .layerMaxXMinYCorner,
                        .layerMaxXMaxYCorner
                    ]
                case .right:
                    view.layer.maskedCorners = [
                        .layerMinXMinYCorner,
                        .layerMinXMaxYCorner
                    ]
                }
            } else {
                view.layer.maskedCorners = []
            }
        }
        view.layer.masksToBounds = shouldClip
    }

    @objc
    private static func maskTapped() {
        // Intentionally left blank to swallow touches outside the popup content.
    }
}

extension PopupPositionBridge {
    func toDisplayPosition() -> PopupDisplayPosition {
        switch self {
        case .Center:
            return .center
        case .Bottom:
            return .bottom
        case .Left:
            return .left
        case .Right:
            return .right
        @unknown default:
            return .bottom
        }
    }
}
#endif

#if os(macOS)
import AppKit
import WebKit
import OSLog
import CLingXiaSwiftAPI

@MainActor
public enum PopupDisplayPosition {
    case center
    case bottom
    case left
    case right
}

@MainActor
public final class LxAppPopup {
    private static let log = OSLog(subsystem: "LingXia", category: "Popup")

    private static var overlayView: NSView?
    private static var popupContainer: NSView?
    private static var popupWebView: WKWebView?
    private static var currentAppId: String?
    private static var currentPath: String?

    private struct LayoutResult {
        let width: CGFloat
        let height: CGFloat
        let isFullWidth: Bool
        let isFullHeight: Bool
    }

    public static func showPopup(
        appId: String,
        path: String,
        widthRatio: Double,
        heightRatio: Double,
        position: PopupDisplayPosition
    ) -> Bool {
        if let existingOverlay = overlayView {
            existingOverlay.removeFromSuperview()
            cleanupPopup()
        }

        guard let webView = WebViewManager.findWebView(appId: appId, path: path) else {
            os_log("showPopup failed: WebView not found for %{public}@:%{public}@", log: log, type: .error, appId, path)
            return false
        }

        let rootContainer: NSView
        if let panelView = macOSLxApp.contentPanelView {
            rootContainer = panelView
        } else if let fallbackContainer = webView.window?.contentView {
            rootContainer = fallbackContainer
        } else {
            os_log("showPopup failed: no usable window container", log: log, type: .error)
            return false
        }

        rootContainer.layoutSubtreeIfNeeded()

        let layout = resolveLayout(
            widthRatio: widthRatio,
            heightRatio: heightRatio,
            position: position,
            containerSize: rootContainer.bounds.size
        )

        // Create overlay
        let overlay = NSView()
        overlay.translatesAutoresizingMaskIntoConstraints = false
        overlay.wantsLayer = true

        // Create semi-transparent mask
        let maskView = PopupMaskView()
        maskView.translatesAutoresizingMaskIntoConstraints = false
        maskView.wantsLayer = true
        maskView.layer?.backgroundColor = NSColor(white: 0, alpha: 0.45).cgColor
        overlay.addSubview(maskView)

        // Create popup container
        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.wantsLayer = true
        overlay.addSubview(container)

        rootContainer.addSubview(overlay)

        // Overlay fills root container
        NSLayoutConstraint.activate([
            overlay.leadingAnchor.constraint(equalTo: rootContainer.leadingAnchor),
            overlay.trailingAnchor.constraint(equalTo: rootContainer.trailingAnchor),
            overlay.topAnchor.constraint(equalTo: rootContainer.topAnchor),
            overlay.bottomAnchor.constraint(equalTo: rootContainer.bottomAnchor),

            maskView.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
            maskView.trailingAnchor.constraint(equalTo: overlay.trailingAnchor),
            maskView.topAnchor.constraint(equalTo: overlay.topAnchor),
            maskView.bottomAnchor.constraint(equalTo: overlay.bottomAnchor),
        ])

        // Container width
        if layout.isFullWidth {
            NSLayoutConstraint.activate([
                container.leadingAnchor.constraint(equalTo: overlay.leadingAnchor),
                container.trailingAnchor.constraint(equalTo: overlay.trailingAnchor),
            ])
        } else {
            container.widthAnchor.constraint(equalToConstant: layout.width).isActive = true
            switch position {
            case .left:
                container.leadingAnchor.constraint(equalTo: overlay.leadingAnchor).isActive = true
            case .right:
                container.trailingAnchor.constraint(equalTo: overlay.trailingAnchor).isActive = true
            default:
                container.centerXAnchor.constraint(equalTo: overlay.centerXAnchor).isActive = true
            }
        }

        // Container height
        if layout.isFullHeight {
            NSLayoutConstraint.activate([
                container.topAnchor.constraint(equalTo: overlay.topAnchor),
                container.bottomAnchor.constraint(equalTo: overlay.bottomAnchor),
            ])
        } else {
            container.heightAnchor.constraint(equalToConstant: layout.height).isActive = true
        }

        // Container vertical position
        switch position {
        case .bottom:
            container.bottomAnchor.constraint(equalTo: overlay.bottomAnchor).isActive = true
        case .center:
            container.centerYAnchor.constraint(equalTo: overlay.centerYAnchor).isActive = true
        case .left, .right:
            container.centerYAnchor.constraint(equalTo: overlay.centerYAnchor).isActive = true
            if !layout.isFullHeight {
                NSLayoutConstraint.activate([
                    container.topAnchor.constraint(greaterThanOrEqualTo: overlay.topAnchor, constant: 16),
                    container.bottomAnchor.constraint(lessThanOrEqualTo: overlay.bottomAnchor, constant: -16),
                ])
            }
        }

        // Create sheet view with rounded corners
        let sheetView = NSView()
        sheetView.translatesAutoresizingMaskIntoConstraints = false
        sheetView.wantsLayer = true
        sheetView.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        applyCornerStyle(to: sheetView, position: position, isFullHeight: layout.isFullHeight)
        container.addSubview(sheetView)

        NSLayoutConstraint.activate([
            sheetView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            sheetView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            sheetView.topAnchor.constraint(equalTo: container.topAnchor),
            sheetView.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])

        // Add WebView to sheet
        webView.removeFromSuperview()
        webView.translatesAutoresizingMaskIntoConstraints = false
        WebViewManager.configureWebViewTransparency(webView, transparent: false)
        sheetView.addSubview(webView)
        NSLayoutConstraint.activate([
            webView.leadingAnchor.constraint(equalTo: sheetView.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: sheetView.trailingAnchor),
            webView.topAnchor.constraint(equalTo: sheetView.topAnchor),
            webView.bottomAnchor.constraint(equalTo: sheetView.bottomAnchor),
        ])
        webView.resumeWebView()

        overlay.layoutSubtreeIfNeeded()

        popupWebView = webView
        overlayView = overlay
        popupContainer = container
        currentAppId = appId
        currentPath = path

        lingxia.onPageShow(appId, path)
        return true
    }

    public static func hidePopup(appId: String) -> Bool {
        if let activeAppId = currentAppId, !activeAppId.isEmpty, activeAppId != appId {
            os_log("hidePopup called with mismatched appId (expected %{public}@, got %{public}@)", log: log, type: .info, activeAppId, appId)
        }

        guard let overlay = overlayView else {
            return true
        }

        overlay.removeFromSuperview()
        cleanupPopup()
        return true
    }

    private static func cleanupPopup() {
        popupWebView?.pauseWebView()
        popupWebView?.removeFromSuperview()
        popupWebView = nil
        popupContainer = nil
        overlayView = nil
        currentAppId = nil
        currentPath = nil
    }

    private static func resolveLayout(
        widthRatio: Double,
        heightRatio: Double,
        position: PopupDisplayPosition,
        containerSize: NSSize
    ) -> LayoutResult {
        var size = containerSize
        if size.width <= 0 || size.height <= 0 {
            size = NSScreen.main?.frame.size ?? NSSize(width: 1200, height: 800)
        }
        let sanitizedWidth = sanitizeFraction(widthRatio)
        let sanitizedHeight = sanitizeFraction(heightRatio)

        let width: CGFloat
        if sanitizedWidth >= 0.999 {
            width = size.width
        } else {
            let computed = CGFloat(sanitizedWidth) * size.width
            let bounded = min(computed, max(size.width - 32, 0))
            width = min(size.width, max(160, bounded))
        }

        let height: CGFloat
        if sanitizedHeight >= 0.999 {
            height = size.height
        } else {
            let computed = CGFloat(sanitizedHeight) * size.height
            let bounded = min(computed, size.height)
            let minHeight: CGFloat = 160
            var resolvedHeight = min(size.height, max(minHeight, bounded))
            if position == .left || position == .right {
                let sideInset: CGFloat = 16
                let maxSideHeight = max(size.height - sideInset * 2, 1)
                resolvedHeight = min(resolvedHeight, maxSideHeight)
            }
            height = resolvedHeight
        }

        return LayoutResult(
            width: width,
            height: height,
            isFullWidth: sanitizedWidth >= 0.999,
            isFullHeight: sanitizedHeight >= 0.999
        )
    }

    private static func sanitizeFraction(_ value: Double) -> Double {
        if value.isNaN || !value.isFinite {
            return 1.0
        }
        return min(max(value, 0.0), 1.0)
    }

    private static func applyCornerStyle(
        to view: NSView,
        position: PopupDisplayPosition,
        isFullHeight: Bool
    ) {
        guard let layer = view.layer else { return }
        let radius: CGFloat = 16

        let shouldClip = !(isFullHeight && position == .bottom)
        layer.cornerRadius = shouldClip ? radius : 0

        if shouldClip {
            switch position {
            case .bottom:
                // macOS coordinates: minY=bottom, maxY=top
                layer.maskedCorners = [.layerMinXMaxYCorner, .layerMaxXMaxYCorner]
            case .center:
                layer.maskedCorners = [
                    .layerMinXMinYCorner, .layerMaxXMinYCorner,
                    .layerMinXMaxYCorner, .layerMaxXMaxYCorner,
                ]
            case .left:
                layer.maskedCorners = [.layerMaxXMinYCorner, .layerMaxXMaxYCorner]
            case .right:
                layer.maskedCorners = [.layerMinXMinYCorner, .layerMinXMaxYCorner]
            }
        } else {
            layer.maskedCorners = []
        }
        layer.masksToBounds = shouldClip
    }
}

/// Custom NSView to intercept mouse events on the popup mask
@MainActor
private class PopupMaskView: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? {
        self
    }

    override func mouseDown(with event: NSEvent) {
        // Swallow mouse event to prevent interaction with content behind the popup.
    }

    override func mouseUp(with event: NSEvent) {
        // Swallow mouse event to prevent interaction with content behind the popup.
    }

    override func rightMouseDown(with event: NSEvent) {
        // Swallow mouse event to prevent interaction with content behind the popup.
    }

    override func rightMouseUp(with event: NSEvent) {
        // Swallow mouse event to prevent interaction with content behind the popup.
    }

    override func otherMouseDown(with event: NSEvent) {
        // Swallow mouse event to prevent interaction with content behind the popup.
    }

    override func otherMouseUp(with event: NSEvent) {
        // Swallow mouse event to prevent interaction with content behind the popup.
    }

    override func mouseDragged(with event: NSEvent) {
        // Swallow drag event to prevent interaction with content behind the popup.
    }

    override func rightMouseDragged(with event: NSEvent) {
        // Swallow drag event to prevent interaction with content behind the popup.
    }

    override func otherMouseDragged(with event: NSEvent) {
        // Swallow drag event to prevent interaction with content behind the popup.
    }

    override func scrollWheel(with event: NSEvent) {
        // Swallow scroll event to prevent interaction with content behind the popup.
    }

    override func magnify(with event: NSEvent) {
        // Swallow gesture event to prevent interaction with content behind the popup.
    }

    override func smartMagnify(with event: NSEvent) {
        // Swallow gesture event to prevent interaction with content behind the popup.
    }

    override func rotate(with event: NSEvent) {
        // Swallow gesture event to prevent interaction with content behind the popup.
    }

    override func swipe(with event: NSEvent) {
        // Swallow gesture event to prevent interaction with content behind the popup.
    }
}

extension PopupPositionBridge {
    func toDisplayPosition() -> PopupDisplayPosition {
        switch self {
        case .Center:
            return .center
        case .Bottom:
            return .bottom
        case .Left:
            return .left
        case .Right:
            return .right
        @unknown default:
            return .bottom
        }
    }
}
#endif
