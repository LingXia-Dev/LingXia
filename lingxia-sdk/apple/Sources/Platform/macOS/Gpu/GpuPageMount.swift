import Foundation
#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Where a GPU page sits in a host: the macOS lxapp controller and the Runner share this, so
/// both mount, refresh and show a page the same way.
@MainActor
enum GpuPageMount {
    /// The page's GPU view, or nil when it renders in a WebView.
    static func view(appId: String, sessionId: UInt64, pageInstanceId: String?) -> GpuPageView? {
        let token = GpuPages.token(appId: appId, sessionId: sessionId, pageInstanceId: pageInstanceId)
        return token == 0 ? nil : GpuPages.view(for: token)
    }

    /// Pin `view` in `container` as a WebView page sits; a declared pull to refresh moves it.
    static func attach(_ view: GpuPageView, to container: NSView) {
        view.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(view)
        let pullToRefresh = MacPullToRefreshController.attachIfNeeded(to: view, in: container)
        NSLayoutConstraint.activate(pullToRefresh.pageConstraints(in: container))
        pullToRefresh.didAttachPage()
    }

    static func detach(_ view: NSView?) {
        view?.lxPullToRefreshController?.unmount()
        view?.removeFromSuperview()
    }

    /// As attaching a page's WebView does: the page's onShow, then onReady after its frame.
    static func shown(appId: String, path: String) {
        lingxia.onPageShow(appId, path)
    }
}
#endif
