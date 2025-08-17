import SwiftUI
import Foundation

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// Unified SwiftUI Capsule Button management for LxApp - supports both iOS and macOS
@MainActor
public class LxAppCapsuleButtons {
    private static let CAPSULE_BUTTON_TAG = 9999

    #if os(macOS)
    /// Creates and adds capsule buttons to the title bar view (macOS) - Legacy AppKit support
    public static func addCapsuleButtons(
        to titleBarView: NSView,
        windowWidth: CGFloat,
        target: AnyObject,
        moreAction: Selector,
        minimizeAction: Selector,
        closeAction: Selector
    ) {
        let metrics = LxAppTheme.Metrics.self
        let buttonWidth = metrics.capsuleButtonWidth / 3
        let buttonHeight = metrics.capsuleButtonHeight
        let buttonY = metrics.capsuleTopMargin
        let rightMargin = metrics.capsuleTrailingMargin

        // Create buttons with custom icons for consistent styling
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
        let startX = windowWidth - metrics.capsuleButtonWidth - rightMargin
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
        [moreButton, minimizeButton, closeButton].forEach { button in
            button.layer?.zPosition = 1000
        }
    }
    #endif

    #if os(iOS)
    /// Adds capsule button to the view controller (iOS) - SwiftUI implementation
    public static func addCapsuleButton(to viewController: UIViewController, appId: String) {
        guard viewController.view.viewWithTag(CAPSULE_BUTTON_TAG) == nil else { return }
        guard appId != LxAppCore.getHomeLxAppId() else { return }

        // Create SwiftUI capsule buttons using the new unified implementation
        let capsuleButtons = LxAppUnifiedCapsuleView(
            onMoreTapped: {
                // More options functionality
            },
            onCloseTapped: {
                if let iOSViewController = viewController as? iOSLxAppViewController {
                    iOSViewController.performLxAppClose()
                }
            }
        )

        // Create hosting controller
        let hostingController = UIHostingController(rootView: capsuleButtons)
        hostingController.view.backgroundColor = UIColor.clear
        hostingController.view.tag = CAPSULE_BUTTON_TAG
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false

        // Add to view hierarchy
        viewController.view.addSubview(hostingController.view)
        viewController.addChild(hostingController)

        let capsuleWidth = LxAppTheme.Metrics.capsuleButtonWidth
        let capsuleHeight = LxAppTheme.Metrics.capsuleButtonHeight
        let rightMargin = LxAppTheme.Metrics.capsuleTrailingMargin

        // Use the original iOS positioning to align with navigation bar title
        let topMargin: CGFloat = 56 // Status bar height (~47-48) + navigation alignment offset (8)

        // Set constraints
        NSLayoutConstraint.activate([
            hostingController.view.widthAnchor.constraint(equalToConstant: capsuleWidth),
            hostingController.view.heightAnchor.constraint(equalToConstant: capsuleHeight),
            hostingController.view.trailingAnchor.constraint(equalTo: viewController.view.trailingAnchor, constant: -rightMargin),
            hostingController.view.topAnchor.constraint(equalTo: viewController.view.topAnchor, constant: topMargin)
        ])

        hostingController.didMove(toParent: viewController)
    }
    #endif

    #if os(macOS)
    /// Removes capsule buttons from the title bar view (macOS)
    public static func removeCapsuleButtons(from titleBarView: NSView) {
        // Remove buttons and separators - same logic as old implementation
        titleBarView.subviews.forEach { subview in
            if subview is NSButton || subview.frame.width < 2 { // Separators have small width
                subview.removeFromSuperview()
            }
        }
    }
    #endif

    #if os(iOS)
    /// Removes capsule button from the view controller (iOS)
    public static func removeCapsuleButton(from viewController: UIViewController) {
        viewController.view.viewWithTag(CAPSULE_BUTTON_TAG)?.removeFromSuperview()
    }
    #endif

    #if os(macOS)
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

    // MARK: - Custom Image Creation for macOS AppKit Buttons

    public static func createThreeDotsImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setFillColor(NSColor.black.cgColor)

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

    public static func createMinimizeButtonImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setLineWidth(3.5)
            context.setLineCap(.round)
            context.setStrokeColor(NSColor.black.cgColor)

            let lineWidth: CGFloat = 10
            context.move(to: CGPoint(x: (size.width - lineWidth) / 2, y: size.height / 2))
            context.addLine(to: CGPoint(x: (size.width + lineWidth) / 2, y: size.height / 2))
            context.strokePath()
        }

        image.unlockFocus()
        return image
    }

    public static func createCloseButtonImage() -> NSImage {
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
            context.setStrokeColor(NSColor.black.cgColor)
            context.setLineCap(.round)

            let outerCircle = CGRect(
                x: centerX - outerRadius,
                y: centerY - outerRadius,
                width: outerRadius * 2,
                height: outerRadius * 2
            )
            context.strokeEllipse(in: outerCircle)

            context.setFillColor(NSColor.black.cgColor)
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
    #endif


}

