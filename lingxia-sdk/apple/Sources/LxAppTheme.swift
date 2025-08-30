import SwiftUI
import Foundation

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// Unified theme system for LxApp components
public struct LxAppTheme {

    public struct Colors {
        public static let background = Color(LxAppPlatformColor.lxSystemBackground)
        public static let secondaryBackground = Color(LxAppPlatformColor.lxSecondarySystemBackground)
        public static let text = Color(LxAppPlatformColor.lxLabel)
        public static let secondaryText = Color(LxAppPlatformColor.lxSecondaryLabel)
        public static let separator = Color(LxAppPlatformColor.lxSeparator)
        public static let accent = Color(LxAppPlatformColor.lxControlAccentColor)

        // Capsule button colors
        public static let capsuleBackground = Color.white.opacity(0.9)
        public static let capsuleBorder = Color(red: 0.867, green: 0.867, blue: 0.867) // #DDDDDD
        public static let capsuleIcon = Color(red: 0.4, green: 0.4, blue: 0.4) // #666666 darker gray

        // Navigation bar colors
        public static let navigationBackground = Color(LxAppPlatformColor.lxSystemBackground)
        public static let navigationText = Color(LxAppPlatformColor.lxLabel)
    }

    public struct Metrics {
        // Capsule buttons
        public static let capsuleButtonWidth: CGFloat = 87
        public static let capsuleButtonHeight: CGFloat = platformCapsuleHeight
        public static let capsuleCornerRadius: CGFloat = platformCapsuleHeight / 2
        public static let capsuleTopMargin: CGFloat = 2
        public static let capsuleTrailingMargin: CGFloat = platformTrailingMargin

        // Navigation bar
        public static let navigationBarHeight: CGFloat = platformNavigationHeight

        // Tab bar
        public static let tabBarHeight: CGFloat = 64
        public static let tabIconSize: CGFloat = 24

        // Spacing
        public static let standardSpacing: CGFloat = 8
        public static let largeSpacing: CGFloat = 16
        public static let smallSpacing: CGFloat = 4

        // Platform-specific values
        private static var platformCapsuleHeight: CGFloat {
            #if os(iOS)
            return 32
            #else
            return 28
            #endif
        }

        private static var platformTrailingMargin: CGFloat {
            #if os(iOS)
            return 16
            #else
            return 7
            #endif
        }

        private static var platformNavigationHeight: CGFloat {
            #if os(iOS)
            return 44
            #else
            return 32
            #endif
        }
    }

    /// Get the current status bar height dynamically
    /// This should be called once at app startup and cached
    @MainActor
    public static func getStatusBarHeight() -> CGFloat {
        #if os(iOS)
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
            let height = windowScene.statusBarManager?.statusBarFrame.height ?? 20
            return height  // Return actual system status bar height
        }
        return 20  // Standard fallback
        #else
        return 0
        #endif
    }

    public struct Typography {
        public static let navigationTitle = Font.system(size: 17, weight: .medium)
        public static let tabTitle = Font.system(size: 12, weight: .medium)
        public static let body = Font.system(size: 16)
        public static let caption = Font.system(size: 12)
    }

    public struct Animations {
        public static let standard = Animation.easeInOut(duration: 0.3)
        public static let quick = Animation.easeInOut(duration: 0.15)
        public static let slow = Animation.easeInOut(duration: 0.5)
    }
}

public extension LxAppTheme {
    @MainActor
    static var platform: (statusBarHeight: CGFloat, navigationBarHeight: CGFloat) {
        return (statusBarHeight: getStatusBarHeight(), navigationBarHeight: Metrics.navigationBarHeight)
    }
}

/// Custom drawn icons for capsule buttons
public struct LxAppCustomIcons {
    #if os(iOS)
    public static let threeDots = Image(uiImage: createThreeDotsUIImage())
    public static let close = Image(uiImage: createCloseUIImage())
    #else
    public static let threeDots = Image(nsImage: createThreeDotsNSImage())
    public static let close = Image(nsImage: createCloseNSImage())
    #endif

    #if os(iOS)
    private static func createThreeDotsUIImage() -> UIImage {
        let size = CGSize(width: 24, height: 24)
        let renderer = UIGraphicsImageRenderer(size: size)
        return renderer.image { context in
            let cgContext = context.cgContext
            cgContext.setShouldAntialias(true)
            cgContext.setFillColor(UIColor.label.cgColor)

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
            cgContext.fillEllipse(in: leftDotRect)

            // Right dot
            let rightDotRect = CGRect(
                x: centerX + spacing - sideDotRadius,
                y: centerY - sideDotRadius,
                width: sideDotRadius * 2,
                height: sideDotRadius * 2
            )
            cgContext.fillEllipse(in: rightDotRect)

            // Center dot
            let centerDotRect = CGRect(
                x: centerX - centerDotRadius,
                y: centerY - centerDotRadius,
                width: centerDotRadius * 2,
                height: centerDotRadius * 2
            )
            cgContext.fillEllipse(in: centerDotRect)
        }
    }

