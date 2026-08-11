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
            // A second copy cannot share the first's databases. Say so and
            // leave, rather than trapping with a hardware exception.
            FileHandle.standardError.write(
                "\(ProcessInfo.processInfo.processName): cannot start — \(error)\n"
                    .data(using: .utf8) ?? Data()
            )
            exit(1)
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
