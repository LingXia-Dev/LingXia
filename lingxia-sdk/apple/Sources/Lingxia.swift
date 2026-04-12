import Foundation

/// Top-level entry point for the LingXia SDK.
@MainActor
public enum Lingxia {
    static func resolvedShellConfiguration(
        from configuration: LxAppShellConfiguration,
        capabilities: LxAppCapabilities,
        homeAppId: String?
    ) -> LxAppShellConfiguration {
        var config = configuration
        guard case .hidden = config.sidebar,
              capabilities.contains(.shell),
              let homeAppId,
              !homeAppId.isEmpty else {
            return config
        }

        config.sidebar = .declarative(LxAppSidebarTree(sections: [
            LxAppSidebarSection(id: "home", tabs: [
                LxAppSidebarTab(
                    id: "home",
                    label: "Home",
                    icon: "house",
                    appId: homeAppId
                )
            ])
        ]))
        return config
    }

    /// Initialize the LingXia runtime without touching the view layer.
    ///
    /// Use this entry point when building a custom integration around
    /// `LxAppController` / `LxAppHostView`.
    @MainActor
    @discardableResult
    public static func initializeRuntime() throws -> LxAppRuntimeInfo {
        do {
            return try LxAppRuntime.shared.initialize()
        } catch LxAppRuntimeError.alreadyInitialized {
            if let info = LxAppRuntime.shared.info {
                return info
            }
            throw LxAppRuntimeError.initializationFailed(
                message: "runtime reported already initialized, but no runtime info is available"
            )
        }
    }

    /// Make a custom controller the active receiver for runtime-driven open /
    /// navigate / close callbacks. Advanced hosts that do not use the default
    /// shell should call this after creating their controller.
    @MainActor
    public static func activate(controller: LxAppController) {
        LxAppActiveHost.activate(controller: controller)
    }

    public static func enableWebViewDebugging() {
        LxApp.enableWebViewDebugging()
    }

    public static func handleAppLink(url: URL) {
        LxApp.handleAppLink(url: url)
    }

    /// One-call entry point: initialize runtime, create controller,
    /// build shell, show window, and open the home LxApp.
    ///
    /// ```swift
    /// @main struct MyApp: App {
    ///     init() {
    ///         try! Lingxia.quickStart()
    ///     }
    /// }
    /// ```
    @MainActor
    @discardableResult
    public static func quickStart(
        configuration: LxAppShellConfiguration = LxAppShellConfiguration()
    ) throws -> LxAppShell {
        if let currentShell = LxAppActiveHost.activeShell {
            currentShell.show()
            return currentShell
        }

        let info = try initializeRuntime()
        let controller = LxAppController()
        let config = resolvedShellConfiguration(
            from: configuration,
            capabilities: info.capabilities,
            homeAppId: info.homeAppId
        )

        let shell = LxAppShell(controller: controller, configuration: config)
        shell.show()
        return shell
    }
}
