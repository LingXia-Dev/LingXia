import SwiftUI
import UIKit
import lingxia
import Foundation
import os.log

public struct ContentView: View {
    // Use a global flag instead of @State to avoid SwiftUI update cycle issues
    private static var hasInitialized = false

    public var body: some View {
        Color.clear
            .onAppear {
                if !Self.hasInitialized {
                    Self.hasInitialized = true

                    // Enable WebView debugging BEFORE LxApp.initialize()
                    // This ensures debugging is enabled before the first WebView is created
                    LxApp.enableWebViewDebugging()

                    LxApp.initialize()
                }
            }
    }
}

public class AppDelegate: NSObject, UIApplicationDelegate {

    public func application(_ application: UIApplication,
                           didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {

        // Check if app was launched from a notification
        if let notificationUserInfo = launchOptions?[.remoteNotification] as? [AnyHashable: Any] {
            iOSPushManager.handleRemoteNotification(notificationUserInfo)
        }

        return true
    }

    public func application(_ application: UIApplication,
                           didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        iOSPushManager.shared.didRegisterForRemoteNotifications(withDeviceToken: deviceToken)
    }

    public func application(_ application: UIApplication,
                           didFailToRegisterForRemoteNotificationsWithError error: Error) {
        iOSPushManager.shared.didFailToRegisterForRemoteNotifications(withError: error)
    }

    public func application(_ application: UIApplication,
                           didReceiveRemoteNotification userInfo: [AnyHashable: Any],
                           fetchCompletionHandler completionHandler: @escaping (UIBackgroundFetchResult) -> Void) {
        iOSPushManager.didReceiveRemoteNotification(userInfo, fetchCompletionHandler: completionHandler)
    }
}

@main
public struct LxAppApp: App {
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    public init() { }

    public var body: some Scene {
        WindowGroup {
            ContentView()
                .onOpenURL { url in
                    LxApp.handleAppLink(url: url)
                }
        }
    }
}
