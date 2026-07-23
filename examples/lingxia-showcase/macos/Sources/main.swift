import AppKit
import OSLog
import lingxia

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {
    private static let log = OSLog(subsystem: "LingXia", category: "ExampleApp")

    func applicationDidFinishLaunching(_ notification: Notification) {
        Lingxia.enableWebViewDebugging()
        do {
            _ = try Lingxia.quickStart()
            NotificationCenter.default.addObserver(
                self,
                selector: #selector(displayLanguageDidChange(_:)),
                name: Lingxia.displayLanguageDidChangeNotification,
                object: nil
            )
            setupStandardMenu()
        } catch {
            os_log(
                "Lingxia.quickStart() app-ui path failed: %{public}@",
                log: Self.log,
                type: .error,
                String(describing: error)
            )
            fatalError("Lingxia startup failed: \(error)")
        }
    }

    @MainActor @objc private func displayLanguageDidChange(_ notification: Notification) {
        setupStandardMenu()
    }

    /// Rebuilt at launch and whenever the browser settings change the product
    /// display language.
    @MainActor
    private func setupStandardMenu() {
        let mainMenu = NSMenu()

        // App Menu
        let appMenuItem = NSMenuItem()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: L10n.AppMenu.about, action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        appMenu.addItem(NSMenuItem.separator())
        appMenu.addItem(withTitle: L10n.AppMenu.quit, action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appMenuItem.submenu = appMenu
        mainMenu.addItem(appMenuItem)

        // Edit Menu
        let editMenuItem = NSMenuItem()
        let editMenu = NSMenu(title: L10n.AppMenu.edit)
        editMenu.addItem(withTitle: L10n.AppMenu.cut, action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: L10n.AppMenu.copy, action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: L10n.AppMenu.paste, action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: L10n.AppMenu.selectAll, action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        editMenuItem.submenu = editMenu
        mainMenu.addItem(editMenuItem)

        // Window Menu
        let windowMenuItem = NSMenuItem()
        let windowMenu = NSMenu(title: L10n.AppMenu.window)
        windowMenu.addItem(withTitle: L10n.AppMenu.minimize, action: #selector(NSWindow.miniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: L10n.AppMenu.zoom, action: #selector(NSWindow.zoom(_:)), keyEquivalent: "")
        windowMenu.addItem(NSMenuItem.separator())
        windowMenu.addItem(withTitle: L10n.AppMenu.bringAllToFront, action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")
        windowMenuItem.submenu = windowMenu
        mainMenu.addItem(windowMenuItem)
        NSApp.windowsMenu = windowMenu

        NSApp.mainMenu = mainMenu
        NSApp.servicesMenu = nil
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        return !Lingxia.handleAppActivation()
    }
}

let app = NSApplication.shared
let delegate = LingXiaAppDelegate()
app.delegate = delegate
app.run()
