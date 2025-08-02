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

        LxApp.simulateDevice(.iPhoneSE)
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
