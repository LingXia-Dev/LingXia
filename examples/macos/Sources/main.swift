import AppKit
import lingxia

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Initialize LingXia system
        LxApp.initialize()

        // Enable WebView debugging
        LxApp.enableWebViewDebugging()

        // Option 1: Use predefined device size (convenient)
        macOSLxApp.setWindowSize(.iPhoneSE)
        macOSLxApp.setWindowStyle(.capsuleStyle)

        // Option 2
        //macOSLxApp.setWindowStyle(.tabStyle)

        // Open home app immediately
        LxApp.openHomeLxApp()
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
