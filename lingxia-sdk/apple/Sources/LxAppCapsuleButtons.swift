import SwiftUI
import Foundation
import os.log

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

private let capsuleLog = OSLog(subsystem: "LingXia", category: "Capsule")

/// Capsule Button management for LxApp (iOS only)
@MainActor
public class LxAppCapsuleButtons {
    public static let CAPSULE_BUTTON_TAG = 9999

    #if os(iOS)
    public static func addCapsuleButton(to viewController: UIViewController, appId: String) {
        guard viewController.view.viewWithTag(CAPSULE_BUTTON_TAG) == nil else { return }

        let capsuleButtons = LxAppUnifiedCapsuleView(
            onMoreTapped: {
                LxAppCapsuleMenu.show(appId: appId)
            },
            onCloseTapped: {
                let _ = onLxappEvent(appId, LxAppEvent.capsuleClick, LxAppEvent.capsuleActionClose)
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
        let topMargin = LxAppTheme.Metrics.calculateCapsuleTop(statusBarHeight: statusBarHeight)

        NSLayoutConstraint.activate([
            hostingController.view.widthAnchor.constraint(equalToConstant: LxAppTheme.Metrics.capsuleButtonWidth),
            hostingController.view.heightAnchor.constraint(equalToConstant: LxAppTheme.Metrics.capsuleButtonHeight),
            hostingController.view.trailingAnchor.constraint(equalTo: viewController.view.trailingAnchor, constant: -LxAppTheme.Metrics.capsuleTrailingMargin),
            hostingController.view.topAnchor.constraint(equalTo: viewController.view.topAnchor, constant: topMargin)
        ])
        
        viewController.view.layoutIfNeeded()
        let frame = hostingController.view.frame
        os_log("Actual capsule frame: x=%{public}.1f, y=%{public}.1f, width=%{public}.1f, height=%{public}.1f", log: capsuleLog, type: .info, frame.origin.x, frame.origin.y, frame.width, frame.height)
        os_log("statusBarHeight=%{public}.1f, topMargin=%{public}.1f", log: capsuleLog, type: .info, statusBarHeight, topMargin)
    }

    public static func removeCapsuleButton(from viewController: UIViewController) {
        viewController.view.viewWithTag(CAPSULE_BUTTON_TAG)?.removeFromSuperview()
    }

    public static func getMenuButtonBoundingRect() -> [String: Double] {
        let statusBarHeight = LxAppTheme.getStatusBarHeight()
        
        // Match Web layout centering offset.
        let top = statusBarHeight

        let screenWidth = UIScreen.main.bounds.width

        let width = LxAppTheme.Metrics.capsuleButtonWidth
        let height = LxAppTheme.Metrics.capsuleButtonHeight
        let right = screenWidth - LxAppTheme.Metrics.capsuleTrailingMargin
        let left = right - width
        let bottom = top + height

        os_log("getCapsuleRect: statusBarHeight=%{public}.1f, top=%{public}.1f, screenWidth=%{public}.1f", log: capsuleLog, type: .info, statusBarHeight, top, screenWidth)

        return [
            "width": Double(width),
            "height": Double(height),
            "top": Double(top),
            "right": Double(right),
            "bottom": Double(bottom),
            "left": Double(left)
        ]
    }

    /// Get menu button bounding rect as JSON string (for FFI)
    public static func getMenuButtonBoundingRectJSON() -> String {
        let rect = getMenuButtonBoundingRect()
        guard let jsonData = try? JSONSerialization.data(withJSONObject: rect, options: []),
              let jsonString = String(data: jsonData, encoding: .utf8) else {
            return "{}"
        }
        return jsonString
    }

    /// Get capsule rect with async callback pattern (for cross-platform consistency)
    nonisolated public static func getCapsuleRect(callback_id: UInt64) {
        Task { @MainActor in
            let jsonString = getMenuButtonBoundingRectJSON()
            if jsonString.isEmpty || jsonString == "{}" {
                let _ = onCallback(callback_id, false, "2001")
            } else {
                let _ = onCallback(callback_id, true, jsonString)
            }
        }
    }
    #endif

    #if os(macOS)
    /// Get capsule rect with async callback pattern (macOS stub - Capsule mode in lingxia-runner)
    nonisolated public static func getCapsuleRect(callback_id: UInt64) {
        Task { @MainActor in
            // Return a minimal rect for macOS
            let rect: [String: Double] = [
                "width": 0,
                "height": 0,
                "top": 0,
                "right": 0,
                "bottom": 0,
                "left": 0
            ]
            if let jsonData = try? JSONSerialization.data(withJSONObject: rect, options: []),
               let jsonString = String(data: jsonData, encoding: .utf8) {
                let _ = onCallback(callback_id, true, jsonString)
            } else {
                let _ = onCallback(callback_id, false, "2001")
            }
        }
    }
    #endif
}

// MARK: - iOS Capsule UI

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
