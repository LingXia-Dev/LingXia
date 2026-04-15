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

    @MainActor
    public static func handleAppActivation() -> Bool {
        #if os(macOS)
        return LxAppMacAppUIRuntime.handleAppActivation()
        #else
        return false
        #endif
    }

    /// Default product entry point on Apple platforms.
    ///
    /// On macOS this loads bundled `app.json` plus `macos-ui.json` / `ui.json`
    /// and uses them to build the host shell. On iOS this keeps the legacy
    /// home-app startup behavior for now.
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
    public static func quickStart() throws -> LxAppShell {
        #if os(macOS)
        if let currentShell = LxAppActiveHost.activeShell {
            currentShell.show()
            return currentShell
        }

        let bundleConfig = try LxAppAppUIBundleLoader.loadFromMainBundle()
        _ = try initializeRuntime()

        let controller = LxAppController()
        let shellConfiguration = LxAppShellConfiguration(
            sidebar: .declarative(.init()),
            toolbar: .declarative(.default)
        )
        let shell = LxAppShell(
            controller: controller,
            configuration: shellConfiguration,
            startupBehavior: .managedByAppUI
        )
        let hostRuntime = try LxAppMacAppUIRuntime(
            bundleConfig: bundleConfig,
            controller: controller,
            shell: shell
        )
        shell.retainAppUIRuntime(hostRuntime)
        try hostRuntime.start()
        return shell
        #else
        return try quickStart(configuration: LxAppShellConfiguration())
        #endif
    }

    /// Legacy shell override path. Product UI should be configured in `lingxia.yaml`
    /// and started with `quickStart()`.
    @available(*, deprecated, message: "Configure product UI in lingxia.yaml and use Lingxia.quickStart(). Use initializeRuntime() + LxAppController + LxAppHostView for advanced embedding.")
    @MainActor
    @discardableResult
    public static func quickStart(
        configuration: LxAppShellConfiguration
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
