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
    private var baseToolTip: String?

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
            if defaultActivatorID == activator.id {
                baseToolTip = button.toolTip
            }
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
    /// Exclusive menu-bar agents get Quit when JS did not register a menu.
    var includeDefaultQuit = false
    /// Hide the flyout before a context menu appears, matching Windows.
    var onWillShowMenu: (() -> Void)?
    /// Exclusive-tray host has a staged update. Menu + badge are the prompt;
    /// the window-anchored callout is not shown.
    private var updateReady = false
    private var updateOpensStore = false
    private var onUpdateReady: (() -> Void)?

    /// Take the post-download prompt for an exclusive-tray host.
    func presentUpdateReady(openStore: Bool = false, onOpen: @escaping () -> Void) {
        updateReady = true
        updateOpensStore = openStore
        onUpdateReady = onOpen
        refreshTrayText()
    }

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

    private func contextMenu() -> NSMenu? {
        let menu: NSMenu
        if let jsMenu, let copy = jsMenu.copy() as? NSMenu {
            menu = copy
        } else if includeDefaultQuit {
            menu = NSMenu()
            menu.autoenablesItems = false
            let item = NSMenuItem(
                title: L10n.string("lx_app_quit", appConfig.productName),
                action: #selector(quitFromMenu),
                keyEquivalent: "q"
            )
            item.target = self
            menu.addItem(item)
        } else if updateReady {
            menu = NSMenu()
            menu.autoenablesItems = false
        } else {
            return nil
        }
        if updateReady {
            let item = NSMenuItem(
                title: L10n.string(updateOpensStore ? "lx_update_card_open_store" : "lx_update_card_restart"),
                action: #selector(updateReadyClicked),
                keyEquivalent: ""
            )
            item.target = self
            menu.insertItem(item, at: 0)
            if menu.items.count > 1 {
                menu.insertItem(.separator(), at: 1)
            }
        }
        return menu
    }

    @objc private func quitFromMenu() {
        NSApp.terminate(nil)
    }

    @objc private func updateReadyClicked() {
        onUpdateReady?()
    }

    /// Reports whether a status item actually took the value. A product whose
    /// tray never materialised has nothing to badge, and saying otherwise is
    /// what `lx.app.setBadge`'s return value exists to stop.
    @discardableResult
    func setBadge(_ text: String?) -> Bool {
        trayBadge = (text?.isEmpty ?? true) ? nil : text
        return refreshTrayText()
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
    @discardableResult
    private func refreshTrayText() -> Bool {
        guard let id = defaultActivatorID, let item = statusItems[id], let button = item.button else {
            return false
        }
        let marker = (updateReady && trayTitle == nil && trayBadge == nil) ? "●" : nil
        let text = [trayTitle, trayBadge, marker].compactMap { $0 }.joined(separator: " ")
        if text.isEmpty {
            item.length = NSStatusItem.squareLength
            button.title = ""
            button.imagePosition = button.image != nil ? .imageOnly : .noImage
        } else {
            item.length = NSStatusItem.variableLength
            button.title = button.image != nil ? " \(text)" : text
            button.imagePosition = button.image != nil ? .imageLeading : .noImage
        }
        if updateReady {
            button.toolTip = L10n.string(
                updateOpensStore ? "lx_update_available_title" : "lx_update_ready_to_install",
                appConfig.productName)
        } else {
            button.toolTip = baseToolTip
        }
        return true
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
        // Right- / control-click shows the JS-provided menu, or Quit on an
        // exclusive tray that never registered one.
        if isSecondaryClick, let menu = contextMenu(), let statusItem = statusItems[actionID] {
            onWillShowMenu?()
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
