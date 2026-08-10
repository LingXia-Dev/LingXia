import AppKit
// Add the LingXia Swift package dependency in Package.swift before building.
import lingxia

// Before AppKit: this executable is also the product's command line, and a
// command must not open a window or touch a running instance's databases.
Lingxia.runProductCommandIfInvoked()

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
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
