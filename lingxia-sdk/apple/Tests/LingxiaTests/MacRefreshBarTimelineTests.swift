#if os(macOS)
import XCTest
@testable import lingxia

final class MacRefreshBarTimelineTests: XCTestCase {
    private typealias Timeline = MacRefreshBarTimeline

    func testIdleShowsNothing() {
        XCTAssertFalse(Timeline().isShown(at: 5))
        XCTAssertFalse(Timeline().isRunning)
    }

    func testFirstStartIsFreshAndRepeatsChangeNothing() {
        var timeline = Timeline()
        XCTAssertEqual(timeline.start(at: 10, reduceMotion: false), .fresh)
        XCTAssertTrue(timeline.isRunning)
        XCTAssertTrue(timeline.isShown(at: 70))
        XCTAssertEqual(timeline.start(at: 11, reduceMotion: true), .unchanged)
        XCTAssertFalse(timeline.reduceMotion)
        XCTAssertEqual(timeline.sweepPhase(at: 10.6), 0.6, accuracy: 1e-9)
    }

    func testDoneTurnsSolidThenFadesOut() {
        var timeline = Timeline()
        timeline.start(at: 10, reduceMotion: false)
        XCTAssertTrue(timeline.complete(at: 12))
        XCTAssertFalse(timeline.complete(at: 12.01), "only once")
        XCTAssertFalse(timeline.isRunning)
        XCTAssertEqual(timeline.finishEnd!, 12.4, accuracy: 1e-9)
        XCTAssertTrue(timeline.isShown(at: 12.39))
        XCTAssertFalse(timeline.isShown(at: 12.41))
    }

    func testStartDuringTheFinishResumesWithTheSameSweep() {
        var timeline = Timeline()
        timeline.start(at: 10, reduceMotion: false)
        timeline.complete(at: 11)
        let phase = timeline.sweepPhase(at: 11.275)
        XCTAssertEqual(timeline.start(at: 11.275, reduceMotion: false), .resumed)
        XCTAssertTrue(timeline.isRunning)
        XCTAssertNil(timeline.finishEnd, "running again, not fading")
        XCTAssertEqual(timeline.sweepPhase(at: 11.275), phase, accuracy: 1e-9, "the band carries on")
    }

    func testStartAfterTheFadeBeginsAFreshRun() {
        var timeline = Timeline()
        timeline.start(at: 10, reduceMotion: false)
        timeline.complete(at: 11)
        XCTAssertEqual(timeline.start(at: 12, reduceMotion: true), .fresh)
        XCTAssertTrue(timeline.reduceMotion)
        XCTAssertEqual(timeline.sweepPhase(at: 12.2), 0.2, accuracy: 1e-9, "a fresh sweep")

        var reset = timeline
        reset.reset()
        XCTAssertFalse(reset.isShown(at: 12.2))
        XCTAssertEqual(reset.start(at: 20, reduceMotion: false), .fresh, "a hidden bar starts afresh too")
    }
}
#endif
