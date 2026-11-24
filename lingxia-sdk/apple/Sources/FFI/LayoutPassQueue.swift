import Foundation

/// Runs layout passes one at a time, in commit order.
///
/// A pass can call into the core (reporting metrics, closing a child), and the
/// core commits and pushes the next plan synchronously on the same thread. Run
/// inline, that newer plan would apply inside the older pass, which then finishes
/// with its stale plan. Queued here, it applies after the current pass returns.
///
/// Plans for one window coalesce: only the newest pending one applies, and one equal
/// to the last applied is skipped, so a pass that keeps committing the same plan ends.
/// A drain applies at most `bound` plans; the rest wait for the next turn of the run
/// loop, so a pass that commits a different plan every time cannot hold the main
/// thread, and the newest plan still applies.
@MainActor
final class LayoutPassQueue<Key: Hashable, Plan> {
    private let bound: Int
    private let same: (Plan, Plan) -> Bool
    private let onBound: (_ previous: Plan, _ next: Plan) -> Void
    private let later: (@escaping @MainActor () -> Void) -> Void

    private var running = false
    private var order: [Key] = []
    private var pending: [Key: Plan] = [:]
    private var applied: [Key: Plan] = [:]
    private var apply: ((Plan) -> Void)?
    private var resumeScheduled = false
    private var boundReported = false

    init(
        bound: Int = 8,
        same: @escaping (Plan, Plan) -> Bool,
        onBound: @escaping (_ previous: Plan, _ next: Plan) -> Void = { _, _ in },
        later: @escaping (@escaping @MainActor () -> Void) -> Void = { work in
            DispatchQueue.main.async { MainActor.assumeIsolated(work) }
        }
    ) {
        self.bound = max(1, bound)
        self.same = same
        self.onBound = onBound
        self.later = later
    }

    func submit(_ plan: Plan, for key: Key, apply: @escaping (Plan) -> Void) {
        if pending.updateValue(plan, forKey: key) == nil {
            order.append(key)
        }
        self.apply = apply
        if !running && !resumeScheduled {
            drain()
        }
    }

    private func drain() {
        guard let apply else { return }
        running = true
        defer { running = false }
        var count = 0
        var last: (key: Key, plan: Plan)?
        while let key = order.first {
            guard let plan = pending[key] else {
                order.removeFirst()
                continue
            }
            if let current = applied[key], same(current, plan) {
                order.removeFirst()
                pending[key] = nil
                continue
            }
            if count == bound {
                if !boundReported, let last, last.key == key {
                    boundReported = true
                    onBound(last.plan, plan)
                }
                resumeScheduled = true
                later { [weak self] in
                    guard let self else { return }
                    self.resumeScheduled = false
                    if !self.running { self.drain() }
                }
                return
            }
            order.removeFirst()
            pending[key] = nil
            applied[key] = plan
            count += 1
            last = (key, plan)
            apply(plan)
        }
    }
}
