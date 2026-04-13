import Foundation
import OSLog
import WebKit
import CLingXiaRustAPI
import CLingXiaSwiftAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// FFI callbacks dispatched from Rust via the generated bridge.
extension LxApp {

    nonisolated static func openExternalUrlString(_ urlString: String) -> Bool {
        guard let url = URL(string: urlString) else { return false }
        #if os(iOS)
        DispatchQueue.main.async {
            UIApplication.shared.open(url, options: [:], completionHandler: nil)
        }
        return true
        #elseif os(macOS)
        return NSWorkspace.shared.open(url)
        #else
        return false
        #endif
    }

    nonisolated static func openLxApp(
        appid: RustStr,
        path: RustStr,
        session_id: UInt64,
        presentation: Int32,
        panel_id: RustStr
    ) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()
        let panelIdString = panel_id.toString()
        guard session_id > 0 else { return false }

        return executeOnMain {
            if let controller = LxAppActiveHost.activeController {
                let openPresentation: LxAppOpenPresentation = presentation == 1 ? .panel : .normal
                Task { @MainActor in
                    _ = await controller.handleOpen(
                        appId: appIdString,
                        path: pathString,
                        sessionId: session_id,
                        presentation: openPresentation,
                        panelId: panelIdString
                    )
                }
            } else {
                LxAppCore.executeOpenLxApp(
                    appId: appIdString,
                    path: pathString,
                    sessionId: session_id,
                    presentation: presentation,
                    panelId: panelIdString
                )
            }
            return true
        }
    }

    nonisolated static func closeLxApp(appid: RustStr, session_id: UInt64) -> Bool {
        let appIdString = appid.toString()
        guard session_id > 0 else { return false }

        return executeOnMain {
            if let controller = LxAppActiveHost.activeController {
                Task { @MainActor in
                    _ = await controller.handleClose(appId: appIdString, sessionId: session_id)
                }
            } else {
                #if os(iOS)
                iOSLxApp.closeLxApp(appId: appIdString, sessionId: session_id)
                #endif
            }
            return true
        }
    }

    nonisolated static func navigate(appid: RustStr, path: RustStr, animation_type: Int32) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()

        let animationType: LxAppAnimation
        switch animation_type {
        case 1: animationType = .push
        case 2: animationType = .pop
        default: animationType = .none
        }

        return executeOnMain {
            if let controller = LxAppActiveHost.activeController,
               let session = controller.session(forAppId: appIdString) {
                controller.navigate(LxAppNavigateRequest(
                    sessionId: session.id,
                    path: pathString,
                    animation: animationType
                ))
            } else {
                LxAppPlatform.navigate(appId: appIdString, path: pathString, animationType: animationType)
            }
            return true
        }
    }

    nonisolated static func updateTabBarUI(appid: RustStr) -> Bool {
        let appIdString = appid.toString()
        return executeOnMain {
            NotificationCenter.default.post(name: .tabBarStateChanged, object: appIdString)
            return true
        }
    }

    nonisolated static func updateNavBarUI(appid: RustStr) -> Bool {
        let appIdString = appid.toString()
        return executeOnMain {
            NavigationBarStateManager.shared.refreshState(for: appIdString)
            return true
        }
    }

    nonisolated static func updateOrientationUI(appid: RustStr) -> Bool {
        let appIdString = appid.toString()
        return executeOnMain {
            #if os(iOS)
            guard let instance = iOSLxApp.getInstanceUnsafe(),
                  let manager = instance.currentLxAppManager else {
                return false
            }
            return manager.applyOrientationFromRuntime(for: appIdString)
            #else
            return true
            #endif
        }
    }

    nonisolated static func openUrl(
        owner_appid: RustStr,
        owner_session_id: UInt64,
        url: RustStr,
        target: Int32
    ) -> Bool {
        let ownerAppId = owner_appid.toString()
        let urlString = url.toString()
        let selfTarget: Int32 = 1
        let newBrowserTab: Int32 = 2

        guard target == selfTarget || target == newBrowserTab else {
            return openExternalUrlString(urlString)
        }

        guard !ownerAppId.isEmpty, owner_session_id > 0 else { return false }

        #if os(macOS)
        if target == selfTarget && ownerAppId == getBuiltinBrowserAppId().toString() {
            let scheme = URL(string: urlString)?.scheme?.lowercased()
            if let scheme, scheme != "http", scheme != "https" {
                return openExternalUrlString(urlString)
            }
            if executeOnMain({ macOSLxApp.consumeSelfTargetNavigationInActiveBrowserTab(urlString: urlString) }) {
                return true
            }
        }
        #endif

        guard let openedTab = openBrowserTab(ownerAppId, owner_session_id, urlString) else {
            return false
        }
        let tabId = openedTab.toString()
        guard !tabId.isEmpty else { return false }

        #if os(macOS)
        return executeOnMain { macOSLxApp.presentInternalBrowserTab(tabId: tabId) }
        #elseif os(iOS)
        return executeOnMain { LxAppBrowserOverlay.show(tabId: tabId) }
        #else
        return false
        #endif
    }

    public static func handleAppLink(url: URL) {
        guard url.scheme == "https" else { return }
        let _ = onApplinkReceived(url.absoluteString)
    }

    nonisolated static func isPushEnabled() -> Bool {
        #if os(iOS)
        return iOSPushManager.isPushEnabledSync()
        #else
        return false
        #endif
    }

    nonisolated static func showToast(options: ToastOptions) {
        let title = options.title.toString()
        let image = options.image.toString()
        let icon = options.icon
        let duration = options.duration
        let mask = options.mask
        let position = options.position

        executeOnMain {
            LxAppToast.showToast(
                title: title,
                icon: icon,
                image: image.isEmpty ? nil : image,
                duration: duration,
                mask: mask,
                position: position
            )
        }
    }

    nonisolated static func showModal(options: ModalOptions, callback_id: UInt64) {
        LxAppModal.showModal(options: options, callback_id: callback_id)
    }

    @MainActor static func showModal(_ options: [String: Any], callback_id: UInt64) {
        LxAppModal.showModal(options, callback_id: callback_id)
    }

    nonisolated static func showActionSheet(options: ActionSheetOptions, callback_id: UInt64) {
        LxAppActionSheet.showActionSheet(options: options, callback_id: callback_id)
    }

    @MainActor static func showActionSheet(_ options: [String: Any], callback_id: UInt64) {
        LxAppActionSheet.showActionSheet(options, callback_id: callback_id)
    }

    nonisolated static func showPopup(
        appid: RustStr,
        path: RustStr,
        width_ratio: Double,
        height_ratio: Double,
        position: PopupPositionBridge
    ) -> Bool {
        let appIdString = appid.toString()
        let pathString = path.toString()
        let displayPosition = position.toDisplayPosition()

        return executeOnMain {
            LxAppPopup.showPopup(
                appId: appIdString,
                path: pathString,
                widthRatio: width_ratio,
                heightRatio: height_ratio,
                position: displayPosition
            )
        }
    }

    nonisolated static func hidePopup(appid: RustStr) -> Bool {
        let appIdString = appid.toString()
        return executeOnMain { LxAppPopup.hidePopup(appId: appIdString) }
    }

    nonisolated static func reviewDocument(file_path filePath: RustStr, mime_type mimeType: RustStr, show_menu showMenu: Bool) -> Bool {
        let pathString = filePath.toString()
        let mimeString = mimeType.toString()
        return executeOnMain {
            LxAppFile.reviewDocument(
                path: pathString,
                mimeType: mimeString.isEmpty ? nil : mimeString,
                showMenu: showMenu
            )
        }
    }

    nonisolated static func openDocumentExternal(file_path filePath: RustStr, mime_type mimeType: RustStr, show_menu showMenu: Bool) -> Bool {
        let pathString = filePath.toString()
        let mimeString = mimeType.toString()
        return executeOnMain {
            LxAppFile.openExternal(
                path: pathString,
                mimeType: mimeString.isEmpty ? nil : mimeString,
                showMenu: showMenu
            )
        }
    }

    nonisolated static func openDocument(file_path filePath: RustStr, mime_type mimeType: RustStr, show_menu showMenu: Bool) -> Bool {
        if reviewDocument(file_path: filePath, mime_type: mimeType, show_menu: showMenu) {
            return true
        }
        return openDocumentExternal(file_path: filePath, mime_type: mimeType, show_menu: showMenu)
    }

    nonisolated static func chooseFile(
        title: RustStr,
        default_path defaultPath: RustStr,
        multiple: Bool,
        filters_json filtersJson: RustStr,
        callback_id callbackId: UInt64
    ) -> Bool {
        let titleString = title.toString()
        let defaultPathString = defaultPath.toString()
        let filtersJsonString = filtersJson.toString()
        return executeOnMain {
            LxAppFile.chooseFile(
                title: titleString,
                defaultPath: defaultPathString,
                multiple: multiple,
                filtersJson: filtersJsonString,
                callbackId: callbackId
            )
        }
    }

    nonisolated static func chooseDirectory(
        title: RustStr,
        default_path defaultPath: RustStr,
        callback_id callbackId: UInt64
    ) -> Bool {
        let titleString = title.toString()
        let defaultPathString = defaultPath.toString()
        return executeOnMain {
            LxAppFile.chooseDirectory(
                title: titleString,
                defaultPath: defaultPathString,
                callbackId: callbackId
            )
        }
    }

    nonisolated static func revealInFileManager(path: RustStr) -> Bool {
        let pathString = path.toString()
        #if os(macOS)
        return executeOnMain {
            var isDirectory: ObjCBool = false
            guard FileManager.default.fileExists(atPath: pathString, isDirectory: &isDirectory) else {
                return false
            }
            let url = URL(fileURLWithPath: pathString)
            if isDirectory.boolValue {
                return NSWorkspace.shared.open(url)
            }
            NSWorkspace.shared.activateFileViewerSelecting([url])
            return true
        }
        #else
        let _ = pathString
        return false
        #endif
    }

    nonisolated static func hideToast() {
        executeOnMain { LxAppToast.hideToast() }
    }
}

#if os(iOS)
typealias LxAppPlatform = iOSLxApp
#elseif os(macOS)
typealias LxAppPlatform = macOSLxApp
#endif

// Sendable Conformance for FFI Types
extension ToastIcon: @unchecked Sendable {}
extension ToastPosition: @unchecked Sendable {}
extension GroupAlignment: @unchecked Sendable {}
