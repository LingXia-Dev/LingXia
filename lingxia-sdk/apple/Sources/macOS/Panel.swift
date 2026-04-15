#if os(macOS)
import AppKit

// MARK: - Panel Control (macOS)

extension macOSLxApp {

    // MARK: - Toggle Entry Point

    internal static func togglePanel(id: String) {
        LxAppMacAppUIRuntime.active?.toggleManagedSurface(id: id)
    }

    // MARK: - Rust Callback Routing

    @MainActor
    internal static func handlePanelLxAppOpened(
        appId: String,
        path: String,
        sessionId: UInt64,
        panelId: String
    ) -> Bool {
        LxAppMacAppUIRuntime.handlePanelLxAppOpened(
            appId: appId,
            path: path,
            sessionId: sessionId,
            panelId: panelId
        )
    }

    // MARK: - Show / Hide (internal)

    internal static func hidePanel(id: String) {
        activeShell()?.hidePanel(id: id)
    }
}

#endif
