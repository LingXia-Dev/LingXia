#if os(macOS)
import XCTest
@testable import lingxia

final class TrustedBrowserNavigationTests: XCTestCase {
    func testFixedBrowserRoutesUseNativeControlAuthority() {
        for navigation in [
            BrowserLocalNavigation.settings,
            .clearSiteData(tabID: "tab%201"),
            .downloads,
            .bookmarks,
            .history,
        ] {
            XCTAssertEqual(navigation.openAuthority, .nativeControl)
        }
    }
}
#endif
