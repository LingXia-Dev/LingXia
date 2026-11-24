import XCTest
@testable import lingxia

final class TonguePlacementTests: XCTestCase {
    private func place(_ blocked: [ClosedRange<CGFloat>], start: CGFloat = 50, end: CGFloat = 80)
        -> TonguePlacement
    {
        TonguePlacement.place(length: 400, blocked: blocked, start: start, end: end, size: 88, margin: 8)
    }

    func testClearEdgeCentresFlush() {
        XCTAssertEqual(place([]), .flush(offset: 156))
    }

    func testBlockedMiddleSlidesUpWhenOnlyAboveHasRoom() {
        // Above, 50...172 holds the tongue; below, 228...290 is too short.
        XCTAssertEqual(place([180...220], end: 110), .flush(offset: 84))
    }

    func testBlockedMiddleSlidesDownWhenOnlyBelowHasRoom() {
        XCTAssertEqual(place([180...220], start: 120), .flush(offset: 228))
    }

    func testNoClearStretchFallsBackToInset() {
        XCTAssertEqual(place([140...260]), .inset)
    }

    func testWholeEdgeBlockedFallsBackToInset() {
        XCTAssertEqual(place([0...400]), .inset)
    }

    func testCutoutOnTheOtherEdgeLeavesThisOneClear() {
        // The caller passes only this edge's blocks; a cutout on the far edge adds none.
        XCTAssertEqual(place([]), .flush(offset: 156))
    }

    func testPrefersTheSideNearerTheMiddle() {
        // Both sides have room; the block sits below the middle, so above is nearer.
        XCTAssertEqual(place([200...230], end: 30), .flush(offset: 104))
    }
}
