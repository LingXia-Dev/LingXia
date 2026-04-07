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

                    // Enable WebView debugging BEFORE LxApp.initialize()
                    LxApp.enableWebViewDebugging()

                    LxApp.initialize()
                }
            }
    }
}

@main
public struct LxAppApp: App {
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
