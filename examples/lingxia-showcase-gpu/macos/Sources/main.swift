import AppKit
import OSLog
import lingxia

// Before AppKit: this executable is also the product's command line, and a
// command must not open a window or touch a running instance's databases.
Lingxia.runProductCommandIfInvoked()

class LingXiaAppDelegate: NSObject, NSApplicationDelegate {
    private static let log = OSLog(subsystem: "LingXia", category: "ExampleApp")

    func applicationDidFinishLaunching(_ notification: Notification) {
        Lingxia.enableWebViewDebugging()
        do {
            _ = try Lingxia.quickStart()
        } catch {
            os_log(
                "Lingxia.quickStart() app-ui path failed: %{public}@",
                log: Self.log,
                type: .error,
                String(describing: error)
            )
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

let app = NSApplication.shared
let delegate = LingXiaAppDelegate()
app.delegate = delegate
app.run()