/// Pure SwiftUI Capsule Button View - Cross-platform implementation
public struct LxAppCapsuleButtonView: View {
    let appId: String
    let onMoreTapped: () -> Void
    let onCloseTapped: () -> Void
    let onMinimizeTapped: (() -> Void)?

    @State private var isHomeLxApp: Bool = false

    public init(
        appId: String,
        onMoreTapped: @escaping () -> Void,
        onCloseTapped: @escaping () -> Void,
        onMinimizeTapped: (() -> Void)? = nil
    ) {
        self.appId = appId
        self.onMoreTapped = onMoreTapped
        self.onCloseTapped = onCloseTapped
        self.onMinimizeTapped = onMinimizeTapped
    }

    public var body: some View {
        if !isHomeLxApp {
            capsuleButtonContent
                .onAppear { checkHomeLxApp() }
        }
    }

    private var capsuleButtonContent: some View {
        HStack(spacing: 0) {
            // More button
            Button(action: onMoreTapped) {
                LxAppIcons.threeDots
                    .font(.system(size: 16))
                    .foregroundColor(.black)
            }
            .buttonStyle(PlainButtonStyle())
            .frame(width: buttonWidth, height: LxAppTheme.Metrics.capsuleButtonHeight)

            // Separator
            Rectangle()
                .fill(Color.black.opacity(0.2))
                .frame(width: 1.0, height: separatorHeight)

            // Minimize button (macOS only)
            #if os(macOS)
            if let onMinimize = onMinimizeTapped {
                Button(action: onMinimize) {
                    LxAppIcons.minimize
                        .font(.system(size: 16))
                        .foregroundColor(.black)
                }
                .buttonStyle(PlainButtonStyle())
                .frame(width: buttonWidth, height: LxAppTheme.Metrics.capsuleButtonHeight)

                Rectangle()
                    .fill(Color.black.opacity(0.2))
                    .frame(width: 1.0, height: separatorHeight)
            }
            #endif

            // Close button
            Button(action: onCloseTapped) {
                LxAppIcons.close
                    .font(.system(size: 16))
                    .foregroundColor(.black)
            }
            .buttonStyle(PlainButtonStyle())
            .frame(width: buttonWidth, height: LxAppTheme.Metrics.capsuleButtonHeight)
        }
        .background(
            RoundedRectangle(cornerRadius: LxAppTheme.Metrics.capsuleCornerRadius)
                .fill(Color.white.opacity(0.85))
                .background(.ultraThinMaterial)
                .overlay(
                    RoundedRectangle(cornerRadius: LxAppTheme.Metrics.capsuleCornerRadius)
                        .stroke(Color.black.opacity(0.15), lineWidth: 1.0)
                )
        )
        .frame(width: LxAppTheme.Metrics.capsuleButtonWidth, height: LxAppTheme.Metrics.capsuleButtonHeight)
    }

    private var buttonWidth: CGFloat {
        #if os(iOS)
        return LxAppTheme.Metrics.capsuleButtonWidth / 2
        #else
        return LxAppTheme.Metrics.capsuleButtonWidth / (onMinimizeTapped != nil ? 3 : 2)
        #endif
    }

    private var separatorHeight: CGFloat {
        LxAppTheme.Metrics.capsuleButtonHeight - 12
    }

    // MARK: - Lifecycle
    private func checkHomeLxApp() {
        isHomeLxApp = appId == LxAppCore.getHomeLxAppId()
    }
}

public struct LxAppCapsuleButtonModifier: ViewModifier {
    let appId: String
    let onMoreTapped: () -> Void
    let onCloseTapped: () -> Void
    let onMinimizeTapped: (() -> Void)?

    public init(
        appId: String,
        onMoreTapped: @escaping () -> Void,
        onCloseTapped: @escaping () -> Void,
        onMinimizeTapped: (() -> Void)? = nil
    ) {
        self.appId = appId
        self.onMoreTapped = onMoreTapped
        self.onCloseTapped = onCloseTapped
        self.onMinimizeTapped = onMinimizeTapped
    }

    public func body(content: Content) -> some View {
        ZStack {
            content

            VStack {
                HStack {
                    Spacer()

                    LxAppCapsuleButtonView(
                        appId: appId,
                        onMoreTapped: onMoreTapped,
                        onCloseTapped: onCloseTapped,
                        onMinimizeTapped: onMinimizeTapped
                    )
                    .padding(.trailing, platformTrailingPadding)
                }
                .padding(.top, platformTopPadding)

                Spacer()
            }
        }
    }

    private var platformTopPadding: CGFloat {
        let platform = LxAppTheme.platform
        let capsuleHeight = LxAppTheme.Metrics.capsuleButtonHeight
        let titleCenterY = platform.statusBarHeight + (platform.navigationBarHeight / 2)
        return titleCenterY - (capsuleHeight / 2)
    }

    private var platformTrailingPadding: CGFloat {
        LxAppTheme.Metrics.capsuleTrailingMargin
    }
}

