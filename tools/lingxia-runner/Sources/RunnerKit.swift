import AppKit
import lingxia

/// Public entry point for the LingXia Runner simulator.
public struct RunnerKit {
    @MainActor
    public static func run() {
        let delegate = RunnerKitDelegate()
        let app = NSApplication.shared
        app.delegate = delegate
        app.setActivationPolicy(.regular)
        app.run()
    }
}

@MainActor
private class RunnerKitDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        LxApp.enableWebViewDebugging()
        LxApp.skipAutoOpenWindow = true

        LxApp.openLxAppHandler = { appId, path in
            Task { @MainActor in
                RunnerApp.shared.openLxApp(appId: appId, path: path)
            }
            return true
        }

        LxApp.navigationHandler = { appId, path, animationType in
            Task { @MainActor in
                RunnerApp.shared.handleNavigation(appId: appId, path: path, animationType: animationType)
            }
            return true
        }

        Lingxia.initialize()
        RunnerApp.shared.setDeviceSize(.iPhoneSE)
        RunnerApp.shared.openHomeLxApp()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}
