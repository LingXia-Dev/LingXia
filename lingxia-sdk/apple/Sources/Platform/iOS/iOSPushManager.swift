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

    private var deviceToken: String?

    private override init() {
        super.init()
    }

    /// Check if push notifications are enabled (async version)
    /// Returns true if authorized or provisional, false otherwise
    nonisolated public static func isPushEnabled(completion: @escaping (Bool) -> Void) {
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            let isEnabled = settings.authorizationStatus == .authorized || settings.authorizationStatus == .provisional
            completion(isEnabled)
        }
    }

    /// Check if push notifications are enabled (sync version for FFI)
    /// Returns true if authorized or provisional, false otherwise
    nonisolated public static func isPushEnabledSync() -> Bool {
        let resultPointer = UnsafeMutablePointer<Bool>.allocate(capacity: 1)
        resultPointer.initialize(to: false)
        defer {
            resultPointer.deinitialize(count: 1)
            resultPointer.deallocate()
        }

        let semaphore = DispatchSemaphore(value: 0)

        isPushEnabled { isEnabled in
            resultPointer.pointee = isEnabled
            semaphore.signal()
        }

        semaphore.wait()
        return resultPointer.pointee
    }

    /// Initialize push manager
    public func initialize() {
        UNUserNotificationCenter.current().delegate = self

        // Check current authorization status first
        UNUserNotificationCenter.current().getNotificationSettings { [weak self] settings in
            let authorizationStatus = settings.authorizationStatus

            Task { @MainActor [authorizationStatus] in
                guard let self else { return }

                switch authorizationStatus {
                case .authorized, .provisional:
                    // Authorized - register for remote notifications
                    os_log("Notification permission authorized, registering for remote notifications", log: Self.log, type: .info)
                    UIApplication.shared.registerForRemoteNotifications()
                case .notDetermined:
                    // Request permission first
                    os_log("Notification permission not determined, requesting permission", log: Self.log, type: .info)
                    self.requestPermission()
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

    /// Request notification permission
    private func requestPermission() {
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) { granted, _ in
            DispatchQueue.main.async {
                if granted {
                    os_log("Notification permission granted, registering for remote notifications", log: Self.log, type: .info)
                    UIApplication.shared.registerForRemoteNotifications()
                } else {
                    os_log("Notification permission denied by user", log: Self.log, type: .info)
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

        // Show notification even when app is in foreground
        completionHandler([.banner, .sound, .badge])
    }

    /// Handle notification tap when app is in background or not running
    nonisolated public func userNotificationCenter(_ center: UNUserNotificationCenter,
                                     didReceive response: UNNotificationResponse,
                                     withCompletionHandler completionHandler: @escaping () -> Void) {

        let userInfo = response.notification.request.content.userInfo
        os_log("User tapped notification: %{public}@", log: Self.log, type: .info, String(describing: userInfo))

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