    private static func createCloseUIImage() -> UIImage {
        let size = CGSize(width: 24, height: 24)
        let renderer = UIGraphicsImageRenderer(size: size)
        return renderer.image { context in
            let cgContext = context.cgContext
            cgContext.setShouldAntialias(true)
            let centerX = size.width / 2
            let centerY = size.height / 2
            let outerRadius = size.width * 0.35
            let innerRadius: CGFloat = 2.5

            cgContext.setLineWidth(2.2)
            cgContext.setStrokeColor(UIColor.label.cgColor)
            cgContext.setLineCap(.round)

            let outerCircle = CGRect(
                x: centerX - outerRadius,
                y: centerY - outerRadius,
                width: outerRadius * 2,
                height: outerRadius * 2
            )
            cgContext.strokeEllipse(in: outerCircle)

            cgContext.setFillColor(UIColor.label.cgColor)
            let innerCircle = CGRect(
                x: centerX - innerRadius,
                y: centerY - innerRadius,
                width: innerRadius * 2,
                height: innerRadius * 2
            )
            cgContext.fillEllipse(in: innerCircle)
        }
    }
    #endif

    #if os(macOS)
    private static func createThreeDotsNSImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()

        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setFillColor(NSColor.labelColor.cgColor)

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

    private static func createCloseNSImage() -> NSImage {
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
            context.setStrokeColor(NSColor.labelColor.cgColor)
            context.setLineCap(.round)

            let outerCircle = CGRect(
                x: centerX - outerRadius,
                y: centerY - outerRadius,
                width: outerRadius * 2,
                height: outerRadius * 2
            )
            context.strokeEllipse(in: outerCircle)

            context.setFillColor(NSColor.labelColor.cgColor)
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

/// Unified icon system with custom drawn icons for capsule buttons
public struct LxAppIcons {
    public static let threeDots = LxAppCustomIcons.threeDots
    public static let close = LxAppCustomIcons.close
    public static let minimize = Image(systemName: "minus")
    public static let back = Image(systemName: "chevron.left")
    public static let home = Image(systemName: "house.fill")
    public static let loading = Image(systemName: "arrow.clockwise")
}



#if os(iOS)
public typealias LxAppPlatformColor = UIColor
public typealias PlatformColor = UIColor
#else
public typealias LxAppPlatformColor = NSColor
public typealias PlatformColor = NSColor
#endif

extension PlatformColor {
    /// Initialize color from a UInt32 ARGB value
    convenience init(argb: UInt32) {
        let alpha = CGFloat((argb >> 24) & 0xFF) / 255.0
        let red = CGFloat((argb >> 16) & 0xFF) / 255.0
        let green = CGFloat((argb >> 8) & 0xFF) / 255.0
        let blue = CGFloat(argb & 0xFF) / 255.0

        self.init(red: red, green: green, blue: blue, alpha: alpha)
    }

    /// Convert color to UInt32 ARGB value
    func toARGB() -> UInt32 {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0

        #if os(iOS)
        self.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        #else
        self.getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        #endif

        let a = UInt32(alpha * 255) & 0xFF
        let r = UInt32(red * 255) & 0xFF
        let g = UInt32(green * 255) & 0xFF
        let b = UInt32(blue * 255) & 0xFF

        return (a << 24) | (r << 16) | (g << 8) | b
    }
}

extension LxAppPlatformColor {
    #if os(iOS)
    static var lxSystemBackground: UIColor { UIColor.systemBackground }
    static var lxSecondarySystemBackground: UIColor { UIColor.secondarySystemBackground }
    static var lxLabel: UIColor { UIColor.label }
    static var lxSecondaryLabel: UIColor { UIColor.secondaryLabel }
    static var lxSeparator: UIColor { UIColor.separator }
    static var lxControlAccentColor: UIColor { UIColor.systemBlue }
    static var lxDarkGray: UIColor { UIColor.darkGray }
    static var lxLightGray: UIColor { UIColor.lightGray }
    #else
    static var lxSystemBackground: NSColor { NSColor.windowBackgroundColor }
    static var lxSecondarySystemBackground: NSColor { NSColor.controlBackgroundColor }
    static var lxLabel: NSColor { NSColor.labelColor }
    static var lxSecondaryLabel: NSColor { NSColor.secondaryLabelColor }
    static var lxSeparator: NSColor { NSColor.separatorColor }
    static var lxControlAccentColor: NSColor { NSColor.systemBlue }
    static var lxDarkGray: NSColor { NSColor.darkGray }
    static var lxLightGray: NSColor { NSColor.lightGray }
    #endif
}
