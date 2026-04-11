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

    internal static func togglePanel(id: String) {
        guard let s = shell else { return }
        guard panelItems.contains(where: { $0.id == id }) else { return }

        if s.workspaceManager.isPanelVisible(id: id) {
            s.hidePanel(id: id)
        } else {
            _ = onAppEvent(AppEvent.panelIconClick, id)
        }
    }

    // MARK: - Rust Callback Routing

    @MainActor
    internal static func handlePanelLxAppOpened(
        appId: String,
        path: String,
        sessionId: UInt64,
        panelId: String
    ) -> Bool {
        guard let s = shell else { return false }

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
        s.appSessions[appId] = sessionId
        LxAppCore.setSessionId(sessionId, for: appId)

        let pos = PanelPosition(rawValue: item.position) ?? .right
        s.showPanelWithContent(id: item.id, position: pos, appId: appId, path: path)
        applyResolvedIcon(panelId: item.id, to: s)
        return true
    }

    private static func applyResolvedIcon(panelId: String, to shell: LxAppShell) {
        guard let item = panelItems.first(where: { $0.id == panelId }),
              !item.icon.isEmpty,
              let fileUrl = resolveLxUri(item.appId, item.icon)?.toString() else { return }
        shell.sidebarView?.updatePanelIcon(panelId: panelId, iconFileUrl: fileUrl)
    }

    // MARK: - Show / Hide (internal)

    internal static func hidePanel(id: String) {
        shell?.hidePanel(id: id)
    }
}

// MARK: - PanelItemConfig

struct PanelItemConfig {
    let id: String
    let label: String
    let icon: String
    let position: String
    let appId: String
    let path: String

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
