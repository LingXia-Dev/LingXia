import AppKit
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
        return true
    }
}

// Entry point
let app = NSApplication.shared
let delegate = LingXiaAppDelegate()
app.delegate = delegate
app.run()
