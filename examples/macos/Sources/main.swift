import AppKit
import lingxia

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Enable WebView debugging BEFORE LxApp.initialize()
        // This ensures debugging is enabled before the first WebView is created
        LxApp.enableWebViewDebugging()

        LxApp.initialize()

        // Opiton for Desktop
        //macOSLxApp.setWindowStyle(.tabStyle)

        macOSLxApp.setWindowSize(.iPhoneSE)
        macOSLxApp.setWindowStyle(.capsuleStyle)

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
