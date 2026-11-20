#if os(iOS)
import UIKit
import XCTest
@testable import lingxia

final class IOSTabBarStartupTests: XCTestCase {
    @MainActor
    func testNotificationsWaitForContainersAndResumeAfterViewLoad() {
        let appId = "ios-tabbar-startup-test"
        let previousAppId = LxAppCore.currentAppId
        let previousPath = LxAppCore.getCurrentPath()
        defer {
            LxAppCore.setCurrentApp(
                appId: previousAppId ?? "", path: previousPath, pageInstanceId: nil)
        }
        LxAppCore.setCurrentApp(appId: appId, path: "/", pageInstanceId: nil)

        let controller = LxAppViewController()
        // This app has no runtime tab config, so a refresh would remove this
        // probe. It detects an early UI update without booting the engine.
        let tabBar = LingXiaTabBar(frame: .zero)
        tabBar.appId = appId
        controller.currentTabBar = tabBar

        NotificationCenter.default.post(name: .tabBarStateChanged, object: appId)
        NotificationCenter.default.post(name: .tabBarStateChanged, object: appId)
        XCTAssertFalse(controller.isViewLoaded)
        XCTAssertNil(controller.rootContainer)
        XCTAssertTrue(controller.currentTabBar === tabBar)

        controller.loadViewIfNeeded()
        let rootContainer = controller.rootContainer
        // isViewLoaded alone is insufficient while setupUI is constructing
        // the containers. Exercise that intermediate state as well.
        controller.rootContainer = nil
        NotificationCenter.default.post(name: .tabBarStateChanged, object: appId)
        XCTAssertTrue(controller.currentTabBar === tabBar)

        controller.rootContainer = rootContainer
        rootContainer?.addSubview(tabBar)
        NotificationCenter.default.post(name: .tabBarStateChanged, object: appId)
        XCTAssertNil(controller.currentTabBar)
        XCTAssertNil(tabBar.superview)
    }
}
#endif
