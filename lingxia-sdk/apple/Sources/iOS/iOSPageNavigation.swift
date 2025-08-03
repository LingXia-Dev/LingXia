#if os(iOS)
import UIKit
import Foundation
import os.log

/// iOS Page Navigation management for LxApp
@MainActor
public class iOSPageNavigation {

    /// Switches to a specific tab - simplified to match Android/Harmony approach
    public static func switchToTab(targetPath: String, in viewController: iOSLxAppViewController) {
        guard let tabBar = viewController.tabBar else { return }

        let tabIndex = tabBar.findTabIndexByPath(targetPath)
        guard tabIndex >= 0 else { return }

        // Skip if already on this tab (like Harmony)
        if viewController.currentWebView?.currentPath == targetPath { return }

        // Update TabBar UI first (like Android)
        tabBar.setSelectedIndex(tabIndex, notifyListener: false)

        let appId = viewController.appId

        // Optimized NavigationBar handling for TabBar switches
        let pageConfig = PageNavigationCore.getNavigationBarConfig(appId: appId, path: targetPath)
        let shouldShowNavigationBar = PageNavigationCore.shouldShowNavigationBar(pageConfig: pageConfig)
        let currentHasNavBar = viewController.navigationBar != nil

        // Only update NavigationBar if state actually changes (key optimization)
        if currentHasNavBar != shouldShowNavigationBar {
            if shouldShowNavigationBar {
                updateNavigationBar(
                    appId: appId,
                    path: targetPath,
                    isBackNavigation: false,
                    disableAnimation: true,
                    in: viewController
                )
            } else {
                // Remove NavigationBar if not needed, but use optimized removal
                viewController.removeNavigationBarForTabSwitch()
            }
        } else if shouldShowNavigationBar {
            // NavigationBar exists and should stay - just update content without animation
            updateNavigationBar(
                appId: appId,
                path: targetPath,
                isBackNavigation: false,
                disableAnimation: true,
                in: viewController
            )
        }

        // Setup WebView
        viewController.setupWebViewWithoutNavBarUpdate(appId: appId, path: targetPath)
        LxAppCore.setLastActivePath(targetPath, for: appId)
        viewController.applyTransparencyEffectsAfterTabSwitch()
    }

    /// Switches to a specific page
    public static func switchPage(targetPath: String, in viewController: iOSLxAppViewController) {
        guard !viewController.isDestroyed else { return }

        let params = PageNavigationCore.parseNavigationParams(from: targetPath)

        navigateToPage(
            targetPath: params.cleanPath,
            isReplace: params.isReplace,
            isBackNavigation: false,
            in: viewController
        )
    }

    /// Navigates to a specific page
    public static func navigateToPage(
        targetPath: String,
        isReplace: Bool = false,
        isBackNavigation: Bool = false,
        in viewController: iOSLxAppViewController
    ) {
        guard !viewController.isDestroyed else { return }

        let appId = viewController.appId

        // Update navigation bar
        updateNavigationBar(
            appId: appId,
            path: targetPath,
            isBackNavigation: isBackNavigation,
            in: viewController
        )

        // Setup WebView for the new page
        viewController.setupWebViewIfReady(appId: appId, path: targetPath)

        // Update tab bar selection if needed
        if let tabBar = viewController.tabBar {
            tabBar.syncSelectedTabWithCurrentPath(targetPath)
        }
    }

    /// Updates the navigation bar for a page
    public static func updateNavigationBar(
        appId: String,
        path: String,
        isBackNavigation: Bool,
        disableAnimation: Bool = false,
        in viewController: iOSLxAppViewController
    ) {
        let pageConfig = PageNavigationCore.getNavigationBarConfig(appId: appId, path: path)

        // Determine NavigationBar visibility using PageNavigationCore
        let shouldShowNavigationBar = PageNavigationCore.shouldShowNavigationBar(pageConfig: pageConfig)

        if shouldShowNavigationBar {
            viewController.ensureNavigationBarExists()

            guard let navigationBar = viewController.navigationBar else {
                return
            }

            let title = pageConfig?.title_text.toString() ?? ""
            let bgColor = PlatformColor(argb: pageConfig?.background_color ?? 0xFFFFFFFF)
            let textColor = getTextColor(from: pageConfig?.text_style.toString())

            // Get TabBar config to check if this is a tab root page
            // First try to get from the TabBar instance, then fallback to direct config lookup
            let tabBarConfig: TabBarConfig?
            if let existingConfig = viewController.tabBar?.config {
                tabBarConfig = existingConfig
            } else {
                // Fallback: get TabBar config directly from the app configuration
                tabBarConfig = getTabBarConfig(appId)
            }

            let showBackButton = shouldShowBackButton(for: path, appId: appId, tabBarConfig: tabBarConfig)

            navigationBar.updateStateAndAnimate(
                title: title,
                bgColor: bgColor,
                textColor: textColor,
                showBackButton: showBackButton,
                isBackNavigation: isBackNavigation,
                disableAnimation: disableAnimation,
                onBackClickListener: {
                    handleBackButtonClick(in: viewController)
                }
            )
        } else {
            // NavigationBar should be hidden - remove it completely
            viewController.removeNavigationBar()
        }
    }



    /// Handles back button click
    public static func handleBackButtonClick(in viewController: iOSLxAppViewController) {
        let result = onBackPressed(viewController.appId)
        if result {
            viewController.performLxAppClose()
        }
    }

    private static func getTextColor(from textStyle: String?) -> UIColor {
        switch textStyle?.lowercased() {
        case "white":
            return UIColor.white
        case "black", nil:
            return UIColor.black
        default:
            return UIColor.black
        }
    }

    private static func shouldShowBackButton(for path: String, appId: String, tabBarConfig: TabBarConfig? = nil) -> Bool {
        return PageNavigationCore.shouldShowBackButton(for: path, appId: appId, tabBarConfig: tabBarConfig)
    }
}

extension iOSLxAppViewController {

    /// Switches to tab (public interface)
    public func switchToTab(targetPath: String) {
        iOSPageNavigation.switchToTab(targetPath: targetPath, in: self)
    }

    /// Switches page (public interface)
    public func switchPage(targetPath: String) {
        iOSPageNavigation.switchPage(targetPath: targetPath, in: self)
    }

    /// Navigates to page (public interface)
    public func navigateToPage(targetPath: String, isReplace: Bool = false, isBackNavigation: Bool = false) {
        iOSPageNavigation.navigateToPage(
            targetPath: targetPath,
            isReplace: isReplace,
            isBackNavigation: isBackNavigation,
            in: self
        )
    }

    /// Updates navigation bar (public interface)
    public func updateNavigationBar(appId: String, path: String, isBackNavigation: Bool, disableAnimation: Bool = false) {
        iOSPageNavigation.updateNavigationBar(
            appId: appId,
            path: path,
            isBackNavigation: isBackNavigation,
            disableAnimation: disableAnimation,
            in: self
        )
    }

    /// Gets page config (public interface)
    public func getNavigationBarConfig(appId: String, path: String) -> NavigationBarConfig? {
        return PageNavigationCore.getNavigationBarConfig(appId: appId, path: path)
    }

    /// Handles back button click (public interface)
    public func handleBackButtonClick() {
        iOSPageNavigation.handleBackButtonClick(in: self)
    }
}

#endif
