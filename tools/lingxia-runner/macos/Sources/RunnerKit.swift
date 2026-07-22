import AppKit
import Darwin
@_spi(Runner) import lingxia

private func removeRunnerPidFileIfRequested() {
    let env = ProcessInfo.processInfo.environment
    guard let path = env["LINGXIA_RUNNER_PID_FILE"], !path.isEmpty else { return }
    try? FileManager.default.removeItem(atPath: path)
}

/// Public entry point for the LingXia Runner simulator.
public struct RunnerKit {
    @MainActor
    public static func run() {
        writePidFileIfRequested()
        let delegate = RunnerKitDelegate()
        let app = NSApplication.shared
        app.delegate = delegate
        app.setActivationPolicy(.regular)
        app.run()
    }

    /// Record our real pid where `lingxia dev` can find it (per project), so it
    /// terminates exactly this Runner and leaves other projects' Runners alone.
    /// Writing our own pid is also correct across a LaunchServices hand-off.
    @MainActor
    private static func writePidFileIfRequested() {
        let env = ProcessInfo.processInfo.environment
        guard let path = env["LINGXIA_RUNNER_PID_FILE"], !path.isEmpty else { return }
        let url = URL(fileURLWithPath: path)
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try? "\(getpid())".write(to: url, atomically: true, encoding: .utf8)
        NotificationCenter.default.addObserver(
            forName: NSApplication.willTerminateNotification, object: nil, queue: .main
        ) { _ in
            removeRunnerPidFileIfRequested()
        }
    }
}

@MainActor
private class RunnerKitDelegate: NSObject, NSApplicationDelegate {
    private let controller = LxAppController()

    func applicationDidFinishLaunching(_ notification: Notification) {
        Lingxia.enableWebViewDebugging()
        if let rawURL = ProcessInfo.processInfo.environment["LINGXIA_RUNNER_WEB_URL"],
           let url = URL(string: rawURL),
           url.scheme == "http" || url.scheme == "https" {
            guard initializeRuntime() else { return }
            RunnerApp.shared.setDeviceSize(.defaultDevice)
            RunnerApp.shared.openWeb(url: url)
            return
        }
        RunnerApp.shared.bind(controller: controller)
        Lingxia.activate(controller: controller)
        guard initializeRuntime() else { return }

        controller.setInterceptor(.willOpen) { context in
            guard case .object(let payload) = context.payload,
                  case .string(let appId)? = payload["appId"],
                  case .string(let path)? = payload["path"] else {
                return .reject(reason: "runner requires appId/path in willOpen payload")
            }

            RunnerApp.shared.openLxApp(appId: appId, path: path)
            return .handled
        }

        RunnerApp.shared.setDeviceSize(.defaultDevice)
        Task { @MainActor in
            _ = try? await controller.openHomeApp()
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return ProcessInfo.processInfo.environment["LINGXIA_RUNNER_WEB_URL"] != nil
    }

    private func initializeRuntime() -> Bool {
        do {
            _ = try Lingxia.initializeRuntime()
            return true
        } catch {
            NSLog("LingXia Runner runtime initialization failed: %@", error.localizedDescription)
            removeRunnerPidFileIfRequested()
            Darwin.exit(EXIT_FAILURE)
        }
    }
}
