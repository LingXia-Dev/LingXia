#if os(iOS)
import Foundation

/// The lx feature that needs a runtime permission.
public enum PermissionFeature: String, Sendable {
    case camera
    case scanCode
    case mediaLibrary
    case saveMedia
    case location
    case wifi
    case notification
}

public enum PermissionOutcome: Sendable {
    case granted
    case denied
    /// Denied and the system will not prompt again; only Settings can grant it.
    case deniedPermanently
}

/// A system permission prompt LingXia is about to show.
@MainActor
public final class PermissionPrompt {
    public let feature: PermissionFeature
    /// The system permission it asks for: `camera`, `microphone`, `photos`,
    /// `location`, or `notifications`. Video capture prompts for the camera
    /// and the microphone separately.
    public let permission: String
    /// The lxapp that triggered it, when one is foreground.
    public let appId: String?
    /// The user denied it before, so [proceed] shows no system prompt; guide
    /// the user to Settings instead.
    public let blocked: Bool
    private let decide: (Bool) -> Void

    init(feature: PermissionFeature, permission: String, appId: String?, blocked: Bool,
         decide: @escaping (Bool) -> Void) {
        self.feature = feature
        self.permission = permission
        self.appId = appId
        self.blocked = blocked
        self.decide = decide
    }

    /// Show the system prompt.
    public func proceed() { decide(true) }

    /// Skip the system prompt; the lx API fails as permission denied.
    public func deny() { decide(false) }
}

/// Host hook, called each time an lx API needs a permission it lacks (never
/// once granted), including after a denial (`prompt.blocked`). Prompts are
/// gated one at a time, on the main actor.
///
/// App Review rejects an explanation screen that offers a way out before the
/// system prompt; on iOS, explain with a single "Continue" that calls `proceed()`.
@MainActor
public protocol PermissionGate: AnyObject {
    /// Call `prompt.proceed()` or `prompt.deny()`, now or later; only the first call counts.
    func beforePrompt(_ prompt: PermissionPrompt)

    /// The system prompt closed. Not called after `prompt.deny()`.
    func afterPrompt(_ prompt: PermissionPrompt, outcome: PermissionOutcome)
}

public extension PermissionGate {
    func afterPrompt(_ prompt: PermissionPrompt, outcome: PermissionOutcome) {}
}

extension Lingxia {
    /// Runs `gate` each time an lx API needs a permission it lacks; `nil`
    /// prompts directly. Last write wins.
    public static func setPermissionGate(_ gate: PermissionGate?) {
        PermissionGates.gate = gate
    }
}

@MainActor
enum PermissionGates {
    static var gate: PermissionGate?
    private static var queue: [() -> Void] = []
    private static var busy = false

    /// Runs `prompt` unless the gate denies, in which case `declined` runs.
    /// `prompt` reports its outcome through `finish`. Without a gate, `prompt`
    /// runs directly.
    static func run(
        _ feature: PermissionFeature,
        permission: String,
        blocked: Bool,
        declined: @escaping () -> Void,
        prompt: @escaping (_ finish: @escaping (PermissionOutcome) -> Void) -> Void
    ) {
        guard let gate else {
            prompt { _ in }
            return
        }
        queue.append {
            gateOne(gate, feature, permission: permission, blocked: blocked, declined: declined, prompt: prompt)
        }
        if !busy { next() }
    }

    private static func next() {
        guard !queue.isEmpty else {
            busy = false
            return
        }
        busy = true
        queue.removeFirst()()
    }

    private static func gateOne(
        _ gate: PermissionGate,
        _ feature: PermissionFeature,
        permission: String,
        blocked: Bool,
        declined: @escaping () -> Void,
        prompt: @escaping (_ finish: @escaping (PermissionOutcome) -> Void) -> Void
    ) {
        var decided = false
        var request: PermissionPrompt?
        request = PermissionPrompt(
            feature: feature,
            permission: permission,
            appId: currentAppId(),
            blocked: blocked
        ) { allowed in
            Task { @MainActor in
                guard !decided, let request else { return }
                decided = true
                guard allowed else {
                    declined()
                    next()
                    return
                }
                var finished = false
                prompt { outcome in
                    Task { @MainActor in
                        guard !finished else { return }
                        finished = true
                        gate.afterPrompt(request, outcome: outcome)
                        next()
                    }
                }
            }
        }
        gate.beforePrompt(request!)
    }

    private static func currentAppId() -> String? {
        let appId = getCurrentLxApp().appid.toString()
        return appId.isEmpty ? nil : appId
    }

    /// Runs a gated prompt from a background thread and waits for its result;
    /// `nil` when `timeout` passes first.
    nonisolated static func blocking(
        timeout: DispatchTime = .distantFuture,
        _ body: @escaping @MainActor (_ done: @escaping (Bool) -> Void) -> Void
    ) -> Bool? {
        guard !Thread.isMainThread else {
            assertionFailure("a gated prompt cannot block the main thread")
            return false
        }
        let result = GateBox(false)
        let semaphore = DispatchSemaphore(value: 0)
        Task { @MainActor in
            body { granted in
                result.value = granted
                semaphore.signal()
            }
        }
        return semaphore.wait(timeout: timeout) == .timedOut ? nil : result.value
    }
}

private final class GateBox<T>: @unchecked Sendable {
    var value: T
    init(_ value: T) { self.value = value }
}
#endif
