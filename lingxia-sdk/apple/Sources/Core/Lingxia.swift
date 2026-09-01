import Foundation

/// Top-level entry point for the LingXia SDK.
@MainActor
public enum Lingxia {
    /// Posted on the main thread after the user changes the effective display
    /// language. Host-owned native chrome can observe this and rebuild its
    /// localized labels.
    public nonisolated static let displayLanguageDidChangeNotification =
        Notification.Name("LingxiaDisplayLanguageDidChange")

    /// Effective display language selected by the runtime. A saved user
    /// choice takes precedence over the locale supplied during
    /// initialization.
    ///
    /// Before `quickStart()`/`initializeRuntime()` completes, the Rust side
    /// cannot yet resolve a saved override, so this falls back to the real
    /// system locale (`Locale.current`) instead of silently returning a
    /// fixed default in the wrong language.
    public nonisolated static var displayLanguage: String {
        guard LxAppRuntime.isInitializedUnsafe else {
            return Locale.current.identifier
        }
        return getDisplayLanguage().toString()
    }

    static func resolvedShellConfiguration(
        from configuration: LxAppShellConfiguration,
        capabilities: LxAppCapabilities,
        homeAppId: String?
    ) -> LxAppShellConfiguration {
        var config = configuration
        guard case .hidden = config.sidebar,
              capabilities.contains(.browser),
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
        CrashBacktrace.install()
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

    /// Run the product's command line and exit, when this process was invoked
    /// as one.
    ///
    /// Call this at the very top of `main`, before AppKit: the product's
    /// executable doubles as its command line, and a command must neither open
    /// a window nor initialize the runtime — initialization opens the app's
    /// databases and would collide with an instance already running.
    ///
    /// Returns normally when the process should carry on and become the app.
    public static func runProductCommandIfInvoked() {
        // Registration only publishes the linked host addon. It does not
        // initialize AppKit or LingXia, and lets Rust install host-owned CLI
        // commands before it classifies and parses this process's arguments.
        LxAppCore.registerNativeHostAddonOnce()
        let directories = LxAppDirectoryFactory.createDirectoryConfig()
        let code = productRunCliIfInvoked(directories.dataPath)
        if code >= 0 {
            exit(code)
        }
    }

    #if os(macOS)
    /// Default product entry point: loads bundled `app.json` plus
    /// `macos-ui.json` / `ui.json` and uses them to build the host shell.
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
        if let currentShell = LxAppActiveHost.activeShell {
            currentShell.show()
            return currentShell
        }

        let bundleConfig = try LxAppAppUIBundleLoader.loadFromMainBundle()
        _ = try initializeRuntime()
        LxAppHostTheme.install(bundleConfig.app.theme)

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
    }
    #else
    /// Default product entry point: puts the launch cover on screen first,
    /// then boots the runtime underneath it. The boot blocks the main
    /// thread, so running it before the cover's frame would keep the launch
    /// screen up through the entire initialization; deferred two frames, the
    /// cover is already what the user is looking at while the runtime boots.
    ///
    /// ```swift
    /// @main struct MyApp: App {
    ///     init() {
    ///         try! Lingxia.quickStart()
    ///     }
    /// }
    /// ```
    @MainActor
    public static func quickStart() throws {
        LingXiaSplashOverlay.attachIfNeeded()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.032) {
            do {
                _ = try quickStart(configuration: LxAppShellConfiguration())
            } catch {
                NSLog("Lingxia.quickStart failed: \(error)")
            }
        }
    }
    #endif

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
            homeAppId: info.lxAppId
        )

        let shell = LxAppShell(controller: controller, configuration: config)
        shell.show()
        return shell
    }
}
