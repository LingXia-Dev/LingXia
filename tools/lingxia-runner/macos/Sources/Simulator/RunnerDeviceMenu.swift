import AppKit

@MainActor
final class RunnerDeviceMenu: NSObject {
    private static let appRestartTag = 0x4C58_5253
    private static let appQuitTag = 0x4C58_5154
    private static let menuTag = 0x4C58_4456

    private weak var deviceMenu: NSMenu?
    private var selectedOrientation: RunnerDeviceOrientation = .portrait

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

    func updateSelectedDevice(_ device: MobileDeviceSize, orientation: RunnerDeviceOrientation) {
        installIfNeeded()
        selectedOrientation = device.supportsOrientation ? orientation : .portrait
        deviceMenu?.items.forEach { item in
            guard let value = item.representedObject as? String else { return }
            if let id = Self.deviceId(from: value) {
                item.state = id == device.id ? .on : .off
            } else if let orientation = Self.orientation(from: value) {
                item.isEnabled = device.supportsOrientation
                item.state = device.supportsOrientation && orientation == selectedOrientation ? .on : .off
            }
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

        for orientation in RunnerDeviceOrientation.allCases {
            let item = NSMenuItem(
                title: orientation.displayName,
                action: #selector(orientationSelected(_:)),
                keyEquivalent: ""
            )
            item.target = self
            item.representedObject = Self.orientationValue(orientation)
            menu.addItem(item)
        }
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
            item.representedObject = Self.deviceValue(device.id)
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
        guard let value = sender.representedObject as? String,
              let id = Self.deviceId(from: value),
              let device = MobileDeviceSize.allCases.first(where: { $0.id == id }) else {
            return
        }
        RunnerApp.shared.setDeviceSize(device)
    }

    @objc private func orientationSelected(_ sender: NSMenuItem) {
        guard let value = sender.representedObject as? String,
              let orientation = Self.orientation(from: value) else {
            return
        }
        RunnerApp.shared.setDeviceOrientation(orientation)
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

    private static func deviceValue(_ id: String) -> String {
        "device:\(id)"
    }

    private static func orientationValue(_ orientation: RunnerDeviceOrientation) -> String {
        "orientation:\(orientation.rawValue)"
    }

    private static func deviceId(from value: String) -> String? {
        guard value.hasPrefix("device:") else { return nil }
        return String(value.dropFirst("device:".count))
    }

    private static func orientation(from value: String) -> RunnerDeviceOrientation? {
        guard value.hasPrefix("orientation:") else { return nil }
        return RunnerDeviceOrientation(rawValue: String(value.dropFirst("orientation:".count)))
    }
}