public extension View {
    /// Adds capsule buttons to the view
    func lxAppCapsuleButtons(
        appId: String,
        onMoreTapped: @escaping () -> Void,
        onCloseTapped: @escaping () -> Void,
        onMinimizeTapped: (() -> Void)? = nil
    ) -> some View {
        self.modifier(
            LxAppCapsuleButtonModifier(
                appId: appId,
                onMoreTapped: onMoreTapped,
                onCloseTapped: onCloseTapped,
                onMinimizeTapped: onMinimizeTapped
            )
        )
    }

    /// Adds capsule buttons with default LxApp actions
    func lxAppCapsuleButtons(appId: String) -> some View {
        self.lxAppCapsuleButtons(
            appId: appId,
            onMoreTapped: {
                print("More button tapped")
            },
            onCloseTapped: {
                #if os(macOS)
                macOSLxApp.closeLxApp(appId: appId)
                #elseif os(iOS)
                iOSLxApp.closeLxApp(appId: appId)
                #endif
            },
            onMinimizeTapped: {
                #if os(macOS)
                if let window = NSApp.keyWindow {
                    window.miniaturize(nil)
                }
                #endif
            }
        )
    }
}

#if os(iOS)
/// Unified, standardized capsule button for LingXia, restoring the original visual style.
/// This view should be used across the app to ensure visual consistency.
public struct LxAppUnifiedCapsuleView: View {
    let onMoreTapped: () -> Void
    let onCloseTapped: () -> Void

    public init(onMoreTapped: @escaping () -> Void, onCloseTapped: @escaping () -> Void) {
        self.onMoreTapped = onMoreTapped
        self.onCloseTapped = onCloseTapped
    }

    public var body: some View {
        HStack(spacing: 0) {
            // More button
            Button(action: onMoreTapped) {
                if let moreImage = Self.createMoreDotsImageiOS() {
                    Image(uiImage: moreImage)
                } else {
                    Image(systemName: "ellipsis") // Fallback
                }
            }
            .frame(width: 43.5, height: 32)
            .contentShape(Rectangle())
            .buttonStyle(PlainButtonStyle())

            // Separator
            Rectangle()
                .fill(Color.gray.opacity(0.3))
                .frame(width: 0.5, height: 20)

            // Close button
            Button(action: onCloseTapped) {
                if let closeImage = Self.createCloseButtonImageiOS() {
                    Image(uiImage: closeImage)
                } else {
                    Image(systemName: "xmark") // Fallback
                }
            }
            .frame(width: 43.5, height: 32)
            .contentShape(Rectangle())
            .buttonStyle(PlainButtonStyle())
        }
        .frame(width: 87, height: 32)
        .background(
            Capsule().fill(Color.white.opacity(0.9))
                .background(.ultraThinMaterial)
        )
        .clipShape(Capsule())
        .overlay(
            Capsule().stroke(Color.gray.opacity(0.3), lineWidth: 0.5)
        )
        .shadow(color: .black.opacity(0.1), radius: 2, x: 0, y: 1)
    }
    
    private static func createMoreDotsImageiOS() -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)
        defer { UIGraphicsEndImageContext() }

        guard let context = UIGraphicsGetCurrentContext() else { return nil }

        context.setFillColor(UIColor.darkGray.cgColor)

        let centerY = size.height / 2
        let centerX = size.width / 2
        let centerDotRadius = size.height / 7
        let sideDotRadius = size.height / 10
        let spacing = centerDotRadius * 2.8

        context.fillEllipse(in: CGRect(x: centerX - spacing - sideDotRadius, y: centerY - sideDotRadius, width: sideDotRadius * 2, height: sideDotRadius * 2))
        context.fillEllipse(in: CGRect(x: centerX + spacing - sideDotRadius, y: centerY - sideDotRadius, width: sideDotRadius * 2, height: sideDotRadius * 2))
        context.fillEllipse(in: CGRect(x: centerX - centerDotRadius, y: centerY - centerDotRadius, width: centerDotRadius * 2, height: centerDotRadius * 2))

        return UIGraphicsGetImageFromCurrentImageContext()
    }

    private static func createCloseButtonImageiOS() -> UIImage? {
        let size = CGSize(width: 24, height: 24)
        UIGraphicsBeginImageContextWithOptions(size, false, 0)
        defer { UIGraphicsEndImageContext() }

        guard let context = UIGraphicsGetCurrentContext() else { return nil }

        context.setStrokeColor(UIColor.darkGray.cgColor)
        context.setLineWidth(2.2)
        context.setLineCap(.round)

        let centerX = size.width / 2
        let centerY = size.height / 2
        let outerRadius = size.width * 0.35
        let innerRadius: CGFloat = 2.5

        context.strokeEllipse(in: CGRect(x: centerX - outerRadius, y: centerY - outerRadius, width: outerRadius * 2, height: outerRadius * 2))
        context.fillEllipse(in: CGRect(x: centerX - innerRadius, y: centerY - innerRadius, width: innerRadius * 2, height: innerRadius * 2))

        return UIGraphicsGetImageFromCurrentImageContext()
    }
}
#endif