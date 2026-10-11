#if os(iOS)
import UIKit
import WebKit
import XCTest
@testable import lingxia

final class NativeRootCompositionTests: XCTestCase {
    @MainActor
    func testLayerSignatureRejectsOtherRootsAndScrolledAnchors() {
        let scroll = UIScrollView(frame: CGRect(x: 0, y: 0, width: 200, height: 100))
        scroll.contentSize = CGSize(width: 216, height: 120)
        let size = scroll.bounds.size
        XCTAssertTrue(NativeRootComposition.matches(scroll, extent: CGSize(width: 16, height: 20), size: size))
        XCTAssertFalse(NativeRootComposition.matches(scroll, extent: CGSize(width: 20, height: 20), size: size))
        scroll.contentOffset.y = 10
        XCTAssertFalse(NativeRootComposition.matches(scroll, extent: CGSize(width: 16, height: 20), size: size))
        XCTAssertNil(NativeRootComposition.rect(["x": 0, "y": Double.infinity, "width": 200, "height": 100]))
    }

    @MainActor
    func testFallbackVisibilityAndTeardown() {
        let web = WKWebView()
        let overlay = UIView()
        web.scrollView.addSubview(overlay)
        let container = NativeRootContainerView()
        let composition = NativeRootComposition(container: container, overlay: overlay, webView: web)
        composition.update(nil)
        XCTAssertTrue(container.superview === overlay)
        XCTAssertFalse(composition.sameLayer)
        composition.setActive(false)
        XCTAssertTrue(container.isHidden)
        composition.setActive(true)
        XCTAssertFalse(container.isHidden)
        composition.teardown()
        composition.update(nil)
        XCTAssertNil(container.superview, "late geometry must not resurrect an unmounted root")
    }

    @MainActor
    func testPaintOrderClippingAndTransparentHoles() {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 400))
        let host = UIView(frame: window.bounds)
        window.addSubview(host)
        let scroll = UIScrollView(frame: host.bounds)
        host.addSubview(scroll)
        let lower = UIButton(frame: CGRect(x: 0, y: 0, width: 80, height: 80))
        let upper = UIButton(frame: lower.frame)
        scroll.addSubview(lower)
        scroll.addSubview(upper)
        let router = NativeComponentHitRouter()
        // Reverse registration intentionally: paint order must decide the hit.
        router.registerNativeView(upper, in: scroll)
        router.registerNativeView(lower, in: scroll)
        let point = CGPoint(x: 20, y: 20)
        XCTAssertTrue(router.nativeView(at: point, in: host, webHit: host, event: nil) === upper)
        lower.layer.zPosition = 1
        XCTAssertTrue(router.nativeView(at: point, in: host, webHit: host, event: nil) === lower)
        scroll.clipsToBounds = true
        scroll.frame.size.width = 10
        XCTAssertNil(router.nativeView(at: point, in: host, webHit: host, event: nil))
        scroll.frame = host.bounds
        scroll.alpha = 0.005
        XCTAssertNil(router.nativeView(at: point, in: host, webHit: host, event: nil))
        scroll.alpha = 1
        router.unregisterNativeView(lower)
        router.unregisterNativeView(upper)
        let hole = TransparentRoot(frame: host.bounds)
        scroll.addSubview(hole)
        router.registerNativeView(hole, in: scroll)
        XCTAssertNil(router.nativeView(at: point, in: host, webHit: host, event: nil))
    }

    @MainActor
    func testCompositedRootCannotStealHTMLTouches() {
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 400))
        let host = UIView(frame: window.bounds)
        window.addSubview(host)
        let anchor = UIScrollView(frame: host.bounds)
        host.addSubview(anchor)
        let native = UIButton(frame: CGRect(x: 0, y: 0, width: 80, height: 80))
        anchor.addSubview(native)
        let html = UIView(frame: host.bounds)
        host.addSubview(html)
        let router = NativeComponentHitRouter()
        router.registerNativeView(native, in: anchor, requiresWebKitHit: true)
        let point = CGPoint(x: 20, y: 20)
        XCTAssertNil(router.nativeView(at: point, in: host, webHit: html, event: nil))
        XCTAssertTrue(router.nativeView(at: point, in: host, webHit: anchor, event: nil) === native)
        anchor.removeFromSuperview()
        XCTAssertNil(router.nativeView(at: point, in: host, webHit: anchor, event: nil))
    }
}

@MainActor
private final class TransparentRoot: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? { nil }
}
#endif
