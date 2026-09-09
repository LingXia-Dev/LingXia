import Foundation

/// Native-component messages are an lxapp-page transport, not a browser
/// control-document transport. Browser/external WebViews therefore remain
/// explicitly unproven and never receive this handler.
enum NativeComponentSurfaceBinding: Equatable {
    case lxAppPage(NativeComponentPageBinding)
    case unproven

    func admits(
        isMainFrame: Bool,
        currentPageInstanceID: String?,
        currentWebViewIdentity: UInt?,
        currentAttachmentGeneration: UInt64?
    ) -> Bool {
        guard isMainFrame, case .lxAppPage(let binding) = self else { return false }
        return binding.pageInstanceID == currentPageInstanceID
            && binding.webViewIdentity == currentWebViewIdentity
            && binding.attachmentGeneration == currentAttachmentGeneration
    }
}

struct NativeComponentPageBinding: Equatable {
    let pageInstanceID: String
    let webViewIdentity: UInt
    let attachmentGeneration: UInt64

    init?(pageInstanceID: String, webViewIdentity: UInt, attachmentGeneration: UInt64) {
        let pageInstanceID = pageInstanceID.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !pageInstanceID.isEmpty, webViewIdentity != 0, attachmentGeneration != 0 else {
            return nil
        }
        self.pageInstanceID = pageInstanceID
        self.webViewIdentity = webViewIdentity
        self.attachmentGeneration = attachmentGeneration
    }
}
