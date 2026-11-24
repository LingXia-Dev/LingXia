import XCTest
@testable import lingxia

@MainActor
final class LayoutPassQueueTests: XCTestCase {
    /// Work the queue hands to the next run-loop turn, run by the test when it chooses.
    private var deferred: [@MainActor () -> Void] = []

    private func queue(bound: Int = 8, onBound: @escaping (Int, Int) -> Void = { _, _ in })
        -> LayoutPassQueue<String, Int>
    {
        LayoutPassQueue<String, Int>(
            bound: bound, same: ==, onBound: onBound, later: { [unowned self] in self.deferred.append($0) })
    }

    private func runDeferred() {
        while !deferred.isEmpty { deferred.removeFirst()() }
    }

    func testPlanCommittedInsideAPassAppliesAfterIt() {
        let passes = queue()
        var events: [String] = []
        func apply(_ plan: Int) {
            events.append("begin \(plan)")
            // Plan 6's pass reports a width; the core answers with plan 7 at once.
            if plan == 6 { passes.submit(7, for: "w", apply: apply) }
            events.append("end \(plan)")
        }
        passes.submit(6, for: "w", apply: apply)
        XCTAssertEqual(events, ["begin 6", "end 6", "begin 7", "end 7"])
    }

    func testPendingPlansOfOneWindowCoalesceToTheNewest() {
        let passes = queue()
        var applied: [String] = []
        func apply(_ plan: Int) {
            applied.append("\(plan)")
            if plan == 1 {
                passes.submit(2, for: "w", apply: apply)
                passes.submit(5, for: "x", apply: apply)
                passes.submit(3, for: "w", apply: apply)
            }
        }
        passes.submit(1, for: "w", apply: apply)
        XCTAssertEqual(applied, ["1", "3", "5"])
    }

    func testALoopOfIdenticalPlansEnds() {
        let passes = queue()
        var count = 0
        func apply(_ plan: Int) {
            count += 1
            // Every pass makes the core commit the same layout again.
            passes.submit(plan, for: "w", apply: apply)
        }
        passes.submit(4, for: "w", apply: apply)
        runDeferred()
        XCTAssertEqual(count, 1)
        XCTAssertTrue(deferred.isEmpty)
    }

    func testTheBoundYieldsTheThreadAndStillAppliesTheNewestPlan() {
        var reported: [(Int, Int)] = []
        let passes = queue(bound: 3, onBound: { reported.append(($0, $1)) })
        var applied: [Int] = []
        func apply(_ plan: Int) {
            applied.append(plan)
            // Each pass commits a new plan until 10.
            if plan < 10 { passes.submit(plan + 1, for: "w", apply: apply) }
        }
        passes.submit(1, for: "w", apply: apply)
        XCTAssertEqual(applied, [1, 2, 3])
        XCTAssertEqual(deferred.count, 1)
        runDeferred()
        XCTAssertEqual(applied, Array(1...10))
        XCTAssertEqual(applied.last, 10)
        XCTAssertEqual(reported.count, 1)
        XCTAssertEqual(reported.first?.0, 3)
        XCTAssertEqual(reported.first?.1, 4)
    }

    func testAPlanSubmittedWhileAResumeIsPendingIsNotLost() {
        let passes = queue(bound: 1)
        var applied: [Int] = []
        func apply(_ plan: Int) {
            applied.append(plan)
            if plan == 1 { passes.submit(2, for: "w", apply: apply) }
        }
        passes.submit(1, for: "w", apply: apply)
        passes.submit(9, for: "w", apply: apply)
        XCTAssertEqual(applied, [1])
        runDeferred()
        XCTAssertEqual(applied, [1, 9])
    }
}
