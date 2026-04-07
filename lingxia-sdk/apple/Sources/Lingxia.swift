/// Top-level entry point for the LingXia SDK.
public enum Lingxia {

    /// Initialize the LingXia SDK and automatically open the Home LxApp.
    ///
    /// Call this once, before any LxApp UI is presented.
    /// Set `LxApp.skipAutoOpenWindow` / `LxApp.openLxAppHandler` / `LxApp.navigationHandler`
    /// before calling this if you need custom window management.
    @MainActor
    public static func initialize() {
        LxAppCore.initializeCore()
        #if os(iOS)
        iOSLxApp.initialize()
        #elseif os(macOS)
        _ = macOSLxApp.initialize()
        #endif
    }
}
