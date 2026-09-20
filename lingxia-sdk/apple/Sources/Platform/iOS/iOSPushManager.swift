#if os(iOS)
import Foundation
import UIKit
import UserNotifications
import os.log
import CLingXiaRustAPI

/// iOS Push Manager
@MainActor
final class iOSPushManager: NSObject {

    public static let shared = iOSPushManager()

    nonisolated private static let log = OSLog(subsystem: "LingXia", category: "Push")
    nonisolated private static let authorizationDefaultsKey = "app.lingxia.push.authorization-enabled"
    nonisolated private static let authorizationLock = NSLock()
    nonisolated(unsafe) private static var cachedIsPushEnabled: Bool? =
        UserDefaults.standard.object(forKey: authorizationDefaultsKey) as? Bool

    private var deviceToken: String?

    private override init() {
        super.init()
    }

    /// Check if push notifications are enabled (async version)
    /// Returns true if authorized or provisional, false otherwise
    nonisolated public static func isPushEnabled(completion: @escaping (Bool) -> Void) {
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            let isEnabled = settings.authorizationStatus == .authorized || settings.authorizationStatus == .provisional
            setCachedPushEnabled(isEnabled)
            completion(isEnabled)
        }
    }

    /// Return the latest authorization state without ever blocking the main thread.
    nonisolated public static func isPushEnabledSync() -> Bool {
        if let cached = cachedPushEnabled() {
            isPushEnabled { _ in }
            return cached
        }

        let refreshed = DispatchSemaphore(value: 0)
        isPushEnabled { _ in refreshed.signal() }
        guard !Thread.isMainThread else { return false }

        _ = refreshed.wait(timeout: .now() + 1)
        return cachedPushEnabled() ?? false
    }

    nonisolated private static func cachedPushEnabled() -> Bool? {
        authorizationLock.lock()
        defer { authorizationLock.unlock() }
        return cachedIsPushEnabled
    }

    nonisolated private static func setCachedPushEnabled(_ enabled: Bool) {
        authorizationLock.lock()
        defer { authorizationLock.unlock() }
        cachedIsPushEnabled = enabled
        UserDefaults.standard.set(enabled, forKey: authorizationDefaultsKey)
    }

    /// Claim the notification-center delegate, and nothing else.
    ///
    /// A tap that launched the process is delivered right after
    /// `didFinishLaunching`, and iOS drops it when no delegate is set yet — so
    /// this has to run before the runtime exists, and must not touch it.
    /// `activate_notification` holds such a token until the intent store is up.
    public static func installDelegate() {
        UNUserNotificationCenter.current().delegate = shared
    }

    /// Initialize push manager
    public func initialize() {
        UNUserNotificationCenter.current().delegate = self

        // Check current authorization status first
        UNUserNotificationCenter.current().getNotificationSettings { [weak self] settings in
            let authorizationStatus = settings.authorizationStatus
            Self.setCachedPushEnabled(
                authorizationStatus == .authorized || authorizationStatus == .provisional
            )

            Task { @MainActor [authorizationStatus] in
                guard let self else { return }

                switch authorizationStatus {
                case .authorized, .provisional:
                    // Authorized - register for remote notifications
                    os_log("Notification permission authorized, registering for remote notifications", log: Self.log, type: .info)
                    UIApplication.shared.registerForRemoteNotifications()
                case .notDetermined:
                    // A device token needs no authorization, and declaring the
                    // capability must not prompt: the product asks through
                    // `lx.app.notification.requestPermission()`.
                    UIApplication.shared.registerForRemoteNotifications()
                case .denied:
                    os_log("Notification permission denied", log: Self.log, type: .info)
                case .ephemeral:
                    os_log("Ephemeral notification permission", log: Self.log, type: .info)
                @unknown default:
                    os_log("Unknown notification authorization status", log: Self.log, type: .info)
                }
            }
        }
    }

    /// Handle device token registration
    public func didRegisterForRemoteNotifications(withDeviceToken deviceToken: Data) {
        let tokenString = deviceToken.map { String(format: "%02.2hhx", $0) }.joined()
        self.deviceToken = tokenString

        os_log("✅ Device token registered: %{public}@", log: Self.log, type: .info, tokenString)

        // Send token to native
        let _ = onPushTokenReceived(tokenString)
    }

    /// Handle device token registration failure
    public func didFailToRegisterForRemoteNotifications(withError error: Error) {
        LXLog.error("Failed to register for remote notifications", category: "Push", error: error)
    }

    /// Handle incoming remote notification
    public func didReceiveRemoteNotification(_ userInfo: [AnyHashable: Any],
                                           fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void) {
        os_log("Received remote notification: %{public}@", log: Self.log, type: .info, String(describing: userInfo))

        // Process the notification
        processNotificationData(userInfo: userInfo, trigger: "background")

        completionHandler(.newData)
    }

    /// Handle incoming remote notification (static method)
    public static func didReceiveRemoteNotification(_ userInfo: [AnyHashable: Any],
                                                   fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void) {
        shared.didReceiveRemoteNotification(userInfo, fetchCompletionHandler: completionHandler)
    }

    /// Convenient method for handling remote notifications without dispatch delay
    /// Use this in AppDelegate for launch notifications
    public static func handleRemoteNotification(_ userInfo: [AnyHashable: Any]) {
        // This is a launch notification (app started from notification)
        shared.processNotificationData(userInfo: userInfo, trigger: "launch")
    }
}

