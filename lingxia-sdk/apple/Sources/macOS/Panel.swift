#if os(macOS)
import AppKit
import os.log
import CLingXiaRustAPI

// MARK: - Panel Control (macOS)

extension macOSLxApp {

    // MARK: - Panel State

    /// Panel items parsed from app.json panels config. Used by SidebarView.
    internal static var panelItems: [PanelItemConfig] = []

    /// Load panel config from the cached JSON (called once after init).
    internal static func loadPanelConfig() {
        guard let json = LxAppCore.panelsConfigJson,
              let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let rawItems = obj["items"] as? [[String: Any]] else {
            return
        }
        panelItems = rawItems.compactMap { PanelItemConfig(json: $0) }
    }

    // MARK: - Toggle Entry Point

    /// Called when a panel icon button is clicked.
    /// - If visible: hide the panel.
    /// - If hidden: send host-scoped event to Rust; Rust chooses target lxapp and opens it
    ///   with presentation=panel. Swift only handles rendering.
    public static func togglePanel(id: String) {
        guard let controller = tabWindowController else { return }
        guard panelItems.contains(where: { $0.id == id }) else { return }

        if controller.workspaceManager.isPanelVisible(id: id) {
            controller.hidePanel(id: id)
        } else {
            _ = onAppEvent(AppEvent.panelIconClick, id)
        }
    }

    // MARK: - Rust Callback Routing

    /// Called from LxAppCore.executeOpenLxApp when Rust requested panel presentation.
    /// Maps panelId (preferred) / appId to configured panel item, then attaches WebView and shows panel.
    @MainActor
    internal static func handlePanelLxAppOpened(
        appId: String,
        path: String,
        sessionId: UInt64,
        panelId: String
    ) -> Bool {
        guard let controller = tabWindowController else { return false }

        let resolvedItem: PanelItemConfig?
        if !panelId.isEmpty {
            resolvedItem = panelItems.first(where: { $0.id == panelId })
        } else {
            resolvedItem = panelItems.first(where: { $0.appId == appId })
        }

        guard let item = resolvedItem else {
            os_log(
                "handlePanelLxAppOpened: no panel configured for panelId=%{public}@ appId=%{public}@",
                type: .error,
                panelId,
                appId
            )
            return false
        }
        // Register session so showPanelWithContent can look up the WebView
        controller.appSessions[appId] = sessionId
        LxAppCore.setSessionId(sessionId, for: appId)

        let pos = PanelPosition(rawValue: item.position) ?? .right
        controller.showPanelWithContent(id: item.id, position: pos, appId: appId, path: path)
        applyResolvedIcon(panelId: item.id, to: controller)
        return true
    }

    /// Resolve icon from the installed lxapp package and update the sidebar button.
    /// icon is a relative path within the lxapp package; resolveLxUri converts it to file://.
    private static func applyResolvedIcon(panelId: String, to controller: LxAppWindowController) {
        guard let item = panelItems.first(where: { $0.id == panelId }),
              !item.icon.isEmpty,
              let fileUrl = resolveLxUri(item.appId, item.icon)?.toString() else { return }
        controller.sidebarView?.updatePanelIcon(panelId: panelId, iconFileUrl: fileUrl)
    }

    // MARK: - Show / Hide (public API, delegates to window controller)

    /// Show a panel with WebView content at the given position.
    public static func showPanel(id: String, position: PanelPosition, appId: String, path: String) {
        tabWindowController?.showPanelWithContent(id: id, position: position, appId: appId, path: path)
    }

    /// Hide a panel by ID.
    public static func hidePanel(id: String) {
        tabWindowController?.hidePanel(id: id)
    }
}

// MARK: - LxAppWindowController panel methods

extension LxAppWindowController {
    private static let panelAttachMaxRetry = 40
    private static let panelAttachRetryDelay: TimeInterval = 0.05

    /// Show a panel with WebView content. Registers the panel if not already registered.
    func showPanelWithContent(id: String, position: PanelPosition, appId: String, path: String) {
        if !workspaceManager.isPanelRegistered(id: id) {
            let config = PanelConfig(id: id, position: position)
            workspaceManager.registerPanel(config)
        }

        workspaceManager.showPanel(id: id)
        attachPanelWebViewWhenReady(panelId: id, appId: appId, path: path, attempt: 0)
    }

    func hidePanel(id: String) {
        workspaceManager.hidePanel(id: id)
    }

    func togglePanel(id: String) {
        workspaceManager.togglePanel(id: id)
    }

    private func attachPanelWebViewWhenReady(panelId: String, appId: String, path: String, attempt: Int) {
        guard let sessionId = appSessions[appId],
              let container = workspaceManager.panelContainer(id: panelId) else {
            return
        }

        if let webView = WebViewManager.findWebView(appId: appId, path: path, sessionId: sessionId) {
            WebViewManager.attachWebViewToContainer(webView, container: container)
            return
        }

        guard attempt < Self.panelAttachMaxRetry else {
            os_log("panel webview attach timed out for panel=%{public}@ appId=%{public}@ path=%{public}@",
                   type: .error, panelId, appId, path)
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + Self.panelAttachRetryDelay) { [weak self] in
            self?.attachPanelWebViewWhenReady(panelId: panelId, appId: appId, path: path, attempt: attempt + 1)
        }
    }
}

// MARK: - PanelItemConfig

/// Lightweight description of a configured panel item, parsed from app.json.
public struct PanelItemConfig {
    public let id: String
    public let label: String
    public let icon: String
    /// PanelPosition raw value: "left", "right", or "bottom"
    public let position: String
    /// The lxapp appId this panel opens.
    public let appId: String
    /// The page path this panel displays within the lxapp. Empty means use the app's initial route.
    public let path: String

    init?(json: [String: Any]) {
        guard let id = json["id"] as? String,
              let label = json["label"] as? String,
              let icon = json["icon"] as? String,
              let content = json["content"] as? [String: Any],
              let appId = content["appId"] as? String else { return nil }
        self.id = id
        self.label = label
        self.icon = icon
        self.position = json["position"] as? String ?? "right"
        self.appId = appId
        self.path = content["path"] as? String ?? ""
    }
}

#endif
