import XCTest
@testable import lingxia

final class NativeComponentMessageAdmissionTests: XCTestCase {
    private let binding = NativeComponentPageBinding(
        pageInstanceID: "page-instance-1",
        webViewIdentity: 41,
        attachmentGeneration: 7
    )!

    func testOnlyCurrentTopLevelLxAppPageIsAdmitted() {
        let source = NativeComponentSurfaceBinding.lxAppPage(binding)
        XCTAssertTrue(source.admits(
            isMainFrame: true,
            currentPageInstanceID: "page-instance-1",
            currentWebViewIdentity: 41,
            currentAttachmentGeneration: 7
        ))
        XCTAssertFalse(source.admits(
            isMainFrame: false,
            currentPageInstanceID: "page-instance-1",
            currentWebViewIdentity: 41,
            currentAttachmentGeneration: 7
        ))
    }

    func testBrowserControlAndExternalSurfacesAreAlwaysUnproven() {
        XCTAssertFalse(NativeComponentSurfaceBinding.unproven.admits(
            isMainFrame: true,
            currentPageInstanceID: "page-instance-1",
            currentWebViewIdentity: 41,
            currentAttachmentGeneration: 7
        ))
    }

    func testStalePageViewAndGenerationAreRejected() {
        let source = NativeComponentSurfaceBinding.lxAppPage(binding)
        for (page, view, generation) in [
            ("page-instance-2", UInt(41), UInt64(7)),
            ("page-instance-1", UInt(42), UInt64(7)),
            ("page-instance-1", UInt(41), UInt64(8)),
        ] {
            XCTAssertFalse(source.admits(
                isMainFrame: true,
                currentPageInstanceID: page,
                currentWebViewIdentity: view,
                currentAttachmentGeneration: generation
            ))
        }
    }

    func testEmptyOrZeroBindingsCannotBecomeLxAppAuthority() {
        XCTAssertNil(NativeComponentPageBinding(
            pageInstanceID: " ", webViewIdentity: 41, attachmentGeneration: 7))
        XCTAssertNil(NativeComponentPageBinding(
            pageInstanceID: "page", webViewIdentity: 0, attachmentGeneration: 7))
        XCTAssertNil(NativeComponentPageBinding(
            pageInstanceID: "page", webViewIdentity: 41, attachmentGeneration: 0))
    }
}
