import AppKit

@MainActor
final class RunnerDeviceMenu: NSObject {
    private static let appRestartTag = 0x4C58_5253
    private static let appQuitTag = 0x4C58_5154
    private static let menuTag = 0x4C58_4456

    private weak var deviceMenu: NSMenu?

    func installIfNeeded() {
        let mainMenu = NSApp.mainMenu ?? makeBaseMainMenu()
        installAppCommands(in: mainMenu)
        if mainMenu.item(withTag: Self.menuTag) == nil {
            let item = NSMenuItem(title: "Device", action: nil, keyEquivalent: "")
            item.tag = Self.menuTag
            let menu = NSMenu(title: "Device")
            item.submenu = menu
            mainMenu.addItem(item)
            deviceMenu = menu
            populate(menu)
        } else if let menu = mainMenu.item(withTag: Self.menuTag)?.submenu {
            deviceMenu = menu
            if menu.items.isEmpty {
                populate(menu)
            }
        }
        NSApp.mainMenu = mainMenu
    }

    func updateSelectedDevice(_ device: MobileDeviceSize) {
        installIfNeeded()
        deviceMenu?.items.forEach { item in
            guard let id = item.representedObject as? String else { return }
            item.state = id == device.id ? .on : .off
        }
    }

    private func populate(_ menu: NSMenu) {
        let clean = NSMenuItem(
            title: "Clean Cache and Restart LxApp",
            action: #selector(cleanCacheAndRestartLxApp(_:)),
            keyEquivalent: ""
        )
        clean.target = self
        clean.image = NSImage(systemSymbolName: "trash", accessibilityDescription: nil)
        menu.addItem(clean)

        let restart = NSMenuItem(
            title: "Restart LxApp",
            action: #selector(restartLxApp(_:)),
            keyEquivalent: ""
        )
        restart.target = self
        restart.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: nil)
        menu.addItem(restart)
        menu.addItem(.separator())

        var previousShape: RunnerDeviceShape?
        for device in MobileDeviceSize.allCases {
            if let previousShape, previousShape != device.shape {
                menu.addItem(.separator())
            }
            let item = NSMenuItem(
                title: device.displayName,
                action: #selector(deviceSelected(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = device.id
            menu.addItem(item)
            previousShape = device.shape
        }
    }

    private func makeBaseMainMenu() -> NSMenu {
        let mainMenu = NSMenu()
        let appItem = NSMenuItem()
        let appMenu = NSMenu(title: "LingXia Runner")
        appItem.submenu = appMenu
        mainMenu.addItem(appItem)
        return mainMenu
    }

    private func installAppCommands(in mainMenu: NSMenu) {
        let appItem: NSMenuItem
        if let first = mainMenu.items.first {
            appItem = first
        } else {
            appItem = NSMenuItem()
            mainMenu.addItem(appItem)
        }

        let appMenu: NSMenu
        if let existingMenu = appItem.submenu {
            appMenu = existingMenu
        } else {
            appMenu = NSMenu(title: "LingXia Runner")
            appItem.submenu = appMenu
        }

        if appMenu.item(withTag: Self.appRestartTag) == nil {
            let restart = NSMenuItem(
                title: "Restart LingXia Runner",
                action: #selector(restartRunner(_:)),
                keyEquivalent: "r"
            )
            restart.keyEquivalentModifierMask = [.command, .shift]
            restart.target = self
            restart.tag = Self.appRestartTag
            appMenu.insertItem(restart, at: 0)
            if appMenu.items.count > 1, !appMenu.items[1].isSeparatorItem {
                appMenu.insertItem(.separator(), at: 1)
            }
        }

        let hasQuit = appMenu.items.contains { item in
            item.tag == Self.appQuitTag
                || item.action == #selector(NSApplication.terminate(_:))
                || item.title.hasPrefix("Quit ")
        }
        if !hasQuit {
            if !appMenu.items.isEmpty {
                appMenu.addItem(.separator())
            }
            let quit = NSMenuItem(
                title: "Quit LingXia Runner",
                action: #selector(NSApplication.terminate(_:)),
                keyEquivalent: "q"
            )
            quit.target = NSApp
            quit.tag = Self.appQuitTag
            appMenu.addItem(quit)
        }
    }

    @objc private func deviceSelected(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String,
              let device = MobileDeviceSize.allCases.first(where: { $0.id == id }) else {
            return
        }
        RunnerApp.shared.setDeviceSize(device)
    }

    @objc private func cleanCacheAndRestartLxApp(_ sender: NSMenuItem) {
        RunnerApp.shared.cleanCacheAndRestartCurrentLxApp()
    }

    @objc private func restartLxApp(_ sender: NSMenuItem) {
        RunnerApp.shared.restartCurrentLxApp()
    }

    @objc private func restartRunner(_ sender: NSMenuItem) {
        RunnerApp.shared.restartRunner()
    }
}
