import XCTest
@testable import lingxia

@MainActor
final class LayoutPassQueueTests: XCTestCase {
    func testPlanCommittedInsideAPassAppliesAfterIt() {
        let queue = LayoutPassQueue<Int>()
        var events: [String] = []
        func apply(_ plan: Int) {
            events.append("begin \(plan)")
            // Plan 6's pass reports a width; the core answers with plan 7 at once.
            if plan == 6 { queue.submit(7, apply: apply) }
            events.append("end \(plan)")
        }
        queue.submit(6, apply: apply)
        XCTAssertEqual(events, ["begin 6", "end 6", "begin 7", "end 7"])
    }

    func testPassesChainInCommitOrderAndStop() {
        let queue = LayoutPassQueue<Int>()
        var applied: [Int] = []
        func apply(_ plan: Int) {
            applied.append(plan)
            if plan < 3 {
                queue.submit(plan + 1, apply: apply)
                queue.submit(plan + 10, apply: apply)
            }
        }
        queue.submit(1, apply: apply)
        XCTAssertEqual(applied, [1, 2, 11, 3, 12])
        queue.submit(20, apply: apply)
        XCTAssertEqual(applied.last, 20)
    }
}
