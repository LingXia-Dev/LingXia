#if os(macOS)
import XCTest
@testable import lingxia

final class StaticSettingsSourceTests: XCTestCase {
    func testAbsentDestinationProducesNoStaticSource() {
        XCTAssertNil(LxAppStaticSettingsSource(nil))
        XCTAssertNil(LxAppStaticSettingsSource.fromBootstrapJSON("null"))
        XCTAssertNil(LxAppStaticSettingsSource.fromBootstrapJSON("{}"))
    }

    @MainActor
    func testAddonDestinationProducesFooterWithoutBundledDestination() throws {
        let app = try JSONDecoder().decode(
            LxAppGeneratedAppConfig.self,
            from: Data(#"{"productName":"Native host"}"#.utf8)
        )
        XCTAssertNil(app.settingsDestination)

        let source = LxAppStaticSettingsSource.fromBootstrapJSON(
            #"{"kind":"nativeAction","actionId":"preferences"}"#
        )
        XCTAssertEqual(source?.destinationKind, .nativeAction)
        let footer = LxAppStaticSettingsSource.mergeFooter(runtimeItems: [], source: source)
        XCTAssertEqual(footer.map(\.id), [LxAppStaticSettingsSource.sidebarItemID])
        XCTAssertEqual(footer.first?.sidebarActionSource, .staticSettings)
        var calls = 0
        XCTAssertTrue(source?.activate(itemID: LxAppStaticSettingsSource.sidebarItemID) {
            calls += 1
            return true
        } == true)
        XCTAssertEqual(calls, 1)
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

    func testOnlyReservedTypedIdentityCollidesWithStaticEntry() {
        XCTAssertFalse(LxAppStaticSettingsSource.acceptsRuntimeSidebarAction(
            id: LxAppStaticSettingsSource.sidebarItemID
        ))
        XCTAssertTrue(LxAppStaticSettingsSource.acceptsRuntimeSidebarAction(
            id: "settings"
        ))
        XCTAssertTrue(LxAppStaticSettingsSource.acceptsRuntimeSidebarAction(
            id: "help"
        ))
    }

    @MainActor
    func testStaticFooterExistsOnlyForAConfiguredTypedSource() {
        let runtime = [
            LxAppUIActionItem(
                id: "settings",
                label: "Settings",
                iconURL: URL(fileURLWithPath: "/tmp/settings.svg")
            ),
            LxAppUIActionItem(
                id: LxAppStaticSettingsSource.sidebarItemID,
                label: "Runtime collision",
                iconURL: nil
            ),
        ]

        let absent = LxAppStaticSettingsSource.mergeFooter(
            runtimeItems: runtime,
            source: nil
        )
        XCTAssertEqual(absent.map(\.id), ["settings"])
        XCTAssertTrue(absent.allSatisfy { $0.sidebarActionSource == .runtime })

        let source = LxAppStaticSettingsSource(
            .browserControlPage(route: "/settings", query: nil)
        )
        let configured = LxAppStaticSettingsSource.mergeFooter(
            runtimeItems: runtime,
            source: source
        )
        XCTAssertEqual(
            configured.map(\.id),
            ["settings", LxAppStaticSettingsSource.sidebarItemID]
        )
        XCTAssertEqual(configured.last?.sidebarActionSource, .staticSettings)
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
