#if os(macOS)
import XCTest
@testable import lingxia

@MainActor
final class SurfaceViewportPresenterTests: XCTestCase {
    private typealias Presenters = SurfaceViewportPresenter.Presenters

    func testEveryHostReportsUntilOneClaimsTheApp() {
        let presenters = Presenters()
        XCTAssertTrue(presenters.admits("app", from: NSObject()))
        XCTAssertTrue(presenters.admits("app", from: nil))
    }

    func testOnlyThePresentingHostReportsAndASwitchMovesIt() {
        var presenters = Presenters()
        let shell = NSObject(), phone = NSObject()
        presenters.present("app", by: shell)
        XCTAssertTrue(presenters.admits("app", from: shell))
        XCTAssertFalse(presenters.admits("app", from: phone), "the phone frame is not shown")
        presenters.present("app", by: phone)
        XCTAssertFalse(presenters.admits("app", from: shell), "the shell ordered out stays quiet")
        XCTAssertTrue(presenters.admits("app", from: phone))
        XCTAssertTrue(presenters.admits("other", from: shell), "a claim is per lxapp")
    }

    func testAClosedPresenterReleasesItsClaim() {
        var presenters = Presenters()
        let phone = NSObject()
        do {
            let shell = NSObject()
            presenters.present("app", by: shell)
            XCTAssertFalse(presenters.admits("app", from: phone))
        }
        XCTAssertTrue(presenters.admits("app", from: phone))
    }

    func testAFrameDrawnSmallerReportsTheDevicesPoints() {
        let phone = CGSize(width: 393, height: 659)
        for scale in [1.0, 0.75, 0.4] as [CGFloat] {
            let shown = CGSize(width: phone.width * scale, height: phone.height * scale)
            let reported = SurfaceViewportPresenter.devicePoints(shown, displayScale: scale)
            XCTAssertEqual(reported.width, phone.width, accuracy: 0.001)
            XCTAssertEqual(reported.height, phone.height, accuracy: 0.001)
        }
        XCTAssertEqual(SurfaceViewportPresenter.devicePoints(phone, displayScale: 0), phone)
    }
}
#endif
