import AppKit
import lingxia

@_silgen_name("lingxia_register_extensions")
func lingxia_register_extensions()

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        LxApp.registerExtensions = {
            lingxia_register_extensions()
        }

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
