import Foundation

/// Runs layout passes one at a time, in commit order.
///
/// A pass can call into the core (reporting metrics, closing a child), and the
/// core commits and pushes the next plan synchronously on the same thread. Run
/// inline, that newer plan would apply inside the older pass, which then finishes
/// with its stale plan. Queued here, it applies after the current pass returns.
/// This cannot loop where recursion would not: a pass that commits nothing new
/// leaves the queue empty.
@MainActor
final class LayoutPassQueue<Plan> {
    private var running = false
    private var pending: [Plan] = []

    func submit(_ plan: Plan, apply: (Plan) -> Void) {
        pending.append(plan)
        guard !running else { return }
        running = true
        defer { running = false }
        // Invariant: the last plan applied is always the newest.
        while !pending.isEmpty {
            apply(pending.removeFirst())
        }
    }
}
