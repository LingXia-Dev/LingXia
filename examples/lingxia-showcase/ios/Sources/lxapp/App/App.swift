import SwiftUI
import lingxia

public struct ContentView: View {
    // Use a global flag instead of @State to avoid SwiftUI update cycle issues
    private static var hasInitialized = false

    public var body: some View {
        Color.clear
            .onAppear {
                if !Self.hasInitialized {
                    Self.hasInitialized = true

                    // Enable WebView debugging BEFORE Lingxia.quickStart()
                    Lingxia.enableWebViewDebugging()
                    _ = try? Lingxia.quickStart()
                }
            }
    }
}

@main
public struct LxAppApp: App {
    public init() {
        // Before the first view appears: iOS hands a cold-start notification
        // tap over right after launch and drops it if nothing is listening.
        Lingxia.installNotificationDelegate()
    }

    public var body: some Scene {
        WindowGroup {
            ContentView()
                .onOpenURL { url in
                    Lingxia.handleAppLink(url: url)
                }
        }
    }
}
