import AppKit

@MainActor
final class RunnerDeviceMenu: NSObject {
    private static let menuTag = 0x4C58_4456

    private weak var deviceMenu: NSMenu?

    func installIfNeeded() {
        let mainMenu = NSApp.mainMenu ?? makeBaseMainMenu()
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
        let appMenu = NSMenu()
        appMenu.addItem(
            NSMenuItem(
                title: "Quit LingXia Runner",
                action: #selector(NSApplication.terminate(_:)),
                keyEquivalent: "q"
            )
        )
        appItem.submenu = appMenu
        mainMenu.addItem(appItem)
        return mainMenu
    }

    @objc private func deviceSelected(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String,
              let device = MobileDeviceSize.allCases.first(where: { $0.id == id }) else {
            return
        }
        RunnerApp.shared.setDeviceSize(device)
    }
}
