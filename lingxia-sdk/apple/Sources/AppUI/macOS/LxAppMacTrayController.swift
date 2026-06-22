#if os(macOS)
import AppKit
import OSLog

@MainActor
final class LxAppMacTrayController: NSObject {
    private static let log = OSLog(subsystem: "LingXia", category: "MacTray")

    private let appConfig: LxAppGeneratedAppConfig
    private let uiConfigURL: URL
    private let onActivate: (String) -> Void

    private var statusItems: [String: NSStatusItem] = [:]
    private(set) var defaultActivatorID: String?

    init(
        appConfig: LxAppGeneratedAppConfig,
        uiConfigURL: URL,
        onActivate: @escaping (String) -> Void
    ) {
        self.appConfig = appConfig
        self.uiConfigURL = uiConfigURL
        self.onActivate = onActivate
        super.init()
    }

    func installMenuBarActivators(_ activators: [LxAppUIConfig.Activator]) {
        removeAllStatusItems()
        defaultActivatorID = nil

        for activator in activators {
            let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
            statusItems[activator.id] = statusItem
            if defaultActivatorID == nil {
                defaultActivatorID = activator.id
            }

            guard let button = statusItem.button else { continue }
            button.identifier = NSUserInterfaceItemIdentifier(activator.id)
            button.target = self
            button.action = #selector(statusItemClicked(_:))
            button.toolTip = activator.label ?? activator.id
            button.sendAction(on: [.leftMouseUp, .rightMouseUp, .otherMouseUp])

            if let iconURL = resolvedIconURL(for: activator),
               let image = NSImage(contentsOf: iconURL) {
                image.size = NSSize(width: 18, height: 18)
                image.isTemplate = isTemplateMenuBarIcon(iconURL)
                button.image = image
                button.imagePosition = .imageOnly
            } else {
                os_log(
                    "menubar icon unavailable or unsuitable for activator=%{public}@ icon=%{public}@; using system fallback",
                    log: Self.log,
                    type: .info,
                    activator.id,
                    activator.icon ?? "nil"
                )
                if let fallbackImage = NSImage(systemSymbolName: "app.fill", accessibilityDescription: activator.label) {
                    fallbackImage.isTemplate = true
                    fallbackImage.size = NSSize(width: 16, height: 16)
                    button.image = fallbackImage
                    button.imagePosition = .imageOnly
                } else {
                    button.title = shortMenuBarTitle(for: activator)
                }
            }
        }
    }

    func button(for activatorID: String) -> NSStatusBarButton? {
        statusItems[activatorID]?.button
    }

    func anyButtonContains(screenPoint point: NSPoint) -> Bool {
        statusItems.values.contains { item in
            guard let button = item.button, let window = button.window else {
                return false
            }
            return window.convertToScreen(button.frame).contains(point)
        }
    }

    private func removeAllStatusItems() {
        for item in statusItems.values {
            NSStatusBar.system.removeStatusItem(item)
        }
        statusItems.removeAll()
    }

    @objc private func statusItemClicked(_ sender: NSStatusBarButton) {
        guard let actionID = sender.identifier?.rawValue else { return }
        onActivate(actionID)
    }

    private func shortMenuBarTitle(for activator: LxAppUIConfig.Activator) -> String {
        if let label = activator.label, let first = label.first {
            return String(first)
        }
        if let first = appConfig.productName.first {
            return String(first)
        }
        return "L"
    }

    private func resolvedIconURL(for activator: LxAppUIConfig.Activator) -> URL? {
        guard let icon = activator.icon else { return nil }
        return LxAppAppUIBundleLoader.resolveRelativeResource(icon, baseURL: uiConfigURL)
    }

    private func isTemplateMenuBarIcon(_ url: URL) -> Bool {
        switch url.pathExtension.lowercased() {
        case "pdf", "svg", "svgz":
            return true
        default:
            return false
        }
    }
}
#endif
