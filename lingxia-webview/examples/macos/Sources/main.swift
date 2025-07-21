import Cocoa
import lingxia
import os.log

let appLog = OSLog(subsystem: "LingXia", category: "App")

@MainActor
class AppDelegate: NSObject, NSApplicationDelegate {
    private let log = appLog

    func applicationDidFinishLaunching(_ aNotification: Notification) {

        print("AppDelegate: Initializing LxApps...")
        // Initialize LxApps - if fails, terminate app
        guard macOSLxApp.initialize() else {
            os_log("Failed to initialize LxApps", log: log, type: .error)
            NSApp.terminate(nil)
            return
        }

        // iPhone 11 physical size: 6.1cm x 13.2cm (2.4" x 5.2")
        LxApp.setWindowSize(widthCm: 6.1, heightCm: 13.2)

        LxApp.setWindowStyle(.customCapsule)
        //LxApp.setWindowStyle(.systemDefault)

        LxApp.openHomeLxApp()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}

let app = NSApplication.shared
let appDelegate = AppDelegate()
app.delegate = appDelegate

app.run()
