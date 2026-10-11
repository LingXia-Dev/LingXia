#if os(macOS)
import AppKit

/// The menu bar every host gets: About and Quit, plus the Edit and Window
/// menus. AppKit routes ⌘Q, ⌘C/⌘V/⌘X/⌘A, and ⌘M only through menu items, so
/// without them those shortcuts never reach the app or a focused WebView.
/// A host that built its own `NSApp.mainMenu` keeps it.
@MainActor
enum LxAppMacMainMenu {
    private static var installed: NSMenu?
    private static var languageObserver: NSObjectProtocol?

    static func installIfNeeded() {
        // Without a nib AppKit still puts up a placeholder menu bar whose app
        // menu is empty; only a menu with items is the host's own.
        let hostBuilt = NSApp.mainMenu?.items.contains { ($0.submenu?.numberOfItems ?? 0) > 0 } ?? false
        guard !hostBuilt else { return }
        install()
        NSLog("LingXia.MainMenu: standard menu bar installed")
        languageObserver = NotificationCenter.default.addObserver(
            forName: Lingxia.displayLanguageDidChangeNotification,
            object: nil,
            queue: .main
        ) { _ in
            MainActor.assumeIsolated {
                // Rebuild only the menu installed here; a host may have
                // replaced it since.
                guard let installed, NSApp.mainMenu === installed else { return }
                install()
            }
        }
    }

    private static func install() {
        let mainMenu = NSMenu()

        let appMenu = NSMenu()
        let about = NSMenuItem(
            title: L10n.AppMenu.about,
            action: #selector(AboutTarget.showAbout(_:)),
            keyEquivalent: ""
        )
        about.target = AboutTarget.shared
        appMenu.addItem(about)
        appMenu.addItem(.separator())
        appMenu.addItem(
            withTitle: L10n.AppMenu.quit,
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        )
        mainMenu.addItem(submenuItem(appMenu))

        // Nil targets: the first responder (a WebView or text field) handles them.
        let editMenu = NSMenu(title: L10n.AppMenu.edit)
        editMenu.addItem(withTitle: L10n.AppMenu.cut, action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: L10n.AppMenu.copy, action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: L10n.AppMenu.paste, action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(
            withTitle: L10n.AppMenu.selectAll,
            action: #selector(NSText.selectAll(_:)),
            keyEquivalent: "a"
        )
        mainMenu.addItem(submenuItem(editMenu))

        let windowMenu = NSMenu(title: L10n.AppMenu.window)
        windowMenu.addItem(
            withTitle: L10n.AppMenu.minimize,
            action: #selector(NSWindow.miniaturize(_:)),
            keyEquivalent: "m"
        )
        windowMenu.addItem(withTitle: L10n.AppMenu.zoom, action: #selector(NSWindow.zoom(_:)), keyEquivalent: "")
        windowMenu.addItem(.separator())
        windowMenu.addItem(
            withTitle: L10n.AppMenu.bringAllToFront,
            action: #selector(NSApplication.arrangeInFront(_:)),
            keyEquivalent: ""
        )
        mainMenu.addItem(submenuItem(windowMenu))

        NSApp.mainMenu = mainMenu
        NSApp.windowsMenu = windowMenu
        NSApp.servicesMenu = nil
        // The bold menu-bar title is the app menu's title. AppKit resets it to
        // the system-language bundle name while taking the new menu, so set it
        // a turn later.
        DispatchQueue.main.async { appMenu.title = L10n.AppMenu.hostAppName }
        installed = mainMenu
    }

    /// The standard panel names the app by its system-language bundle name;
    /// pass the display-language one so it matches the menu. An empty
    /// `.version` drops the "(2005)" build number, which is a store
    /// ordering key, not something to show people.
    @MainActor
    private final class AboutTarget: NSObject {
        static let shared = AboutTarget()

        @objc func showAbout(_ sender: Any?) {
            NSApp.orderFrontStandardAboutPanel(options: [
                .applicationName: L10n.AppMenu.hostAppName,
                .version: "",
            ])
            NSApp.activate(ignoringOtherApps: true)
        }
    }

    private static func submenuItem(_ menu: NSMenu) -> NSMenuItem {
        let item = NSMenuItem()
        item.submenu = menu
        return item
    }
}
#endif
