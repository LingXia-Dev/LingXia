import AppKit
@_spi(Runner) import lingxia

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
    private let controller = LxAppController()

    func applicationDidFinishLaunching(_ notification: Notification) {
        Lingxia.enableWebViewDebugging()
        RunnerApp.shared.bind(controller: controller)
        Lingxia.activate(controller: controller)
        _ = try? Lingxia.initializeRuntime()

        controller.setInterceptor(.willOpen) { context in
            guard case .object(let payload) = context.payload,
                  case .string(let appId)? = payload["appId"],
                  case .string(let path)? = payload["path"] else {
                return .reject(reason: "runner requires appId/path in willOpen payload")
            }

            RunnerApp.shared.openLxApp(appId: appId, path: path)
            return .handled
        }

        RunnerApp.shared.setDeviceSize(.iPhoneSE)
        Task { @MainActor in
            _ = try? await controller.openHomeApp()
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}