// UNUserNotificationCenterDelegate
extension iOSPushManager: UNUserNotificationCenterDelegate {

    /// Handle notification when app is in foreground
    nonisolated public func userNotificationCenter(_ center: UNUserNotificationCenter,
                                     willPresent notification: UNNotification,
                                     withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {

        let userInfo = notification.request.content.userInfo
        os_log("Received notification in foreground: %{public}@", log: Self.log, type: .info, String(describing: userInfo))

        // Process the notification directly (foreground - no forwarding to Rust)
        os_log("Foreground notification - not forwarding to Rust", log: Self.log, type: .info)

        if userInfo[MacLocalNotification.localMarker] != nil {
            // Only a scheduled notification gets here while active.
            completionHandler([.banner, .list, .sound])
            return
        }

        // Remote push still surfaces a banner while the app is foreground.
        completionHandler([.banner, .sound, .badge])
    }

    /// Handle notification tap when app is in background or not running
    nonisolated public func userNotificationCenter(_ center: UNUserNotificationCenter,
                                     didReceive response: UNNotificationResponse,
                                     withCompletionHandler completionHandler: @escaping () -> Void) {

        let userInfo = response.notification.request.content.userInfo
        os_log("User tapped notification: %{public}@", log: Self.log, type: .info, String(describing: userInfo))

        if userInfo[MacLocalNotification.localMarker] != nil {
            let token = userInfo[MacLocalNotification.tokenKey] as? String ?? ""
            DispatchQueue.main.async {
                // Resolve even an empty token: the host still has to bring the
                // product forward and say the target is gone.
                let _ = onNotificationActivated(token)
            }
            completionHandler()
            return
        }

        // Process the notification with tap trigger
        self.processNotificationData(userInfo: userInfo, trigger: "tap")

        completionHandler()
    }

    /// Process notification data and handle applink if present
    nonisolated private func processNotificationData(userInfo: [AnyHashable: Any], trigger: String) {
        let pushTrigger: PushTrigger
        switch trigger {
        case "background":
            pushTrigger = .Background
        case "tap":
            pushTrigger = .Tap
        case "launch":
            pushTrigger = .Launch
        default:
            LXLog.error("Unknown trigger type: \(trigger)", category: "Push")
            return
        }

        // Check for applink field and send to native
        if let applink = userInfo["applink"] as? String {
            os_log("Found applink in push notification (trigger: %{public}@): %{public}@", log: Self.log, type: .info, trigger, applink)
            let _ = onPushlinkReceived(applink, pushTrigger)
        }
    }
}

#endif
