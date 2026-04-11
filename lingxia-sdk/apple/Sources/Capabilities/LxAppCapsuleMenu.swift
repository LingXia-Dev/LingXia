import Foundation
import os.log

#if os(iOS)
import UIKit
#elseif os(macOS)
import AppKit
#endif

/// Capsule Menu Bottom Sheet
/// Shows LxApp info and action buttons when clicking the 3-dots capsule button
class LxAppCapsuleMenu {

    private static let log = OSLog(subsystem: "LingXia", category: "CapsuleMenu")
    private static let sheetContainerTag = 1001

    #if os(iOS)
    /// Show capsule menu for an LxApp
    static func show(appId: String) {
        let appInfo = getLxAppInfo(appId)
        if appInfo.app_name.toString().isEmpty {
            os_log("Failed to get LxApp info for: %{public}@", log: log, type: .error, appId)
            return
        }

        DispatchQueue.main.async {
            showIOSCapsuleMenu(appId: appId, appInfo: appInfo)
        }
    }
    @MainActor
    private static func showIOSCapsuleMenu(appId: String, appInfo: LxAppInfo) {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
              let rootViewController = window.rootViewController else {
            os_log("Could not find root view controller", log: log, type: .error)
            return
        }

        var topViewController = rootViewController
        while let presentedViewController = topViewController.presentedViewController {
            topViewController = presentedViewController
        }

        let menuView = createCapsuleMenuView(appId: appId, appInfo: appInfo)
        presentCapsuleMenu(menuView, on: topViewController)
    }

    @MainActor
    private static func createCapsuleMenuView(appId: String, appInfo: LxAppInfo) -> UIView {
        // Extract info strings
        let appName = appInfo.app_name.toString()
        let version = appInfo.version.toString()
        let releaseType = appInfo.release_type.toString()

        let backgroundView = UIView(frame: UIScreen.main.bounds)
        backgroundView.backgroundColor = UIColor.black.withAlphaComponent(0.5)
        backgroundView.alpha = 0

        let containerView = UIView()
        containerView.tag = sheetContainerTag
        containerView.backgroundColor = .white
        containerView.layer.cornerRadius = 16
        containerView.layer.maskedCorners = [.layerMinXMinYCorner, .layerMaxXMinYCorner]
        containerView.translatesAutoresizingMaskIntoConstraints = false

        // Header: App name and version on one line
        let headerView = createHeaderView(appName: appName, version: version, releaseType: releaseType)
        containerView.addSubview(headerView)

        // Separator
        let separator = createSeparator()
        containerView.addSubview(separator)

        // Button row
        let buttonsRow = createButtonsRow(appId: appId, backgroundView: backgroundView)
        containerView.addSubview(buttonsRow)

        // Full-screen dismiss area behind the bottom sheet.
        let dismissControl = UIControl()
        dismissControl.translatesAutoresizingMaskIntoConstraints = false
        backgroundView.addSubview(dismissControl)

        // Layout
        NSLayoutConstraint.activate([
            dismissControl.topAnchor.constraint(equalTo: backgroundView.topAnchor),
            dismissControl.leadingAnchor.constraint(equalTo: backgroundView.leadingAnchor),
            dismissControl.trailingAnchor.constraint(equalTo: backgroundView.trailingAnchor),
            dismissControl.bottomAnchor.constraint(equalTo: backgroundView.bottomAnchor),

            headerView.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 16),
            headerView.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: 20),
            headerView.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -20),

            separator.topAnchor.constraint(equalTo: headerView.bottomAnchor, constant: 12),
            separator.leadingAnchor.constraint(equalTo: containerView.leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: containerView.trailingAnchor),
            separator.heightAnchor.constraint(equalToConstant: 1),

