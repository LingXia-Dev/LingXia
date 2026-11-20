#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// The core keeps one viewport per lxapp, so when more than one host can show an lxapp (the
/// Runner's phone frame and its desktop shell) only the host presenting it reports; a host
/// ordered out stays quiet. Until a host claims an lxapp, every host reports.
@MainActor
enum SurfaceViewportPresenter {
    /// Which host presents each lxapp, held weakly: a closed host releases its claim.
    struct Presenters {
        private final class Claim {
            weak var host: AnyObject?
            init(_ host: AnyObject) { self.host = host }
        }
        private var claims: [String: Claim] = [:]

        mutating func present(_ appId: String, by host: AnyObject) {
            claims[appId] = Claim(host)
        }

        func admits(_ appId: String, from host: AnyObject?) -> Bool {
            guard let presenter = claims[appId]?.host else { return true }
            return host === presenter
        }

        func presents(_ appId: String, _ host: AnyObject) -> Bool {
            claims[appId]?.host === host
        }
    }

    static var presenters = Presenters()
    static let retryLimit = 20
    static let retryDelay: TimeInterval = 0.05

    static func admits(appId: String, from host: AnyObject?) -> Bool {
        presenters.admits(appId, from: host)
    }

    /// `host` presents `appId` from now on. Once the switch to it settles, `measure` gives the
    /// viewport in the page's points, or nil while it cannot yet (it is retried a while).
    static func present(appId: String, by host: AnyObject, measure: @escaping @MainActor () -> CGSize?) {
        presenters.present(appId, by: host)
        // After the switch: the host left can lay out once more while it goes.
        DispatchQueue.main.async { report(appId: appId, by: host, measure: measure, attempt: 0) }
    }

    private static func report(appId: String, by host: AnyObject, measure: @escaping @MainActor () -> CGSize?, attempt: Int) {
        guard presenters.presents(appId, host) else { return }
        if let size = measure(), size.width > 0, size.height > 0 {
            _ = setSurfaceViewport(appId, Double(size.width), Double(size.height))
            return
        }
        guard attempt < retryLimit else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + retryDelay) { [weak host] in
            guard let host else { return }
            report(appId: appId, by: host, measure: measure, attempt: attempt + 1)
        }
    }
}
#endif
