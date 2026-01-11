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
    public static let CAPSULE_BUTTON_TAG = 9999

    #if os(macOS)
    /// Add capsule button using SwiftUI
    public static func addCapsuleButton(to viewController: NSViewController, appId: String) {
        let identifier = NSUserInterfaceItemIdentifier("CapsuleButton_\(CAPSULE_BUTTON_TAG)")
        guard viewController.view.subviews.first(where: { $0.identifier == identifier }) == nil else {
            return
        }

        let capsuleButtons = LxAppUnifiedCapsuleViewMacOS(
            onMoreTapped: {
                let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionMore)
            },
            onMinimizeTapped: { viewController.view.window?.miniaturize(nil) },
            onCloseTapped: {
                let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionClose)
            }
        )

        let hostingController = NSHostingController(rootView: capsuleButtons)
        let wrapperView = NSView()
        wrapperView.identifier = identifier
        wrapperView.translatesAutoresizingMaskIntoConstraints = false
        wrapperView.addSubview(hostingController.view)
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false

        viewController.view.addSubview(wrapperView)
        viewController.addChild(hostingController)

        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: wrapperView.topAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: wrapperView.leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: wrapperView.trailingAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: wrapperView.bottomAnchor),
            wrapperView.topAnchor.constraint(equalTo: viewController.view.topAnchor, constant: LxAppTheme.Metrics.capsuleTopMargin),
            wrapperView.trailingAnchor.constraint(equalTo: viewController.view.trailingAnchor, constant: -LxAppTheme.Metrics.capsuleTrailingMargin),
            wrapperView.widthAnchor.constraint(equalToConstant: LxAppTheme.Metrics.capsuleButtonWidth),
            wrapperView.heightAnchor.constraint(equalToConstant: LxAppTheme.Metrics.capsuleButtonHeight)
        ])
    }
    #endif

    #if os(iOS)
    public static func addCapsuleButton(to viewController: UIViewController, appId: String) {
        guard viewController.view.viewWithTag(CAPSULE_BUTTON_TAG) == nil else { return }

        let capsuleButtons = LxAppUnifiedCapsuleView(
            onMoreTapped: {
                let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionMore)
            },
            onCloseTapped: {
                let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionClose)
            }
        )

        let hostingController = UIHostingController(rootView: capsuleButtons)
        hostingController.view.backgroundColor = UIColor.clear
        hostingController.view.tag = CAPSULE_BUTTON_TAG
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false

        viewController.view.addSubview(hostingController.view)
        viewController.addChild(hostingController)
        hostingController.didMove(toParent: viewController)

        let statusBarHeight = LxAppTheme.getStatusBarHeight()
        let navbarCenterY = statusBarHeight + (LxAppTheme.Metrics.navigationBarHeight / 2)
        let topMargin = navbarCenterY - (LxAppTheme.Metrics.capsuleButtonHeight / 2)

        NSLayoutConstraint.activate([
            hostingController.view.widthAnchor.constraint(equalToConstant: LxAppTheme.Metrics.capsuleButtonWidth),
            hostingController.view.heightAnchor.constraint(equalToConstant: LxAppTheme.Metrics.capsuleButtonHeight),
            hostingController.view.trailingAnchor.constraint(equalTo: viewController.view.trailingAnchor, constant: -LxAppTheme.Metrics.capsuleTrailingMargin),
            hostingController.view.topAnchor.constraint(equalTo: viewController.view.topAnchor, constant: topMargin)
        ])
    }
    #endif

    #if os(macOS)
    /// Removes capsule buttons from the title bar view (macOS)
    public static func removeCapsuleButtons(from titleBarView: NSView) {
        titleBarView.subviews.forEach { subview in
            if subview is NSButton || subview.frame.width < 2 {
                subview.removeFromSuperview()
            }
        }
    }
    #endif

    #if os(iOS)
    public static func removeCapsuleButton(from viewController: UIViewController) {
        viewController.view.viewWithTag(CAPSULE_BUTTON_TAG)?.removeFromSuperview()
    }
    #endif

    #if os(macOS)
    public static func removeCapsuleButton(from viewController: NSViewController) {
        let identifier = NSUserInterfaceItemIdentifier("CapsuleButton_\(CAPSULE_BUTTON_TAG)")
        viewController.view.subviews.first { $0.identifier == identifier }?.removeFromSuperview()
    }

    public static func createThreeDotsImage() -> NSImage {
        let image = NSImage(size: LxAppImageHelper.imageSize)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setFillColor(NSColor.black.cgColor)
            LxAppImageHelper.drawThreeDotsPattern(in: context, size: LxAppImageHelper.imageSize)
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
        capsuleButtonContent
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
        #if os(iOS)
        let statusBarHeight: CGFloat
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
            statusBarHeight = windowScene.statusBarManager?.statusBarFrame.height ?? 44
        } else {
            statusBarHeight = 44
        }

        // Align with navbar center
        let navbarCenterY = statusBarHeight + (LxAppTheme.Metrics.navigationBarHeight / 2)
        return navbarCenterY - (LxAppTheme.Metrics.capsuleButtonHeight / 2)
        #else
        return 0
        #endif
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
                let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionMore)
            },
            onCloseTapped: {
                let _ = onUiEvent(appId, LxAppUIEvent.capsuleClick, LxAppUIEvent.capsuleActionClose)
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

#if os(macOS)
public struct LxAppUnifiedCapsuleViewMacOS: View {
    let onMoreTapped: () -> Void
    let onMinimizeTapped: () -> Void
    let onCloseTapped: () -> Void

    public var body: some View {
        HStack(spacing: 0) {
            Button(action: onMoreTapped) {
                Image(nsImage: LxAppCapsuleButtons.createThreeDotsImage())
            }
            .frame(width: 29, height: 32)
            .buttonStyle(PlainButtonStyle())

            Rectangle().fill(Color.gray.opacity(0.3)).frame(width: 0.5, height: 20)

            Button(action: onMinimizeTapped) {
                Image(nsImage: LxAppCapsuleButtons.createMinimizeButtonImage())
            }
            .frame(width: 29, height: 32)
            .buttonStyle(PlainButtonStyle())

            Rectangle().fill(Color.gray.opacity(0.3)).frame(width: 0.5, height: 20)

            Button(action: onCloseTapped) {
                Image(nsImage: LxAppCapsuleButtons.createCloseButtonImage())
            }
            .frame(width: 29, height: 32)
            .buttonStyle(PlainButtonStyle())
        }
        .frame(width: 87, height: 32)
        .background(Capsule().fill(Color.white.opacity(0.9)).background(.ultraThinMaterial))
        .clipShape(Capsule())
        .overlay(Capsule().stroke(Color.gray.opacity(0.3), lineWidth: 0.5))
        .shadow(color: .black.opacity(0.1), radius: 2, x: 0, y: 1)
    }
}
#endif

#if os(iOS)
private enum CapsuleMetrics {
    static let height: CGFloat = 32
    static let buttonWidth: CGFloat = 38
    static let dividerWidth: CGFloat = 0.5
    static let dividerHeight: CGFloat = 20
    static let iconMaxWidth: CGFloat = 28
    static let iconMaxHeight: CGFloat = 20
    static let edgePadding: CGFloat = 4
    static let totalWidth: CGFloat =
        buttonWidth * 2 + dividerWidth + edgePadding * 2
}

private struct CapsuleIcon: View {
    let name: String

    var body: some View {
        if let image = LxIcon.image(named: name) {
            Image(uiImage: image)
                .resizable()
                .aspectRatio(contentMode: .fit)
                .frame(
                    maxWidth: CapsuleMetrics.iconMaxWidth,
                    maxHeight: CapsuleMetrics.iconMaxHeight
                )
        }
    }
}

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
                CapsuleIcon(name: "icon_capsule_menu")
            }
            .frame(width: CapsuleMetrics.buttonWidth, height: CapsuleMetrics.height)
            .contentShape(Rectangle())
            .buttonStyle(PlainButtonStyle())

            // Separator
            Rectangle()
                .fill(Color.gray.opacity(0.3))
                .frame(width: CapsuleMetrics.dividerWidth, height: CapsuleMetrics.dividerHeight)

            // Close button
            Button(action: onCloseTapped) {
                CapsuleIcon(name: "icon_capsule_close")
            }
            .frame(width: CapsuleMetrics.buttonWidth, height: CapsuleMetrics.height)
            .contentShape(Rectangle())
            .buttonStyle(PlainButtonStyle())
        }
        .padding(.horizontal, CapsuleMetrics.edgePadding)
        .frame(width: CapsuleMetrics.totalWidth, height: CapsuleMetrics.height)
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
}
#endif
