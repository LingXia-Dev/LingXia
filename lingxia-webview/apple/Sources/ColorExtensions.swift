import Foundation

#if os(iOS)
import UIKit
#elseif os(macOS)
import Cocoa
#endif

/// Shared color extension for both iOS and macOS
extension PlatformColor {

    /// Create color from hex string
    /// Supports formats: #RGB, #RRGGBB, #AARRGGBB
    convenience init?(hexString: String) {
        let hex = hexString.trimmingCharacters(in: CharacterSet.alphanumerics.inverted)
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let a, r, g, b: UInt64

        switch hex.count {
        case 3: // RGB (12-bit)
            (a, r, g, b) = (255, (int >> 8) * 17, (int >> 4 & 0xF) * 17, (int & 0xF) * 17)
        case 6: // RGB (24-bit)
            (a, r, g, b) = (255, int >> 16, int >> 8 & 0xFF, int & 0xFF)
        case 8: // ARGB (32-bit)
            (a, r, g, b) = (int >> 24, int >> 16 & 0xFF, int >> 8 & 0xFF, int & 0xFF)
        default:
            return nil
        }

        #if os(iOS)
        self.init(
            red: CGFloat(r) / 255,
            green: CGFloat(g) / 255,
            blue: CGFloat(b) / 255,
            alpha: CGFloat(a) / 255
        )
        #else
        self.init(
            red: Double(r) / 255,
            green: Double(g) / 255,
            blue: Double(b) / 255,
            alpha: Double(a) / 255
        )
        #endif
    }

    /// Convert color to hex string
    var hexString: String {
        #if os(iOS)
        var r: CGFloat = 0
        var g: CGFloat = 0
        var b: CGFloat = 0
        var a: CGFloat = 0
        getRed(&r, green: &g, blue: &b, alpha: &a)

        let red = Int(r * 255)
        let green = Int(g * 255)
        let blue = Int(b * 255)

        return String(format: "#%02X%02X%02X", red, green, blue)
        #else
        guard let rgbColor = usingColorSpace(.deviceRGB) else {
            return "#000000"
        }

        let red = Int(rgbColor.redComponent * 255)
        let green = Int(rgbColor.greenComponent * 255)
        let blue = Int(rgbColor.blueComponent * 255)

        return String(format: "#%02X%02X%02X", red, green, blue)
        #endif
    }

    /// Create color from RGBA values (0-255 range)
    static func fromRGBA(red: Int, green: Int, blue: Int, alpha: Int = 255) -> PlatformColor {
        #if os(iOS)
        return PlatformColor(
            red: CGFloat(max(0, min(255, red))) / 255.0,
            green: CGFloat(max(0, min(255, green))) / 255.0,
            blue: CGFloat(max(0, min(255, blue))) / 255.0,
            alpha: CGFloat(max(0, min(255, alpha))) / 255.0
        )
        #else
        return PlatformColor(
            red: Double(max(0, min(255, red))) / 255.0,
            green: Double(max(0, min(255, green))) / 255.0,
            blue: Double(max(0, min(255, blue))) / 255.0,
            alpha: Double(max(0, min(255, alpha))) / 255.0
        )
        #endif
    }

    /// Parses RGBA color string (e.g., "rgba(255, 0, 0, 0.5)") - from original iOS implementation
    static func fromRGBAString(_ rgba: String) -> PlatformColor? {
        let trimmed = rgba.trimmingCharacters(in: .whitespacesAndNewlines)

        guard trimmed.hasPrefix("rgba(") && trimmed.hasSuffix(")") else {
            return nil
        }

        let values = trimmed
            .replacingOccurrences(of: "rgba(", with: "")
            .replacingOccurrences(of: ")", with: "")
            .components(separatedBy: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }

        guard values.count == 4,
              let r = Int(values[0]),
              let g = Int(values[1]),
              let b = Int(values[2]),
              let a = Float(values[3]) else {
            return nil
        }

        #if os(iOS)
        return PlatformColor(
            red: CGFloat(max(0, min(255, r))) / 255.0,
            green: CGFloat(max(0, min(255, g))) / 255.0,
            blue: CGFloat(max(0, min(255, b))) / 255.0,
            alpha: CGFloat(max(0, min(1, a)))
        )
        #else
        return PlatformColor(
            red: Double(max(0, min(255, r))) / 255.0,
            green: Double(max(0, min(255, g))) / 255.0,
            blue: Double(max(0, min(255, b))) / 255.0,
            alpha: Double(max(0, min(1, a)))
        )
        #endif
    }
}
