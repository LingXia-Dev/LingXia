import AppKit
import lingxia

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Enable WebView debugging BEFORE LxApp.initialize()
        // This ensures debugging is enabled before the first WebView is created
        LxApp.enableWebViewDebugging()

        LxApp.initialize()
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
