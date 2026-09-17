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
    static let shared = MacLocalNotification()
    private static let lock = NSLock()
    nonisolated(unsafe) private static var lastError = ""
    static let localMarker = "lx.local"

    static func installDelegate() {
        UNUserNotificationCenter.current().delegate = shared
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

    static func requestPermission() -> String {
        let center = UNUserNotificationCenter.current()
        let semaphore = DispatchSemaphore(value: 0)
        var status = "default"
        center.getNotificationSettings { settings in
            switch settings.authorizationStatus {
            case .authorized, .provisional:
                status = "granted"
                semaphore.signal()
            case .denied:
                status = "denied"
                semaphore.signal()
            case .notDetermined:
                center.requestAuthorization(options: [.alert, .sound]) { granted, error in
                    if let error {
                        setLastError(error.localizedDescription)
                    }
                    status = granted ? "granted" : "denied"
                    semaphore.signal()
                }
            @unknown default:
                status = "denied"
                semaphore.signal()
            }
        }
        _ = semaphore.wait(timeout: .now() + 30)
        return status
    }

    static func show(
        id: String,
        title: String,
        body: String,
        applink: String,
        deliverAtMs: Int64,
        silent: Bool
    ) -> String {
        let permission = requestPermission()
        guard permission == "granted" else {
            setLastError("notification permission is \(permission)")
            return ""
        }

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
            return id
        }

        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        var userInfo: [String: Any] = [localMarker: true]
        if !applink.isEmpty {
            userInfo["applink"] = applink
        }
        content.userInfo = userInfo
        if silent {
            content.sound = nil
        } else {
            content.sound = .default
        }

        let request = UNNotificationRequest(identifier: id, content: content, trigger: trigger)
        let semaphore = DispatchSemaphore(value: 0)
        var ok = true
        UNUserNotificationCenter.current().add(request) { error in
            if let error {
                setLastError(error.localizedDescription)
                ok = false
            }
            semaphore.signal()
        }
        _ = semaphore.wait(timeout: .now() + 10)
        return ok ? id : ""
    }

    static func cancel(id: String) -> Bool {
        let center = UNUserNotificationCenter.current()
        center.removePendingNotificationRequests(withIdentifiers: [id])
        center.removeDeliveredNotifications(withIdentifiers: [id])
        return true
    }

    static func cancelAll() -> Bool {
        let center = UNUserNotificationCenter.current()
        center.getPendingNotificationRequests { requests in
            let ids = requests.compactMap { request -> String? in
                request.content.userInfo[localMarker] != nil ? request.identifier : nil
            }
            if !ids.isEmpty {
                center.removePendingNotificationRequests(withIdentifiers: ids)
            }
        }
        center.getDeliveredNotifications { notes in
            let ids = notes.compactMap { note -> String? in
                note.request.content.userInfo[localMarker] != nil
                    ? note.request.identifier : nil
            }
            if !ids.isEmpty {
                center.removeDeliveredNotifications(withIdentifiers: ids)
            }
        }
        return true
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        if MacLocalNotification.isFrontmost() {
            completionHandler([])
            return
        }
        completionHandler([.banner, .sound])
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        if let applink = response.notification.request.content.userInfo["applink"] as? String,
           !applink.isEmpty {
            DispatchQueue.main.async {
                _ = onApplinkReceived(applink)
                #if os(macOS)
                NSApp.activate(ignoringOtherApps: true)
                #endif
            }
        }
        completionHandler()
    }
}
#endif
