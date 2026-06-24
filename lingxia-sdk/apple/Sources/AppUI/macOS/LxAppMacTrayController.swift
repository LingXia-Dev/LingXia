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
    /// activator id → the lxapp id it targets, so tray click/menu events are
    /// delivered only to the owning lxapp (not broadcast to every loaded app).
    private var activatorSurface: [String: String] = [:]
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
        activatorSurface.removeAll()

        for activator in activators {
            let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
            statusItems[activator.id] = statusItem
            activatorSurface[activator.id] = activator.action.surface
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

    // Runtime updates (lx.tray.*). They target the single tray's status item.
    private var trayTitle: String?
    private var trayBadge: String?
    private var jsMenu: NSMenu?
    /// When true (a JS `lx.tray.onClick` handler is registered), a left-click is
    /// delivered to JS instead of running the tray's configured surface action.
    var clickIntercepted = false

    /// lx.tray.show()/hide() — toggle the status item's visibility.
    func setVisible(_ visible: Bool) {
        for item in statusItems.values {
            item.isVisible = visible
        }
    }

    private struct TrayMenuItemSpec: Decodable {
        let label: String?
        let separator: Bool?
        let enabled: Bool?
        let checked: Bool?
    }

    /// lx.tray.setMenu — rebuild the right-click dropdown from a JSON spec. Item
    /// clicks are reported back to JS by index via the app event bus.
    func setMenu(_ json: String) {
        guard let data = json.data(using: .utf8),
              let specs = try? JSONDecoder().decode([TrayMenuItemSpec].self, from: data),
              !specs.isEmpty
        else {
            jsMenu = nil
            return
        }
        let menu = NSMenu()
        menu.autoenablesItems = false
        for (index, spec) in specs.enumerated() {
            if spec.separator == true {
                menu.addItem(.separator())
                continue
            }
            let item = NSMenuItem(
                title: spec.label ?? "",
                action: #selector(jsMenuItemClicked(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.tag = index
            item.isEnabled = spec.enabled ?? true
            item.state = (spec.checked ?? false) ? .on : .off
            menu.addItem(item)
        }
        jsMenu = menu
    }

    @objc private func jsMenuItemClicked(_ sender: NSMenuItem) {
        let appId = activatorSurface[defaultActivatorID ?? ""] ?? ""
        _ = onAppEvent(AppEvent.trayMenuClick, "\(appId):\(sender.tag)")
    }

    func setBadge(_ text: String?) {
        trayBadge = (text?.isEmpty ?? true) ? nil : text
        refreshTrayText()
    }

    func setTitle(_ text: String?) {
        trayTitle = (text?.isEmpty ?? true) ? nil : text
        refreshTrayText()
    }

    func setIcon(_ iconPath: String) {
        guard let id = defaultActivatorID, let button = statusItems[id]?.button else { return }
        guard let url = LxAppAppUIBundleLoader.resolveRelativeResource(iconPath, baseURL: uiConfigURL),
              let image = NSImage(contentsOf: url) else { return }
        image.size = NSSize(width: 18, height: 18)
        image.isTemplate = isTemplateMenuBarIcon(url)
        button.image = image
        refreshTrayText()
    }

    /// macOS status items have no native count badge, so the title and badge are
    /// composited as text beside the icon (idiomatic, like the menu-bar clock).
    private func refreshTrayText() {
        guard let id = defaultActivatorID, let button = statusItems[id]?.button else { return }
        let text = [trayTitle, trayBadge].compactMap { $0 }.joined(separator: " ")
        if text.isEmpty {
            button.title = ""
            button.imagePosition = button.image != nil ? .imageOnly : .noImage
        } else {
            button.title = button.image != nil ? " \(text)" : text
            button.imagePosition = button.image != nil ? .imageLeading : .noImage
        }
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
        let event = NSApp.currentEvent
        let isSecondaryClick =
            event?.type == .rightMouseUp
            || (event?.type == .leftMouseUp && event?.modifierFlags.contains(.control) == true)
        // Right- / control-click shows the JS-provided menu (if any).
        if isSecondaryClick, let menu = jsMenu, let statusItem = statusItems[actionID] {
            statusItem.menu = menu
            statusItem.button?.performClick(nil)
            statusItem.menu = nil
            return
        }
        // Left-click: when JS intercepts (lx.tray.onClick registered) deliver only
        // to JS; otherwise run the configured surface action.
        if clickIntercepted {
            _ = onAppEvent(AppEvent.trayClick, activatorSurface[actionID] ?? "")
        } else {
            onActivate(actionID)
        }
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
