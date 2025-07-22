#if os(iOS)
import UIKit
import Foundation

/// iOS-specific view controller support utilities
@MainActor
public class iOSViewControllerSupport {

    /// Configures edge-to-edge display for the view controller
    public static func configureEdgeToEdgeDisplay(_ viewController: UIViewController) {
        viewController.edgesForExtendedLayout = [.top, .bottom, .left, .right]
        viewController.extendedLayoutIncludesOpaqueBars = true
        if #available(iOS 11.0, *) {
            // Use modern content inset adjustment behavior
        } else {
            viewController.automaticallyAdjustsScrollViewInsets = false
        }

        if #available(iOS 11.0, *) {
            viewController.additionalSafeAreaInsets = UIEdgeInsets.zero
        }
    }

    /// Sets transparent background for the view controller
    public static func setTransparentBackground(_ viewController: UIViewController) {
        viewController.view.backgroundColor = UIColor.clear

        if let navigationController = viewController.navigationController {
            navigationController.view.backgroundColor = UIColor.clear
        }

        if let tabBarController = viewController.tabBarController {
            tabBarController.view.backgroundColor = UIColor.clear
        }
    }

    /// Creates and configures a capsule button for iOS
    public static func createCapsuleButton(frame: CGRect, target: Any?, moreAction: Selector, closeAction: Selector) -> UIView {
        let capsuleContainer = UIView(frame: frame)
        capsuleContainer.backgroundColor = UIColor.white.withAlphaComponent(0.9)
        capsuleContainer.layer.cornerRadius = frame.height / 2
        capsuleContainer.layer.borderWidth = 0.5
        capsuleContainer.layer.borderColor = UIColor.lightGray.cgColor

        let buttonWidth = frame.width / 2
        let buttonHeight = frame.height

        // More button
        let moreButton = UIButton(frame: CGRect(x: 0, y: 0, width: buttonWidth, height: buttonHeight))
        moreButton.setImage(createMoreDotsImage(), for: .normal)
        moreButton.addTarget(target, action: moreAction, for: .touchUpInside)

        // Close button
        let closeButton = UIButton(frame: CGRect(x: buttonWidth, y: 0, width: buttonWidth, height: buttonHeight))
        closeButton.setImage(createCloseButtonImage(), for: .normal)
        closeButton.addTarget(target, action: closeAction, for: .touchUpInside)

        // Separator
        let separator = UIView(frame: CGRect(x: buttonWidth - 0.25, y: 6, width: 0.5, height: buttonHeight - 12))
        separator.backgroundColor = UIColor.lightGray.withAlphaComponent(0.3)

        capsuleContainer.addSubview(moreButton)
        capsuleContainer.addSubview(separator)
        capsuleContainer.addSubview(closeButton)

        return capsuleContainer
    }

    /// Updates layout margins for the view controller
    public static func updateLayoutMargins(_ viewController: UIViewController) {
        if #available(iOS 11.0, *) {
            let safeAreaInsets = viewController.view.safeAreaInsets
            viewController.additionalSafeAreaInsets = UIEdgeInsets(
                top: -safeAreaInsets.top,
                left: -safeAreaInsets.left,
                bottom: -safeAreaInsets.bottom,
                right: -safeAreaInsets.right
            )
        }
    }

    /// Calculates the appropriate top anchor for content positioning
    public static func calculateTopAnchor(for viewController: UIViewController) -> (NSLayoutYAxisAnchor, CGFloat) {
        if #available(iOS 11.0, *) {
            return (viewController.view.safeAreaLayoutGuide.topAnchor, 0)
        } else {
            return (viewController.view.topAnchor, 20) // Status bar height
        }
    }

    /// Calculates the appropriate bottom anchor for content positioning
    public static func calculateBottomAnchor(for viewController: UIViewController, isTransparent: Bool) -> NSLayoutYAxisAnchor {
        if #available(iOS 11.0, *) {
            return isTransparent ? viewController.view.bottomAnchor : viewController.view.safeAreaLayoutGuide.bottomAnchor
        } else {
            return viewController.view.bottomAnchor
        }
    }

    // MARK: - Image Creation

    private static func createMoreDotsImage() -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)

        guard let context = UIGraphicsGetCurrentContext() else {
            UIGraphicsEndImageContext()
            return nil
        }

        context.setFillColor(UIColor.darkGray.cgColor)

        let centerY = size.height / 2
        let centerX = size.width / 2
        let dotRadius: CGFloat = 2
        let spacing: CGFloat = 6

        // Left dot
        let leftDotRect = CGRect(x: centerX - spacing - dotRadius, y: centerY - dotRadius, width: dotRadius * 2, height: dotRadius * 2)
        context.fillEllipse(in: leftDotRect)

        // Center dot
        let centerDotRect = CGRect(x: centerX - dotRadius, y: centerY - dotRadius, width: dotRadius * 2, height: dotRadius * 2)
        context.fillEllipse(in: centerDotRect)

        // Right dot
        let rightDotRect = CGRect(x: centerX + spacing - dotRadius, y: centerY - dotRadius, width: dotRadius * 2, height: dotRadius * 2)
        context.fillEllipse(in: rightDotRect)

        let image = UIGraphicsGetImageFromCurrentImageContext()
        UIGraphicsEndImageContext()

        return image
    }

    private static func createCloseButtonImage() -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)

        guard let context = UIGraphicsGetCurrentContext() else {
            UIGraphicsEndImageContext()
            return nil
        }

        context.setStrokeColor(UIColor.darkGray.cgColor)
        context.setLineWidth(2.0)
        context.setLineCap(.round)

        let centerX = size.width / 2
        let centerY = size.height / 2
        let radius: CGFloat = 8

        // Draw circle
        let circleRect = CGRect(x: centerX - radius, y: centerY - radius, width: radius * 2, height: radius * 2)
        context.strokeEllipse(in: circleRect)

        // Draw inner dot
        context.setFillColor(UIColor.darkGray.cgColor)
        let dotRadius: CGFloat = 2
        let dotRect = CGRect(x: centerX - dotRadius, y: centerY - dotRadius, width: dotRadius * 2, height: dotRadius * 2)
        context.fillEllipse(in: dotRect)

        let image = UIGraphicsGetImageFromCurrentImageContext()
        UIGraphicsEndImageContext()

        return image
    }
}

#endif
