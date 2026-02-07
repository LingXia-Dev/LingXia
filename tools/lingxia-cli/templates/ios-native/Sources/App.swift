import SwiftUI
import lingxia

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
