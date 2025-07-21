#if os(iOS)
import UIKit
import Foundation
import os.log

/// iOS Page Navigation management for LxApp
@MainActor
public class iOSPageNavigation {

    /// Switches to a specific tab
    public static func switchToTab(targetPath: String, in viewController: iOSLxAppViewController) {
        guard let tabBar = viewController.tabBar else { return }

        let tabIndex = tabBar.findTabIndexByPath(targetPath)
        guard tabIndex >= 0 else { return }

        tabBar.setSelectedIndex(tabIndex, notifyListener: false)
        switchPage(targetPath: targetPath, in: viewController)
    }

    /// Switches to a specific page
    public static func switchPage(targetPath: String, in viewController: iOSLxAppViewController) {
        guard !viewController.isDestroyed else { return }

        let isReplace = targetPath.contains("?replace=true")
        let cleanPath = targetPath.replacingOccurrences(of: "?replace=true", with: "")

        if isReplace {
            navigateToPage(targetPath: cleanPath, isReplace: true, isBackNavigation: false, in: viewController)
        } else {
            navigateToPage(targetPath: cleanPath, isReplace: false, isBackNavigation: false, in: viewController)
        }
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
        let pageConfig = getPageConfig(appId: appId, path: path, in: viewController)

        // Determine NavigationBar visibility
        let shouldShowNavigationBar: Bool
        if let config = pageConfig {
            shouldShowNavigationBar = !config.hidden
        } else {
            // No configuration available - use default behavior (show NavigationBar)
            shouldShowNavigationBar = true
        }

        if shouldShowNavigationBar {
            viewController.ensureNavigationBarExists()

            guard let navigationBar = viewController.navigationBar else {
                return
            }

            let title = pageConfig?.navigationBarTitleText ?? ""
            let bgColor = pageConfig?.navigationBarBackgroundColor ?? NavigationBarConfig.DEFAULT_BACKGROUND_COLOR
            let textColor = getTextColor(from: pageConfig?.navigationBarTextStyle)
            let showBackButton = shouldShowBackButton(for: path, appId: appId)

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

    /// Gets page configuration
    public static func getPageConfig(appId: String, path: String, in viewController: iOSLxAppViewController) -> NavigationBarConfig? {
        guard let pageConfigJson = lingxia.getPageConfig(appId, path)?.toString() else {
            return nil
        }
        return NavigationBarConfig.fromJson(pageConfigJson)
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

    private static func shouldShowBackButton(for path: String, appId: String) -> Bool {
        // Don't show back button for home app or root pages
        if LxAppCore.isHomeLxApp(appId) {
            return false
        }

        return true
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
    public func getPageConfig(appId: String, path: String) -> NavigationBarConfig? {
        return iOSPageNavigation.getPageConfig(appId: appId, path: path, in: self)
    }

    /// Handles back button click (public interface)
    public func handleBackButtonClick() {
        iOSPageNavigation.handleBackButtonClick(in: self)
    }
}

#endif
