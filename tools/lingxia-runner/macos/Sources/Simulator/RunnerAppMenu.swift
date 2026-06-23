import AppKit

/// The runner's standard macOS app menu (Restart Runner + Quit).
///
/// Device selection lives on the always-visible simulator toolbar and lxapp
/// lifecycle actions live on the phone capsule's "···" menu, so there is no
/// menu-bar "Device" submenu — this only installs the app menu every macOS app
/// is expected to have.
@MainActor
final class RunnerAppMenu: NSObject {
    private static let appRestartTag = 0x4C58_5253
    private static let appQuitTag = 0x4C58_5154

    func installIfNeeded() {
        let mainMenu = NSApp.mainMenu ?? makeBaseMainMenu()
        installAppCommands(in: mainMenu)
        NSApp.mainMenu = mainMenu
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

    @objc private func restartRunner(_ sender: NSMenuItem) {
        RunnerApp.shared.restartRunner()
    }
}
