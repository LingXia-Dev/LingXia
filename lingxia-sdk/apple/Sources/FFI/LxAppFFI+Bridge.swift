import Foundation
import Darwin
import OSLog
import WebKit
import CLingXiaRustAPI
import CLingXiaSwiftAPI

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
import ServiceManagement
#endif

private let lxAppFFILog = OSLog(subsystem: "LingXia", category: "LxAppFFI")

/// FFI callbacks dispatched from Rust via the generated bridge.
extension LxApp {

    nonisolated static func displayLanguageChanged() {
        DispatchQueue.main.async {
            NotificationCenter.default.post(
                name: Lingxia.displayLanguageDidChangeNotification,
                object: nil
            )
        }
    }

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
                let request = LxAppOpenRequest(
                    appId: appIdString,
                    path: pathString,
                    presentation: openPresentation,
                    panelId: panelIdString.isEmpty ? nil : panelIdString
                )
                if controller.hasInterceptors {
                    Task { @MainActor in
                        _ = await controller.handleOpen(
                            appId: appIdString,
                            path: pathString,
                            sessionId: session_id,
                            presentation: openPresentation,
                            panelId: panelIdString
                        )
                    }
                    return true
                }
                do {
                    _ = try controller.openSync(
                        request,
                        sessionId: session_id
                    )
                    return true
                } catch {
                    LXLog.error(
                        "openLxApp rejected by active controller for \(appIdString) path=\(pathString)",
                        category: "LxAppFFI",
                        error: error
                    )
                    return false
                }
            } else {
                return LxAppCore.executeOpenLxApp(
                    appId: appIdString,
                    path: pathString,
                    sessionId: session_id,
                    presentation: presentation,
                    panelId: panelIdString
                )
            }
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
                #elseif os(macOS)
                // Production macOS hosts activate a shell but no controller, so the
                // runtime-driven close must route to the active shell — otherwise an
                // lxapp restart's close-wait times out and the shell keeps stale state.
                LxAppActiveHost.activeShell?.handleRuntimeClose(
                    appId: appIdString, sessionId: session_id)
                #endif
            }
            return true
        }
    }

    nonisolated static func presentSurface(
        id: RustStr,
        appid: RustStr,
        path: RustStr,
        session_id: UInt64,
        page_instance_id: RustStr,
        content: Int32,
        kind: Int32,
        width: Double,
        height: Double,
        width_ratio: Double,
        height_ratio: Double,
        position: Int32,
        role: Int32,
        close_button: Bool,
        dismiss_on_outside: Bool,
        modal: Bool,
        ephemeral_web_data: Bool,
        url_callback: Bool
    ) -> Bool {
        let idString = id.toString()
        let appIdString = appid.toString()
        let pathString = path.toString()
        let pageInstanceId = page_instance_id.toString()
        guard !idString.isEmpty, !appIdString.isEmpty, session_id > 0 else {
            LXLog.error(
                "presentSurface rejected invalid args id=\(idString) app=\(appIdString) session=\(session_id)",
                category: "LxAppFFI"
            )
            return false
        }

        return executeOnMain {
            LxAppSurface.present(
                id: idString,
                appId: appIdString,
                path: pathString,
                sessionId: session_id,
                pageInstanceId: pageInstanceId,
                content: content,
                kind: kind,
                width: width,
                height: height,
                widthRatio: width_ratio,
                heightRatio: height_ratio,
                position: position,
                role: role,
                closeButton: close_button,
                dismissOnOutside: dismiss_on_outside,
                modal: modal,
                ephemeralWebData: ephemeral_web_data,
                urlCallback: url_callback
            )
        }
    }

    /// Adaptive Surface Layout: the shared core derived a new window layout.
    nonisolated static func presentLayout(window_id: RustStr, layout_json: RustStr) -> Bool {
        let windowIdString = window_id.toString()
        let json = layout_json.toString()
        guard !windowIdString.isEmpty, !json.isEmpty else { return false }
        return executeOnMain {
            #if os(macOS)
            return LxAppLayoutReconciler.reconcile(windowId: windowIdString, json: json)
            #elseif os(iOS)
            return LxAppLayoutReconcileriOS.reconcile(windowId: windowIdString, json: json)
            #else
            _ = json
            return false
            #endif
        }
    }

    /// Show or hide a host-declared top-level surface (e.g. the AI-chat panel or
    /// terminal). An empty `edge` keeps the current placement; otherwise it
    /// overrides the declared edge for this show. Returns `false` when there is
    /// no host shell to manage the surface, or when `id` is not a declared
    /// surface.
    nonisolated static func setManagedSurfaceVisible(id: RustStr, visible: Bool, edge: RustStr) -> Bool {
        let idString = id.toString()
        let edgeString = edge.toString()
        guard !idString.isEmpty else { return false }
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            if visible {
                // Declared surfaces first; else fall back to built-in browser
                // routes (downloads/settings) opened as main browser tabs.
                if runtime.openManagedSurface(id: idString, edge: edgeString.isEmpty ? nil : edgeString) {
                    return true
                }
                return runtime.shell.openBuiltinShellSurface(id: idString)
            }
            return runtime.closeManagedSurface(id: idString)
            #else
            _ = visible
            return false
            #endif
        }
    }

    /// Flip a host-declared top-level surface's visibility. Returns `false` when
    /// there is no host shell, or when `id` is not a declared surface.
    nonisolated static func toggleManagedSurface(id: RustStr) -> Bool {
        let idString = id.toString()
        guard !idString.isEmpty else { return false }
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            return runtime.toggleManagedSurface(id: idString)
            #else
            return false
            #endif
        }
    }

    nonisolated static func closeSurface(id: RustStr, appid: RustStr, reason: RustStr) -> Bool {
        let idString = id.toString()
        let appIdString = appid.toString()
        let reasonString = reason.toString()
        guard !idString.isEmpty, !appIdString.isEmpty else { return false }

        return executeOnMain {
            LxAppSurface.close(id: idString, appId: appIdString, reason: reasonString)
        }
    }

    nonisolated static func showSurface(id: RustStr, appid: RustStr) -> Bool {
        let idString = id.toString()
        let appIdString = appid.toString()
        guard !idString.isEmpty, !appIdString.isEmpty else { return false }

        return executeOnMain {
            LxAppSurface.show(id: idString, appId: appIdString)
        }
    }

    nonisolated static func hideSurface(id: RustStr, appid: RustStr) -> Bool {
        let idString = id.toString()
        let appIdString = appid.toString()
        guard !idString.isEmpty, !appIdString.isEmpty else { return false }

        return executeOnMain {
            LxAppSurface.hide(id: idString, appId: appIdString)
        }
    }

    nonisolated static func exitApp() -> Bool {
        return executeOnMain {
            #if os(macOS)
            NSApp.terminate(nil)
            return true
            #elseif os(iOS)
            DispatchQueue.main.async {
                Darwin.exit(0)
            }
            return true
            #else
            return false
            #endif
        }
    }

    nonisolated static func setTrayBadge(text: RustStr) -> Bool {
        let value = text.toString()
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setTrayBadge(value.isEmpty ? nil : value)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setActivatorItems(items_json: RustStr) -> Bool {
        let json = items_json.toString()
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setRuntimeActivatorItems(json)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setShellPins(items_json: RustStr) -> Bool {
        let json = items_json.toString()
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setShellPins(json)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setTrayIcon(icon: RustStr) -> Bool {
        let value = icon.toString()
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setTrayIcon(value)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setTrayTitle(text: RustStr) -> Bool {
        let value = text.toString()
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setTrayTitle(value.isEmpty ? nil : value)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setTrayMenu(items_json: RustStr) -> Bool {
        let value = items_json.toString()
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setTrayMenu(value)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setTrayVisible(visible: Bool) -> Bool {
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setTrayVisible(visible)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setTrayClickIntercept(intercept: Bool) -> Bool {
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            runtime.setTrayClickIntercept(intercept)
            return true
            #else
            return true
            #endif
        }
    }

    nonisolated static func setAppBadge(text: RustStr) -> Bool {
        let value = text.toString()
        return executeOnMain {
            #if os(macOS)
            NSApp.dockTile.badgeLabel = value.isEmpty ? nil : value
            return true
            #elseif os(iOS)
            UIApplication.shared.applicationIconBadgeNumber = Int(value) ?? 0
            return true
            #else
            return false
            #endif
        }
    }

    // Launch-at-startup via the login-item registration of the main app
    // bundle. SMAppService is macOS 13+; older shells report unsupported (-1 /
    // false) and Rust surfaces that as an error. No main-thread hop: the calls
    // are thread-safe and register() can block briefly.

    /// 1 = enabled, 0 = disabled, -1 = unsupported on this shell.
    nonisolated static func autostartIsEnabled() -> Int32 {
        #if os(macOS)
        if #available(macOS 13.0, *) {
            return SMAppService.mainApp.status == .enabled ? 1 : 0
        }
        return -1
        #else
        return -1
        #endif
    }

    nonisolated static func autostartSetEnabled(enabled: Bool) -> Bool {
        #if os(macOS)
        if #available(macOS 13.0, *) {
            let service = SMAppService.mainApp
            do {
                switch (enabled, service.status) {
                case (true, .enabled), (false, .notFound), (false, .notRegistered):
                    // Already in the requested state; register()/unregister()
                    // would throw here, so idempotence has to be explicit.
                    break
                case (true, .requiresApproval):
                    // Registered but switched off by the user in System
                    // Settings; a plain register() throws. Re-registering from
                    // scratch prompts approval again.
                    try? service.unregister()
                    try service.register()
                case (true, _):
                    try service.register()
                case (false, _):
                    try service.unregister()
                }
                return true
            } catch {
                LXLog.error("autostart update failed", category: "LxAppFFI", error: error)
                return false
            }
        }
        return false
        #else
        return false
        #endif
    }

    /// Show the post-download update prompt. `state` is "ready" (downloaded →
    /// minimal sidebar callout; clicking it opens the notes card) or
    /// "ready-force" (forced → blocking notes card, no dismiss). `info_json`
    /// carries {version, releaseNotes, isForceUpdate} the card renders. Returns
    /// `true` only when a macOS shell is present — `false` tells Rust to fall
    /// back (restart when headless).
    nonisolated static func notifyAppUpdateReady(state: RustStr, info_json: RustStr) -> Bool {
        let stateString = state.toString()
        let infoJSON = info_json.toString()
        return executeOnMain {
            #if os(macOS)
            guard let runtime = LxAppMacAppUIRuntime.active else { return false }
            if stateString == "ready-force" {
                runtime.shell.presentUpdateReadyCard(infoJSON: infoJSON)
                return true
            }
            // Normal update: remember the notes, show the minimal callout. The
            // notes card opens when the user clicks the callout.
            runtime.shell.setPendingUpdateInfo(infoJSON)
            runtime.shell.presentUpdateReadyCallout(
                appName: runtime.appConfig.productName, state: .ready)
            return true
            #else
            _ = (stateString, infoJSON)
            return false
            #endif
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

    /// Async variant for lx.showTabBar/hideTabBar: registers a completion
    /// waiter, then posts the state change. The observers deliver on the
    /// main queue asynchronously, so completion is signaled by the observer
    /// that applies the change (TabBarUpdateWaiters.complete), not here.
    nonisolated static func updateTabBarUIAsync(appid: RustStr, callback_id: UInt64) {
        let appIdString = appid.toString()
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                TabBarUpdateWaiters.add(appIdString, callback_id)
            }
            NotificationCenter.default.post(name: .tabBarStateChanged, object: appIdString)
        }
    }

    nonisolated static func updateNavBarUI(appid: RustStr) -> Bool {
        let appIdString = appid.toString()
        return executeOnMain {
            NavigationBarStateManager.shared.refreshState(for: appIdString)
            // Mirror updateTabBarUI: notify imperative hosts (e.g. the runner's
            // AppKit navbar) so they re-render. Without this the runner's navbar
            // stays on its stale init state — the page instance (and thus the
            // real title) doesn't exist yet when the navbar is first applied.
            NotificationCenter.default.post(name: .navBarStateChanged, object: appIdString)
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
        let openTarget = OpenURLTarget(rawValue: target) ?? .external
        os_log(
            "openURL owner=%{public}@ session=%{public}llu target=%{public}d resolvedTarget=%{public}@ url=%{private}@",
            log: lxAppFFILog,
            type: .info,
            ownerAppId,
            owner_session_id,
            target,
            String(describing: openTarget),
            urlString
        )

        if let handler = LxApp.openUrlHandler {
            switch executeOnMain({ handler(ownerAppId, owner_session_id, urlString, openTarget) }) {
            case .handled(let accepted):
                os_log("openURL handled by custom handler accepted=%{public}@", log: lxAppFFILog, type: .info, String(accepted))
                return accepted
            case .useDefault:
                break
            }
        }

        guard openTarget == .selfTarget || openTarget == .newBrowserTab
            || openTarget == .asideBrowser else {
            return openExternalUrlString(urlString)
        }

        let browserEnabled = executeOnMain {
            (LxAppCore.capabilities & LxAppCore.capBrowser) != 0
        }
        guard browserEnabled else {
            return openExternalUrlString(urlString)
        }

        guard !ownerAppId.isEmpty, owner_session_id > 0 else { return false }

        #if os(macOS)
        if openTarget == .selfTarget {
            if ownerAppId == getBuiltinBrowserAppId().toString() {
                let scheme = URL(string: urlString)?.scheme?.lowercased()
                if let scheme, scheme != "http", scheme != "https" {
                    return openExternalUrlString(urlString)
                }
                if executeOnMain({ macOSLxApp.consumeSelfTargetNavigationInActiveBrowserTab(urlString: urlString) }) {
                    return true
                }
                return false
            }
        }
        #endif

        let openedTab = openTarget == .asideBrowser
            ? openAsideBrowserTab(ownerAppId, owner_session_id, urlString)
            : openBrowserTab(ownerAppId, owner_session_id, urlString)
        guard let openedTab else {
            return false
        }
        let tabId = openedTab.toString()
        guard !tabId.isEmpty else { return false }

        #if os(macOS)
        return executeOnMain { macOSLxApp.presentInternalBrowserTab(tabId: tabId) }
        #elseif os(iOS)
        return executeOnMain { LxAppBrowser.show(tabId: tabId) }
        #else
        return false
        #endif
    }

    nonisolated static func presentInternalBrowserTab(tab_id: RustStr) -> Bool {
        let tabId = tab_id.toString()
        #if os(macOS)
        return executeOnMain { macOSLxApp.presentInternalBrowserTab(tabId: tabId) }
        #elseif os(iOS)
        // A runtime-opened tab (e.g. a link that opens a new tab) must switch to
        // and display the new tab, same as a directly opened one.
        return executeOnMain { LxAppBrowser.show(tabId: tabId) }
        #else
        _ = tabId
        return false
        #endif
    }

    nonisolated static func prepareInternalBrowserTabForInput(tab_id: RustStr) -> Bool {
        let tabId = tab_id.toString()
        #if os(macOS)
        return executeOnMain { macOSLxApp.prepareInternalBrowserTabForInput(tabId: tabId) }
        #else
        _ = tabId
        return false
        #endif
    }

    nonisolated static func browserBookmarksChanged() {
        #if os(macOS)
        DispatchQueue.main.async {
            macOSLxApp.browserBookmarksChanged()
        }
        #endif
    }

    nonisolated static func share(
        title: RustStr,
        text: RustStr,
        url: RustStr,
        files_json filesJson: RustStr,
        callback_id callbackId: UInt64
    ) -> Bool {
        let titleString = title.toString()
        let textString = text.toString()
        let urlString = url.toString()
        let filesJsonString = filesJson.toString()
        return executeOnMain {
            LxAppShare.share(
                title: titleString,
                text: textString,
                url: urlString,
                filesJson: filesJsonString,
                callbackId: callbackId
            )
        }
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
