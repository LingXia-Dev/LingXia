#if os(macOS)
import XCTest
@testable import lingxia

final class StaticSettingsSourceTests: XCTestCase {
    func testAbsentDestinationProducesNoStaticSource() {
        XCTAssertNil(LxAppStaticSettingsSource(nil))
    }

    func testEveryDestinationVariantProducesOneResolvableStaticSource() {
        let destinations: [LxAppGeneratedSettingsDestination] = [
            .controlAppPage(appId: "control", page: "settings", query: nil),
            .browserControlPage(route: "/settings", query: nil),
            .nativeAction(actionId: "preferences"),
        ]

        for destination in destinations {
            let source = LxAppStaticSettingsSource(destination)
            var calls = 0
            XCTAssertTrue(source?.activate(
                itemID: LxAppStaticSettingsSource.sidebarItemID,
                using: {
                    calls += 1
                    return true
                }
            ) == true)
            XCTAssertEqual(calls, 1)
            XCTAssertFalse(source?.activate(itemID: "runtime", using: { true }) == true)
        }
    }

    func testRuntimeSettingsIdentityCannotMasqueradeAsStaticEntry() {
        XCTAssertFalse(LxAppStaticSettingsSource.acceptsRuntimeSidebarAction(
            id: "settings",
            label: "Anything"
        ))
        XCTAssertFalse(LxAppStaticSettingsSource.acceptsRuntimeSidebarAction(
            id: "other",
            label: " SETTINGS "
        ))
        XCTAssertFalse(LxAppStaticSettingsSource.acceptsRuntimeSidebarAction(
            id: LxAppStaticSettingsSource.sidebarItemID,
            label: "Anything"
        ))
        XCTAssertTrue(LxAppStaticSettingsSource.acceptsRuntimeSidebarAction(
            id: "help",
            label: "Help"
        ))
    }

    func testBrowserSettingsAndClearSiteDataRemainBrowserLocalRoutes() {
        XCTAssertEqual(BrowserLocalNavigation.settings.url, "lingxia://settings")
        XCTAssertEqual(BrowserLocalNavigation.settings.stableTabID, "settings")
        XCTAssertEqual(
            BrowserLocalNavigation.clearSiteData(tabID: "tab%201").url,
            "lingxia://settings#clear-site-data?tabId=tab%201"
        )
    }

    @MainActor
    func testDeclaredSurfaceMissFailsWithoutBuiltinFallback() {
        let runtime = FakeDeclaredSurfaceRuntime()
        XCTAssertFalse(LxAppDeclaredSurfaceVisibilityRouter.setVisible(
            in: runtime,
            id: "settings",
            visible: true,
            role: nil,
            edge: nil
        ))
        XCTAssertEqual(runtime.openedIDs, ["settings"])
        XCTAssertTrue(runtime.closedIDs.isEmpty)
    }
}

@MainActor
private final class FakeDeclaredSurfaceRuntime: LxAppDeclaredSurfaceVisibilityRouting {
    var openedIDs: [String] = []
    var closedIDs: [String] = []

    func openManagedSurface(id: String, role: String?, edge: String?) -> Bool {
        openedIDs.append(id)
        return false
    }

    func closeManagedSurface(id: String) -> Bool {
        closedIDs.append(id)
        return false
    }
}
#endif
