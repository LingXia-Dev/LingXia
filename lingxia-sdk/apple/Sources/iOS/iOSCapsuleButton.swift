#if os(iOS)
import UIKit
import Foundation

/// iOS Capsule Button management for LxApp
@MainActor
public class iOSCapsuleButton {

    private static let CAPSULE_BUTTON_TAG = 9999

    /// Adds capsule button to the view controller
    public static func addCapsuleButton(to viewController: UIViewController, appId: String) {
        guard viewController.view.viewWithTag(CAPSULE_BUTTON_TAG) == nil else { return }

        let isHomeLxApp = appId == LxAppCore.getHomeLxAppId()
        guard !isHomeLxApp else { return }

        let capsuleWidth: CGFloat = 87
        let capsuleHeight: CGFloat = 32
        let rightMargin: CGFloat = 16
        let topMargin: CGFloat = 48 + 8

        let capsuleContainer = UIView()
        capsuleContainer.tag = CAPSULE_BUTTON_TAG
        capsuleContainer.backgroundColor = UIColor.white.withAlphaComponent(0.9)
        capsuleContainer.layer.cornerRadius = capsuleHeight / 2
        capsuleContainer.layer.borderWidth = 0.5
        capsuleContainer.layer.borderColor = UIColor.lightGray.cgColor
        capsuleContainer.translatesAutoresizingMaskIntoConstraints = false

        let buttonWidth = capsuleWidth / 2
        let buttonHeight = capsuleHeight

        // More button
        let moreButton = UIButton(frame: CGRect(x: 0, y: 0, width: buttonWidth, height: buttonHeight))
        moreButton.setImage(createMoreDotsImage(), for: .normal)
        moreButton.addTarget(viewController, action: #selector(iOSLxAppViewController.moreButtonTapped), for: .touchUpInside)

        // Close button
        let closeButton = UIButton(frame: CGRect(x: buttonWidth, y: 0, width: buttonWidth, height: buttonHeight))
        closeButton.setImage(createCloseButtonImage(), for: .normal)
        closeButton.addTarget(viewController, action: #selector(iOSLxAppViewController.closeButtonTapped), for: .touchUpInside)

        // Separator
        let separator = UIView(frame: CGRect(x: buttonWidth - 0.25, y: 6, width: 0.5, height: buttonHeight - 12))
        separator.backgroundColor = UIColor.lightGray.withAlphaComponent(0.3)

        capsuleContainer.addSubview(moreButton)
        capsuleContainer.addSubview(separator)
        capsuleContainer.addSubview(closeButton)

        viewController.view.addSubview(capsuleContainer)

        // Set constraints
        NSLayoutConstraint.activate([
            capsuleContainer.widthAnchor.constraint(equalToConstant: capsuleWidth),
            capsuleContainer.heightAnchor.constraint(equalToConstant: capsuleHeight),
            capsuleContainer.trailingAnchor.constraint(equalTo: viewController.view.trailingAnchor, constant: -rightMargin),
            capsuleContainer.topAnchor.constraint(equalTo: viewController.view.topAnchor, constant: topMargin)
        ])

        // Bring to front
        viewController.view.bringSubviewToFront(capsuleContainer)
    }

    /// Removes capsule button from the view controller
    public static func removeCapsuleButton(from viewController: UIViewController) {
        viewController.view.viewWithTag(CAPSULE_BUTTON_TAG)?.removeFromSuperview()
    }

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
        let centerDotRadius = size.height / 7
        let sideDotRadius = size.height / 10
        let spacing = centerDotRadius * 2.8

        // Left dot
        let leftDotRect = CGRect(
            x: centerX - spacing - sideDotRadius,
            y: centerY - sideDotRadius,
            width: sideDotRadius * 2,
            height: sideDotRadius * 2
        )
        context.fillEllipse(in: leftDotRect)

        // Right dot
        let rightDotRect = CGRect(
            x: centerX + spacing - sideDotRadius,
            y: centerY - sideDotRadius,
            width: sideDotRadius * 2,
            height: sideDotRadius * 2
        )
        context.fillEllipse(in: rightDotRect)

        // Center dot
        let centerDotRect = CGRect(
            x: centerX - centerDotRadius,
            y: centerY - centerDotRadius,
            width: centerDotRadius * 2,
            height: centerDotRadius * 2
        )
        context.fillEllipse(in: centerDotRect)

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
        context.setLineWidth(2.2)
        context.setLineCap(.round)

        let centerX = size.width / 2
        let centerY = size.height / 2
        let outerRadius = size.width * 0.35
        let innerRadius: CGFloat = 2.5

        // Draw outer circle
        let outerCircle = CGRect(
            x: centerX - outerRadius,
            y: centerY - outerRadius,
            width: outerRadius * 2,
            height: outerRadius * 2
        )
        context.strokeEllipse(in: outerCircle)

        // Draw inner dot
        context.setFillColor(UIColor.darkGray.cgColor)
        let innerCircle = CGRect(
            x: centerX - innerRadius,
            y: centerY - innerRadius,
            width: innerRadius * 2,
            height: innerRadius * 2
        )
        context.fillEllipse(in: innerCircle)

        let image = UIGraphicsGetImageFromCurrentImageContext()
        UIGraphicsEndImageContext()

        return image
    }
}

#endif
