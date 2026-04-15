import AppKit
// Add the LingXia Swift package dependency in Package.swift before building.
import lingxia

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Enable WebView debugging BEFORE Lingxia.quickStart()
        // This ensures debugging is enabled before the first WebView is created
        Lingxia.enableWebViewDebugging()
        do {
            _ = try Lingxia.quickStart()
        } catch {
            fatalError("Lingxia.quickStart failed: \(error)")
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        return !Lingxia.handleAppActivation()
    }
}

// Entry point
let app = NSApplication.shared
let delegate = LingXiaAppDelegate()
app.delegate = delegate
app.run()
