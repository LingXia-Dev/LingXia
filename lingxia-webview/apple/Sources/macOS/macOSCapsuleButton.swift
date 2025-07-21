#if os(macOS)
import Cocoa
import Foundation

/// macOS Capsule Button management for LxApp
@MainActor
public class macOSCapsuleButton {

    // MARK: - Constants
    private static let CAPSULE_BUTTON_WIDTH: CGFloat = 87
    private static let CAPSULE_BUTTON_HEIGHT: CGFloat = 28
    private static let CAPSULE_TOP_MARGIN: CGFloat = 2

    /// Creates and adds capsule buttons to the title bar view
    public static func addCapsuleButtons(
        to titleBarView: NSView,
        windowWidth: CGFloat,
        target: AnyObject,
        moreAction: Selector,
        minimizeAction: Selector,
        closeAction: Selector
    ) {
        let buttonWidth = CAPSULE_BUTTON_WIDTH / 3
        let buttonHeight = CAPSULE_BUTTON_HEIGHT
        let buttonY = CAPSULE_TOP_MARGIN
        let rightMargin: CGFloat = 7

        // Create buttons with proper images
        let moreButton = createCapsuleButton(
            image: createThreeDotsImage(),
            target: target,
            action: moreAction
        )
        let minimizeButton = createCapsuleButton(
            image: createMinimizeButtonImage(),
            target: target,
            action: minimizeAction
        )
        let closeButton = createCapsuleButton(
            image: createCloseButtonImage(),
            target: target,
            action: closeAction
        )

        // Position buttons
        let startX = windowWidth - CAPSULE_BUTTON_WIDTH - rightMargin
        moreButton.frame = NSRect(x: startX, y: buttonY, width: buttonWidth, height: buttonHeight)
        minimizeButton.frame = NSRect(x: startX + buttonWidth, y: buttonY, width: buttonWidth, height: buttonHeight)
        closeButton.frame = NSRect(x: startX + buttonWidth * 2, y: buttonY, width: buttonWidth, height: buttonHeight)

        // Add separators
        addCapsuleButtonSeparators(
            to: titleBarView,
            moreButton: moreButton,
            minimizeButton: minimizeButton,
            buttonY: buttonY,
            buttonHeight: buttonHeight
        )

        // Add to view
        titleBarView.addSubview(moreButton)
        titleBarView.addSubview(minimizeButton)
        titleBarView.addSubview(closeButton)

        // Ensure proper layering
        moreButton.layer?.zPosition = 1000
        minimizeButton.layer?.zPosition = 1000
        closeButton.layer?.zPosition = 1000
    }

    /// Removes capsule buttons from the title bar view
    public static func removeCapsuleButtons(from titleBarView: NSView) {
        // Remove buttons and separators
        titleBarView.subviews.forEach { subview in
            if subview is NSButton || subview.frame.width < 2 { // Separators have small width
                subview.removeFromSuperview()
            }
        }
    }

    private static func createCapsuleButton(image: NSImage?, target: AnyObject, action: Selector) -> NSButton {
        let button = NSButton()
        button.image = image
        button.target = target
        button.action = action
        button.isBordered = false
        button.bezelStyle = .regularSquare
        button.translatesAutoresizingMaskIntoConstraints = true
        button.imageScaling = .scaleProportionallyDown
        button.imagePosition = .imageOnly
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor
        button.setButtonType(.momentaryPushIn)
        return button
    }

    private static func addCapsuleButtonSeparators(
        to titleBarView: NSView,
        moreButton: NSButton,
        minimizeButton: NSButton,
        buttonY: CGFloat,
        buttonHeight: CGFloat
    ) {
        let separatorWidth: CGFloat = 0.5
        let separatorAlpha: CGFloat = 0.15
        let separatorHeight = buttonHeight - 12
        let separatorY = buttonY + 6

        let leftSeparator = NSView(frame: NSRect(
            x: moreButton.frame.maxX - separatorWidth/2,
            y: separatorY,
            width: separatorWidth,
            height: separatorHeight
        ))
        leftSeparator.wantsLayer = true
        leftSeparator.layer?.backgroundColor = NSColor.lightGray.withAlphaComponent(separatorAlpha).cgColor

        let rightSeparator = NSView(frame: NSRect(
            x: minimizeButton.frame.maxX - separatorWidth/2,
            y: separatorY,
            width: separatorWidth,
            height: separatorHeight
        ))
        rightSeparator.wantsLayer = true
        rightSeparator.layer?.backgroundColor = NSColor.lightGray.withAlphaComponent(separatorAlpha).cgColor

        titleBarView.addSubview(leftSeparator)
        titleBarView.addSubview(rightSeparator)
    }

    private static func createThreeDotsImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setFillColor(NSColor.darkGray.cgColor)

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
        }

        image.unlockFocus()
        return image
    }

    private static func createMinimizeButtonImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setLineWidth(3.5)
            context.setLineCap(.round)
            context.setStrokeColor(NSColor.darkGray.cgColor)

            let lineWidth: CGFloat = 10
            context.move(to: CGPoint(x: (size.width - lineWidth) / 2, y: size.height / 2))
            context.addLine(to: CGPoint(x: (size.width + lineWidth) / 2, y: size.height / 2))
            context.strokePath()
        }

        image.unlockFocus()
        return image
    }

    private static func createCloseButtonImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            let centerX = size.width / 2
            let centerY = size.height / 2
            let outerRadius = size.width * 0.35
            let innerRadius: CGFloat = 2.5

            context.setLineWidth(2.2)
            context.setStrokeColor(NSColor.darkGray.cgColor)
            context.setLineCap(.round)

            let outerCircle = CGRect(
                x: centerX - outerRadius,
                y: centerY - outerRadius,
                width: outerRadius * 2,
                height: outerRadius * 2
            )
            context.strokeEllipse(in: outerCircle)

            context.setFillColor(NSColor.darkGray.cgColor)
            let innerCircle = CGRect(
                x: centerX - innerRadius,
                y: centerY - innerRadius,
                width: innerRadius * 2,
                height: innerRadius * 2
            )
            context.fillEllipse(in: innerCircle)
        }

        image.unlockFocus()
        return image
    }
}

#endif
