import SwiftUI
import UIKit
import lingxia
import Foundation
import os.log

/// C function exported from lingxia-lib Rust crate
@_silgen_name("lingxia_register_extensions")
func lingxia_register_extensions()

public struct ContentView: View {
    // Use a global flag instead of @State to avoid SwiftUI update cycle issues
    private static var hasInitialized = false

    public var body: some View {
        Color.clear
            .onAppear {
                if !Self.hasInitialized {
                    Self.hasInitialized = true

                    // Register custom extensions before initialization
                    LxApp.registerExtensions = {
                        lingxia_register_extensions()
                    }

                    // Enable WebView debugging BEFORE LxApp.initialize()
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
