#if os(macOS) || os(iOS)
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import Foundation
import UserNotifications

/// Local notifications for `lx.app.notification`. Does not request permission
/// on install — that waits for `requestPermission` / first `show`.
final class MacLocalNotification: NSObject, UNUserNotificationCenterDelegate {
    nonisolated(unsafe) static let shared = MacLocalNotification()
    private static let lock = NSLock()
    nonisolated(unsafe) private static var lastError = ""
    static let localMarker = "lx.local"

    /// A delegate the host installed first keeps everything that is not ours.
    nonisolated(unsafe) private static weak var hostDelegate: UNUserNotificationCenterDelegate?

    static func installDelegate() {
        let center = UNUserNotificationCenter.current()
        if let existing = center.delegate, existing !== shared {
            hostDelegate = existing
        }
        center.delegate = shared
    }

    static func takeLastError() -> String {
        lock.lock()
        defer { lock.unlock() }
        let value = lastError
        lastError = ""
        return value
    }

    nonisolated private static func setLastError(_ message: String) {
        lock.lock()
        lastError = message
        lock.unlock()
    }

    static func isFrontmost() -> Bool {
        let check = {
            #if os(macOS)
            NSApp.isActive
            #else
            UIApplication.shared.applicationState == .active
            #endif
        }
        if Thread.isMainThread {
            return check()
        }
        return DispatchQueue.main.sync(execute: check)
    }

    /// A value one completion handler writes and the calling thread reads.
    private final class Box<T>: @unchecked Sendable {
        private let lock = NSLock()
        private var stored: T
        init(_ value: T) { stored = value }
        var value: T {
            get { lock.lock(); defer { lock.unlock() }; return stored }
            set { lock.lock(); stored = newValue; lock.unlock() }
        }
    }

    private static func word(for status: UNAuthorizationStatus) -> String {
        switch status {
        case .authorized, .provisional, .ephemeral: return "granted"
        case .denied: return "denied"
        case .notDetermined: return "default"
        @unknown default: return "denied"
        }
    }

    /// Current permission, never prompting. Empty on failure.
    static func permission() -> String {
        let result = Box("")
        let semaphore = DispatchSemaphore(value: 0)
        UNUserNotificationCenter.current().getNotificationSettings { settings in
            result.value = word(for: settings.authorizationStatus)
            semaphore.signal()
        }
        if semaphore.wait(timeout: .now() + 10) == .timedOut {
            setLastError("notification settings did not answer")
            return ""
        }
        return result.value
    }

    /// `granted` / `denied`. Empty when the prompt is left unanswered.
    static func requestPermission() -> String {
        let current = permission()
        guard current == "default" || current == "granted" else { return current }

        let result = Box("")
        let semaphore = DispatchSemaphore(value: 0)
        // Repeat the request for authorized installs too: older versions only
        // requested alerts and sounds, so their authorization omitted badges.
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge]) {
            granted, error in
            if let error {
                setLastError(error.localizedDescription)
            }
            result.value = granted ? "granted" : "denied"
            semaphore.signal()
        }
        if semaphore.wait(timeout: .now() + 60) == .timedOut {
            setLastError("notification permission prompt was not answered")
            return ""
        }
        return result.value
    }

    /// `shown` / `scheduled` / `suppressed`. Empty on failure.
    static func show(
        id: String,
        title: String,
        body: String,
        applink: String,
        deliverAtMs: Int64,
        silent: Bool
    ) -> String {
        var trigger: UNNotificationTrigger?
        if deliverAtMs > 0 {
            let fire = Date(timeIntervalSince1970: TimeInterval(deliverAtMs) / 1000)
            if fire.timeIntervalSinceNow > 0.5 {
                trigger = UNTimeIntervalNotificationTrigger(
                    timeInterval: fire.timeIntervalSinceNow,
                    repeats: false
                )
            }
        }
        if trigger == nil && isFrontmost() {
            // Still an upsert: whatever the id held must not outlive this call.
            _ = cancel(id: id)
            return "suppressed"
        }

        let permission = requestPermission()
        guard permission == "granted" else {
            if !permission.isEmpty {
                setLastError("notification permission is \(permission)")
            }
            return ""
        }

        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        var userInfo: [String: Any] = [localMarker: true]
        if !applink.isEmpty {
            userInfo["applink"] = applink
        }
        content.userInfo = userInfo
        content.sound = silent ? nil : .default

        // A delivered copy is not replaced by a pending request with the same id.
        UNUserNotificationCenter.current().removeDeliveredNotifications(withIdentifiers: [id])
        let request = UNNotificationRequest(identifier: id, content: content, trigger: trigger)
        let failure = Box<String?>(nil)
        let semaphore = DispatchSemaphore(value: 0)
        UNUserNotificationCenter.current().add(request) { error in
            failure.value = error?.localizedDescription
            semaphore.signal()
        }
        if semaphore.wait(timeout: .now() + 10) == .timedOut {
            setLastError("notification center did not accept the request")
            return ""
        }
        if let message = failure.value {
            setLastError(message)
            return ""
        }
        return trigger == nil ? "shown" : "scheduled"
    }

    static func cancel(id: String) -> Bool {
        let center = UNUserNotificationCenter.current()
        center.removePendingNotificationRequests(withIdentifiers: [id])
        center.removeDeliveredNotifications(withIdentifiers: [id])
        return true
    }

    /// Waits for both listings so a `show` issued right after cannot be swept up.
    static func cancelAll() -> Bool {
        let center = UNUserNotificationCenter.current()
        let group = DispatchGroup()
        group.enter()
        center.getPendingNotificationRequests { requests in
            let ids = requests
                .filter { $0.content.userInfo[localMarker] != nil }
                .map(\.identifier)
            if !ids.isEmpty {
                center.removePendingNotificationRequests(withIdentifiers: ids)
            }
            group.leave()
        }
        group.enter()
        center.getDeliveredNotifications { notes in
            let ids = notes
                .filter { $0.request.content.userInfo[localMarker] != nil }
                .map(\.request.identifier)
            if !ids.isEmpty {
                center.removeDeliveredNotifications(withIdentifiers: ids)
            }
            group.leave()
        }
        if group.wait(timeout: .now() + 10) == .timedOut {
            setLastError("notification center did not list notifications")
            return false
        }
        return true
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        guard notification.request.content.userInfo[MacLocalNotification.localMarker] != nil else {
            if let host = MacLocalNotification.hostDelegate,
               host.userNotificationCenter?(
                   center, willPresent: notification, withCompletionHandler: completionHandler
               ) != nil {
                return
            }
            completionHandler([.banner, .list, .sound])
            return
        }
        // Only a scheduled notification gets here while frontmost, and the
        // product asked for it at this time: present it.
        completionHandler([.banner, .list, .sound])
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        let userInfo = response.notification.request.content.userInfo
        guard userInfo[MacLocalNotification.localMarker] != nil else {
            if let host = MacLocalNotification.hostDelegate,
               host.userNotificationCenter?(
                   center, didReceive: response, withCompletionHandler: completionHandler
               ) != nil {
                return
            }
            completionHandler()
            return
        }
        let applink = userInfo["applink"] as? String ?? ""
        DispatchQueue.main.async {
            if !applink.isEmpty {
                _ = onApplinkReceived(applink)
            }
            #if os(macOS)
            NSApp.activate(ignoringOtherApps: true)
            #endif
        }
        completionHandler()
    }
}
#endif