            buttonsRow.topAnchor.constraint(equalTo: separator.bottomAnchor, constant: 12),
            buttonsRow.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: 20),
            buttonsRow.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -20),
            buttonsRow.bottomAnchor.constraint(equalTo: containerView.bottomAnchor, constant: -16)
        ])

        let dismissAction = ActionWrapper {
            dismissCapsuleMenu(backgroundView) {}
        }
        objc_setAssociatedObject(
            dismissControl,
            &AssociatedKeys.dismissActionKey,
            dismissAction,
            .OBJC_ASSOCIATION_RETAIN_NONATOMIC
        )
        dismissControl.addTarget(
            dismissAction,
            action: #selector(ActionWrapper.execute),
            for: .touchUpInside
        )

        backgroundView.addSubview(containerView)

        return backgroundView
    }

    @MainActor
    private static func createHeaderView(appName: String, version: String, releaseType: String) -> UIView {
        let headerView = UIView()
        headerView.translatesAutoresizingMaskIntoConstraints = false
        headerView.clipsToBounds = false

        // App name
        let nameLabel = UILabel()
        nameLabel.text = appName
        nameLabel.font = .boldSystemFont(ofSize: 16)
        nameLabel.textColor = .black
        nameLabel.translatesAutoresizingMaskIntoConstraints = false

        // Separator (·)
        let dotLabel = UILabel()
        dotLabel.text = " · "
        dotLabel.font = .systemFont(ofSize: 16)
        dotLabel.textColor = UIColor(red: 0.8, green: 0.8, blue: 0.8, alpha: 1)
        dotLabel.translatesAutoresizingMaskIntoConstraints = false

        // Version
        let versionLabel = UILabel()
        versionLabel.text = version
        versionLabel.font = .systemFont(ofSize: 14)
        versionLabel.textColor = UIColor(red: 0.6, green: 0.6, blue: 0.6, alpha: 1)
        versionLabel.translatesAutoresizingMaskIntoConstraints = false

        headerView.addSubview(nameLabel)
        headerView.addSubview(dotLabel)
        headerView.addSubview(versionLabel)

        var constraints: [NSLayoutConstraint] = [
            nameLabel.leadingAnchor.constraint(equalTo: headerView.leadingAnchor),
            nameLabel.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),

            dotLabel.leadingAnchor.constraint(equalTo: nameLabel.trailingAnchor),
            dotLabel.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),

            versionLabel.leadingAnchor.constraint(equalTo: dotLabel.trailingAnchor),
            versionLabel.centerYAnchor.constraint(equalTo: headerView.centerYAnchor),
            versionLabel.trailingAnchor.constraint(lessThanOrEqualTo: headerView.trailingAnchor),

            headerView.heightAnchor.constraint(equalToConstant: 24)
        ]

        if let badge = releaseBadge(for: releaseType) {
            let badgeLabel = UILabel()
            badgeLabel.text = badge.text
            badgeLabel.font = .systemFont(ofSize: 10, weight: .semibold)
            badgeLabel.textColor = badge.textColor
            badgeLabel.backgroundColor = badge.backgroundColor
            badgeLabel.textAlignment = .center
            badgeLabel.layer.cornerRadius = 8
            badgeLabel.layer.masksToBounds = true
            badgeLabel.translatesAutoresizingMaskIntoConstraints = false

            headerView.addSubview(badgeLabel)

            constraints.append(contentsOf: [
                badgeLabel.leadingAnchor.constraint(equalTo: versionLabel.trailingAnchor, constant: 4),
                badgeLabel.bottomAnchor.constraint(equalTo: versionLabel.topAnchor, constant: 3),
                badgeLabel.heightAnchor.constraint(equalToConstant: 16),
                badgeLabel.trailingAnchor.constraint(lessThanOrEqualTo: headerView.trailingAnchor),
                badgeLabel.widthAnchor.constraint(greaterThanOrEqualToConstant: 34)
            ])
        }

        NSLayoutConstraint.activate(constraints)

        return headerView
    }

    private static func releaseBadge(for releaseType: String) -> (text: String, textColor: UIColor, backgroundColor: UIColor)? {
        switch releaseType.lowercased() {
        case "developer":
            return ("DEV", UIColor(red: 0.11, green: 0.31, blue: 0.85, alpha: 1.0), UIColor(red: 0.86, green: 0.92, blue: 0.99, alpha: 1.0))
        case "preview":
            return ("PRE", UIColor(red: 0.71, green: 0.33, blue: 0.03, alpha: 1.0), UIColor(red: 1.0, green: 0.93, blue: 0.84, alpha: 1.0))
        default:
            return nil
        }
    }

    @MainActor
    private static func createButtonsRow(appId: String, backgroundView: UIView) -> UIView {
        let rowView = UIView()
        rowView.translatesAutoresizingMaskIntoConstraints = false

        let actions: [(icon: String, title: String, action: String, isDestructive: Bool)] = [
            ("icon_clean_cache", L10n.string("lx_capsule_clean_cache"), "clean_cache_restart", false),
            ("icon_restart", L10n.string("lx_capsule_restart"), "restart", false),
            ("icon_uninstall", L10n.string("lx_capsule_uninstall"), "uninstall", false)
        ]

        let stackView = UIStackView()
        stackView.axis = .horizontal
        stackView.distribution = .fillEqually
        stackView.spacing = 16
        stackView.translatesAutoresizingMaskIntoConstraints = false

        for action in actions {
            let button = createActionButton(
                iconName: action.icon,
                title: action.title,
                isDestructive: action.isDestructive
            ) {
                dismissCapsuleMenu(backgroundView) {
                    _ = onLxappEvent(appId, LxAppEvent.capsuleClick, action.action)
                }
            }
            stackView.addArrangedSubview(button)
        }

        rowView.addSubview(stackView)

        NSLayoutConstraint.activate([
            stackView.topAnchor.constraint(equalTo: rowView.topAnchor),
            stackView.leadingAnchor.constraint(equalTo: rowView.leadingAnchor),
            stackView.trailingAnchor.constraint(equalTo: rowView.trailingAnchor),
            stackView.bottomAnchor.constraint(equalTo: rowView.bottomAnchor),
            stackView.heightAnchor.constraint(equalToConstant: 72)
        ])

        return rowView
    }

    @MainActor
    private static func createActionButton(iconName: String, title: String, isDestructive: Bool, action: @escaping () -> Void) -> UIView {
        let containerView = UIControl()
        containerView.translatesAutoresizingMaskIntoConstraints = false

        // Icon
        let iconView = UIImageView()
        if let icon = LxIcon.image(named: iconName) {
            iconView.image = icon.withRenderingMode(.alwaysTemplate)
        }
        iconView.contentMode = .scaleAspectFit
        iconView.tintColor = isDestructive ? UIColor(red: 1.0, green: 0.23, blue: 0.19, alpha: 1) : UIColor(red: 0.2, green: 0.2, blue: 0.2, alpha: 1)
        iconView.translatesAutoresizingMaskIntoConstraints = false

        // Title
        let titleLabel = UILabel()
        titleLabel.text = title
        titleLabel.font = .systemFont(ofSize: 13)
        titleLabel.textColor = isDestructive ? UIColor(red: 1.0, green: 0.23, blue: 0.19, alpha: 1) : UIColor(red: 0.2, green: 0.2, blue: 0.2, alpha: 1)
        titleLabel.textAlignment = .center
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        containerView.addSubview(iconView)
        containerView.addSubview(titleLabel)

        NSLayoutConstraint.activate([
            iconView.topAnchor.constraint(equalTo: containerView.topAnchor, constant: 12),
            iconView.centerXAnchor.constraint(equalTo: containerView.centerXAnchor),
            iconView.widthAnchor.constraint(equalToConstant: 24),
            iconView.heightAnchor.constraint(equalToConstant: 24),

            titleLabel.topAnchor.constraint(equalTo: iconView.bottomAnchor, constant: 6),
            titleLabel.leadingAnchor.constraint(equalTo: containerView.leadingAnchor, constant: 4),
            titleLabel.trailingAnchor.constraint(equalTo: containerView.trailingAnchor, constant: -4),
            titleLabel.bottomAnchor.constraint(equalTo: containerView.bottomAnchor, constant: -12)
        ])

        // Store action in a wrapper
        let actionWrapper = ActionWrapper(action: action)
        objc_setAssociatedObject(
            containerView,
            &AssociatedKeys.actionKey,
            actionWrapper,
            .OBJC_ASSOCIATION_RETAIN_NONATOMIC
        )
        containerView.addTarget(
            actionWrapper,
            action: #selector(ActionWrapper.execute),
            for: .touchUpInside
        )

        return containerView
    }

    @MainActor
    private static func createSeparator() -> UIView {
        let separator = UIView()
        separator.backgroundColor = UIColor(red: 0.93, green: 0.93, blue: 0.93, alpha: 1)
        separator.translatesAutoresizingMaskIntoConstraints = false
        return separator
    }

    @MainActor
    private static func presentCapsuleMenu(_ menuView: UIView, on viewController: UIViewController) {
        guard let containerView = menuView.subviews.first(where: { $0.tag == sheetContainerTag }) else { return }

        viewController.view.addSubview(menuView)

        containerView.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            menuView.topAnchor.constraint(equalTo: viewController.view.topAnchor),
            menuView.leadingAnchor.constraint(equalTo: viewController.view.leadingAnchor),
            menuView.trailingAnchor.constraint(equalTo: viewController.view.trailingAnchor),
            menuView.bottomAnchor.constraint(equalTo: viewController.view.bottomAnchor),

            containerView.leadingAnchor.constraint(equalTo: menuView.leadingAnchor),
            containerView.trailingAnchor.constraint(equalTo: menuView.trailingAnchor),
            containerView.bottomAnchor.constraint(equalTo: menuView.bottomAnchor)
        ])

        menuView.layoutIfNeeded()
        let offset = max(containerView.bounds.height, 1)

        // Initial position (offscreen)
        containerView.transform = CGAffineTransform(translationX: 0, y: offset)

        UIView.animate(withDuration: 0.3, delay: 0, options: .curveEaseOut) {
            menuView.alpha = 1
            containerView.transform = .identity
        }
    }

    @MainActor
    private static func dismissCapsuleMenu(_ menuView: UIView, completion: @escaping () -> Void) {
        guard let containerView = menuView.subviews.first(where: { $0.tag == sheetContainerTag }) else {
            completion()
            return
        }

        menuView.layoutIfNeeded()
        let offset = max(containerView.bounds.height, 1)

        UIView.animate(withDuration: 0.25, delay: 0, options: .curveEaseIn, animations: {
            menuView.alpha = 0
            containerView.transform = CGAffineTransform(translationX: 0, y: offset)
        }) { _ in
            menuView.removeFromSuperview()
            completion()
        }
    }
    #elseif os(macOS)
    /// Show capsule menu for an LxApp on macOS.
    /// Pops up a native NSMenu near the mouse cursor with the same actions as iOS.
    static func show(appId: String) {
        let appInfo = getLxAppInfo(appId)
        if appInfo.app_name.toString().isEmpty {
            os_log("Failed to get LxApp info for: %{public}@", log: log, type: .error, appId)
            return
        }

        DispatchQueue.main.async {
            showMacOSCapsuleMenu(appId: appId, appInfo: appInfo)
        }
    }

    @MainActor
    private static func showMacOSCapsuleMenu(appId: String, appInfo: LxAppInfo) {
        let menu = buildMacOSCapsuleMenu(appId: appId, appInfo: appInfo)

        // Anchor the menu near the mouse location in the key window.
        guard let window = NSApp.keyWindow ?? NSApp.mainWindow,
              let contentView = window.contentView else {
            os_log("Could not find active window/content view for capsule menu", log: log, type: .error)
            return
        }

        let mouseInWindow = window.mouseLocationOutsideOfEventStream
        let mouseInView = contentView.convert(mouseInWindow, from: nil)
        menu.popUp(positioning: nil, at: mouseInView, in: contentView)
    }

    @MainActor
    private static func buildMacOSCapsuleMenu(appId: String, appInfo: LxAppInfo) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false

        // Header: app name + version + release badge.
        let appName = appInfo.app_name.toString()
        let version = appInfo.version.toString()
        let releaseType = appInfo.release_type.toString()

        var headerTitle = "\(appName) · v\(version)"
        switch releaseType.lowercased() {
        case "developer": headerTitle += "  [DEV]"
        case "preview": headerTitle += "  [PRE]"
        default: break
        }

        let headerItem = NSMenuItem(title: headerTitle, action: nil, keyEquivalent: "")
        headerItem.isEnabled = false
        menu.addItem(headerItem)
        menu.addItem(NSMenuItem.separator())

        let target = MacCapsuleMenuTarget(appId: appId)
        // Retain the target on the menu so it outlives the popup.
        objc_setAssociatedObject(
            menu,
            &AssociatedKeys.macTargetKey,
            target,
            .OBJC_ASSOCIATION_RETAIN_NONATOMIC
        )

        // Clean Cache & Restart
        let cleanItem = NSMenuItem(
            title: L10n.string("lx_capsule_clean_cache"),
            action: #selector(MacCapsuleMenuTarget.cleanCacheClicked),
            keyEquivalent: ""
        )
        cleanItem.image = NSImage(systemSymbolName: "trash", accessibilityDescription: nil)
        cleanItem.target = target
        menu.addItem(cleanItem)

        // Restart
        let restartItem = NSMenuItem(
            title: L10n.string("lx_capsule_restart"),
            action: #selector(MacCapsuleMenuTarget.restartClicked),
            keyEquivalent: ""
        )
        restartItem.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: nil)
        restartItem.target = target
        menu.addItem(restartItem)

        // Uninstall (only for non-home lxapps)
        if !LxAppCore.isHomeLxApp(appId) {
            menu.addItem(NSMenuItem.separator())
            let uninstallItem = NSMenuItem(
                title: L10n.string("lx_capsule_uninstall"),
                action: #selector(MacCapsuleMenuTarget.uninstallClicked),
                keyEquivalent: ""
            )
            uninstallItem.image = NSImage(systemSymbolName: "xmark.bin", accessibilityDescription: nil)
            uninstallItem.target = target
            menu.addItem(uninstallItem)
        }

        return menu
    }
    #else
    public static func show(appId: String) {
        os_log("Capsule menu is not implemented on this platform for %{public}@", log: log, type: .info, appId)
    }
    #endif
}

#if os(macOS)
/// Target object for macOS capsule menu items.
/// Held by the menu via associated object to stay alive during popup.
private final class MacCapsuleMenuTarget: NSObject {
    let appId: String

    init(appId: String) {
        self.appId = appId
    }

    @objc func cleanCacheClicked() {
        _ = onLxappEvent(appId, LxAppEvent.capsuleClick, "clean_cache_restart")
    }

    @objc func restartClicked() {
        _ = onLxappEvent(appId, LxAppEvent.capsuleClick, "restart")
    }

    @objc func uninstallClicked() {
        _ = onLxappEvent(appId, LxAppEvent.capsuleClick, "uninstall")
    }
}
#endif

// MARK: - Helper Classes

#if os(iOS)
private class ActionWrapper {
    let action: () -> Void

    init(action: @escaping () -> Void) {
        self.action = action
    }

    @objc func execute() {
        action()
    }
}
#endif

@MainActor
private struct AssociatedKeys {
    #if os(iOS)
    static var actionKey: UInt8 = 0
    static var dismissActionKey: UInt8 = 0
    #endif
    #if os(macOS)
    static var macTargetKey: UInt8 = 0
    #endif
}
