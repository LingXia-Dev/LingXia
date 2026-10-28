import Foundation
import CLingXiaRustAPI

/// Dev sessions pull the home lxapp's bundle from the dev server before init. The
/// pull is a network hop, so it runs on a background thread and init adopts its
/// outcome; outside a dev session it returns at once.
enum LxAppDevBundles {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var started = false
    private static let finished = DispatchGroup()

    /// Start the pull, once per process.
    static func start() {
        lock.lock()
        defer { lock.unlock() }
        guard !started else { return }
        started = true
        finished.enter()
        let directories = LxAppDirectoryFactory.createDirectoryConfig()
        let locale = Locale.current.identifier
        DispatchQueue.global(qos: .userInitiated).async {
            lingxiaPrepareDevBundles(directories.dataPath, directories.cachesPath, locale)
            finished.leave()
        }
    }

    /// Run `body` on the main queue once the pull finished.
    static func whenFinished(_ body: @escaping @MainActor @Sendable () -> Void) {
        start()
        finished.notify(queue: .main) { MainActor.assumeIsolated(body) }
    }

    /// Main thread, for synchronous entry points: wait for the pull while the run loop
    /// keeps turning, so the window server sees a live app and queued events wait.
    @MainActor
    static func waitPumpingRunLoop() {
        start()
        while finished.wait(timeout: .now()) == .timedOut {
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02))
        }
    }
}
