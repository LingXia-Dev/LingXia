import Foundation
import os.log

/// Core notification handling logic shared between iOS and macOS
public class NotificationCore {
    private static let log = OSLog(subsystem: "LingXia", category: "NotificationCore")

    /// Notification names used by LingXia
    public struct NotificationNames {
        public static let switchPage = Notification.Name("LingXia.SwitchPage")
        public static let closeApp = Notification.Name("LingXia.CloseApp")
        public static let updateNavigationBar = Notification.Name("LingXia.UpdateNavigationBar")
        public static let updateTabBar = Notification.Name("LingXia.UpdateTabBar")
        public static let webViewReady = Notification.Name("LingXia.WebViewReady")
    }

    /// Notification user info keys
    public struct NotificationKeys {
        public static let appId = "appId"
        public static let path = "path"
        public static let isBackNavigation = "isBackNavigation"
        public static let disableAnimation = "disableAnimation"
        public static let webViewPtr = "webViewPtr"
    }

    /// Notification posting utilities
    public struct Poster {

        /// Posts a switch page notification
        public static func postSwitchPage(appId: String, path: String) {
            let userInfo: [String: Any] = [
                NotificationKeys.appId: appId,
                NotificationKeys.path: path
            ]

            NotificationCenter.default.post(
                name: NotificationNames.switchPage,
                object: nil,
                userInfo: userInfo
            )

            os_log("Posted switch page notification: appId=%{public}@, path=%{public}@",
                   log: log, type: .debug, appId, path)
        }

        /// Posts a close app notification
        public static func postCloseApp(appId: String) {
            let userInfo: [String: Any] = [
                NotificationKeys.appId: appId
            ]

            NotificationCenter.default.post(
                name: NotificationNames.closeApp,
                object: nil,
                userInfo: userInfo
            )

            os_log("Posted close app notification: appId=%{public}@",
                   log: log, type: .debug, appId)
        }
    }
}