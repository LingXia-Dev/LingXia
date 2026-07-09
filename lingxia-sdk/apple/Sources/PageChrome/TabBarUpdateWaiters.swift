import Foundation

/// Pending completion callbacks for awaited tabbar updates
/// (lx.showTabBar/hideTabBar), keyed by appid.
///
/// The `.tabBarStateChanged` observers all register with `queue: .main` and
/// apply the change inside a `Task { @MainActor }`, so notification delivery
/// is asynchronous — the poster cannot know when the bar actually changed.
/// Observers call `complete` at the end of their apply; a bounded fallback
/// covers hosts without a signaling observer so the JS promise never waits
/// on silence.
@MainActor
public enum TabBarUpdateWaiters {
    private static var pending: [String: [UInt64]] = [:]

    static func add(_ appId: String, _ callbackId: UInt64) {
        pending[appId, default: []].append(callbackId)
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            complete(appId)
        }
    }

    /// Idempotent: the first signal (observer or fallback) wins.
    public static func complete(_ appId: String) {
        guard let ids = pending.removeValue(forKey: appId) else { return }
        for id in ids {
            _ = onCallback(id, true, "{}")
        }
    }
}
