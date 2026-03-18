import SwiftUI
import Foundation

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

// Shared Image Generation Helper
struct LxAppImageHelper {
    static let imageSize = CGSize(width: 24, height: 24)

    // Shared drawing logic for three dots pattern (used by both iOS and macOS)
    static func drawThreeDotsPattern(in context: CGContext, size: CGSize) {
        let centerY = size.height / 2
        let centerX = size.width / 2
        let centerDotRadius = size.height / 7
        let sideDotRadius = size.height / 10
        let spacing = centerDotRadius * 2.8

        // Left dot
        context.fillEllipse(in: CGRect(x: centerX - spacing - sideDotRadius, y: centerY - sideDotRadius, width: sideDotRadius * 2, height: sideDotRadius * 2))
        // Right dot
        context.fillEllipse(in: CGRect(x: centerX + spacing - sideDotRadius, y: centerY - sideDotRadius, width: sideDotRadius * 2, height: sideDotRadius * 2))
        // Center dot
        context.fillEllipse(in: CGRect(x: centerX - centerDotRadius, y: centerY - centerDotRadius, width: centerDotRadius * 2, height: centerDotRadius * 2))
    }
}

/// Unified theme system for LxApp components
public struct LxAppTheme {

    public struct Colors {
        public static let background = Color(LxAppPlatformColor.lxSystemBackground)
        public static let text = Color(LxAppPlatformColor.lxLabel)
    }

    public struct Metrics {
        // Capsule buttons
        public static let capsuleButtonWidth: CGFloat = platformCapsuleWidth
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

        private static var platformCapsuleWidth: CGFloat {
            #if os(iOS)
            return 84.5
            #else
            return 87
            #endif
        }

        private static var platformTrailingMargin: CGFloat {
            #if os(iOS)
            return 12
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

        public static let capsuleTopMarginFromSafeArea: CGFloat = 8

        public static func calculateCapsuleTop(statusBarHeight: CGFloat) -> CGFloat {
            return statusBarHeight + capsuleTopMarginFromSafeArea
        }
    }

    @MainActor
    public static func getStatusBarHeight() -> CGFloat {
        #if os(iOS)
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
           let window = windowScene.windows.first {
            let safeAreaTop = window.safeAreaInsets.top
            if safeAreaTop > 0 {
                return safeAreaTop
            }
        }
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
            let height = windowScene.statusBarManager?.statusBarFrame.height ?? 44
            return height
        }
        return 44
        #else
        return 0
        #endif
    }

    public struct Typography {
        public static let navigationTitle = Font.system(size: 17, weight: .medium)
        public static let tabTitle = Font.system(size: 12, weight: .medium)

    }
}

/// Custom drawn icons for capsule buttons
struct LxAppCustomIcons {
    #if os(iOS)
    static let threeDots = Image(uiImage: createThreeDotsUIImage())
    static let close = Image(uiImage: createCloseUIImage())
    #else
    static let threeDots = Image(nsImage: createThreeDotsNSImage())
    static let close = Image(nsImage: createCloseNSImage())
    #endif

    #if os(iOS)
    private static func createThreeDotsUIImage() -> UIImage {
        let renderer = UIGraphicsImageRenderer(size: LxAppImageHelper.imageSize)
        return renderer.image { context in
            let cgContext = context.cgContext
            cgContext.setShouldAntialias(true)
            cgContext.setFillColor(UIColor.label.cgColor)
            LxAppImageHelper.drawThreeDotsPattern(in: cgContext, size: LxAppImageHelper.imageSize)
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

}

#if os(iOS)
public typealias LxAppPlatformColor = UIColor
typealias PlatformColor = UIColor
#else
public typealias LxAppPlatformColor = NSColor
typealias PlatformColor = NSColor
#endif

/// Unified color utilities
struct LxAppColorUtils {
    /// Parse color string to UInt32 ARGB
    static func parseColorString(_ colorStr: String, defaultColor: UInt32 = 0xFF000000) -> UInt32 {
        if colorStr.lowercased() == "transparent" {
            return 0x00000000
        }

        if colorStr.hasPrefix("#") {
            let hex = String(colorStr.dropFirst())
            if hex.count == 6, let rgb = UInt32(hex, radix: 16) {
                return 0xFF000000 | rgb // Add full alpha
            }
        }

        return defaultColor
    }

    /// Check if color is transparent
    static func isTransparent(_ colorValue: UInt32) -> Bool {
        return (colorValue >> 24) & 0xFF == 0
    }

    /// Create platform color from ARGB value
    static func platformColor(from argb: UInt32) -> PlatformColor {
        return PlatformColor(argb: argb)
    }

    /// Convert platform color to ARGB value
    static func argbValue(from color: PlatformColor) -> UInt32 {
        return color.toARGB()
    }
}

extension PlatformColor {
    /// Initialize color from a UInt32 ARGB value
    convenience init(argb: UInt32) {
        let alpha = CGFloat((argb >> 24) & 0xFF) / 255.0
        let red = CGFloat((argb >> 16) & 0xFF) / 255.0
        let green = CGFloat((argb >> 8) & 0xFF) / 255.0
        let blue = CGFloat(argb & 0xFF) / 255.0

        self.init(red: red, green: green, blue: blue, alpha: alpha)
    }

    /// Convert color to UInt32 ARGB value - unified implementation
    func toARGB() -> UInt32 {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0

        // Both iOS and macOS use the same method
        self.getRed(&red, green: &green, blue: &blue, alpha: &alpha)

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
    static var lxLabel: UIColor { UIColor.label }

    #else
    static var lxSystemBackground: NSColor { NSColor.windowBackgroundColor }
    static var lxLabel: NSColor { NSColor.labelColor }
    #endif
}
