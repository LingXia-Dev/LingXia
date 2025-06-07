import UIKit

/// Extension to UIColor for hex string support
/// Provides functionality to create UIColor instances from hex color strings
extension UIColor {

    /// Creates a UIColor from a hex string
    /// - Parameter hexString: Hex color string (e.g., "#FF0000", "#ff0000", "FF0000")
    /// - Returns: UIColor instance if parsing succeeds, nil otherwise
    convenience init?(hexString: String) {
        let r, g, b, a: CGFloat

        var hexColor = hexString.trimmingCharacters(in: .whitespacesAndNewlines)

        // Validate input is not empty
        guard !hexColor.isEmpty else { return nil }

        // Remove # prefix if present
        if hexColor.hasPrefix("#") {
            hexColor = String(hexColor.dropFirst())
        }

        // Validate hex characters only
        let hexCharacterSet = CharacterSet(charactersIn: "0123456789ABCDEFabcdef")
        guard hexColor.unicodeScalars.allSatisfy({ hexCharacterSet.contains($0) }) else {
            return nil
        }

        // Support both 6-digit (RGB) and 8-digit (ARGB) hex strings
        switch hexColor.count {
        case 6: // RGB
            let scanner = Scanner(string: hexColor)
            var hexNumber: UInt64 = 0

            if scanner.scanHexInt64(&hexNumber) {
                r = CGFloat((hexNumber & 0xFF0000) >> 16) / 255
                g = CGFloat((hexNumber & 0x00FF00) >> 8) / 255
                b = CGFloat(hexNumber & 0x0000FF) / 255
                a = 1.0

                self.init(red: r, green: g, blue: b, alpha: a)
                return
            }

        case 8: // ARGB
            let scanner = Scanner(string: hexColor)
            var hexNumber: UInt64 = 0

            if scanner.scanHexInt64(&hexNumber) {
                a = CGFloat((hexNumber & 0xFF000000) >> 24) / 255
                r = CGFloat((hexNumber & 0x00FF0000) >> 16) / 255
                g = CGFloat((hexNumber & 0x0000FF00) >> 8) / 255
                b = CGFloat(hexNumber & 0x000000FF) / 255

                self.init(red: r, green: g, blue: b, alpha: a)
                return
            }

        default:
            return nil
        }

        return nil
    }

    /// Converts UIColor to hex string representation
    /// - Parameter includeAlpha: Whether to include alpha channel in the output
    /// - Returns: Hex string representation (e.g., "#FF0000" or "#FFFF0000")
    func toHexString(includeAlpha: Bool = false) -> String {
        var r: CGFloat = 0
        var g: CGFloat = 0
        var b: CGFloat = 0
        var a: CGFloat = 0

        getRed(&r, green: &g, blue: &b, alpha: &a)

        let rgb = Int(r * 255) << 16 | Int(g * 255) << 8 | Int(b * 255)

        if includeAlpha {
            let argb = Int(a * 255) << 24 | rgb
            return String(format: "#%08X", argb)
        } else {
            return String(format: "#%06X", rgb)
        }
    }

    /// Creates a UIColor from RGBA values (0-255 range)
    /// - Parameters:
    ///   - red: Red component (0-255)
    ///   - green: Green component (0-255)
    ///   - blue: Blue component (0-255)
    ///   - alpha: Alpha component (0-255), default 255
    /// - Returns: UIColor instance
    static func fromRGBA(red: Int, green: Int, blue: Int, alpha: Int = 255) -> UIColor {
        return UIColor(
            red: CGFloat(max(0, min(255, red))) / 255.0,
            green: CGFloat(max(0, min(255, green))) / 255.0,
            blue: CGFloat(max(0, min(255, blue))) / 255.0,
            alpha: CGFloat(max(0, min(255, alpha))) / 255.0
        )
    }

    /// Parses RGBA color string (e.g., "rgba(255, 0, 0, 0.5)")
    /// - Parameter rgba: RGBA string
    /// - Returns: UIColor instance if parsing succeeds, nil otherwise
    static func fromRGBAString(_ rgba: String) -> UIColor? {
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

        return UIColor(
            red: CGFloat(max(0, min(255, r))) / 255.0,
            green: CGFloat(max(0, min(255, g))) / 255.0,
            blue: CGFloat(max(0, min(255, b))) / 255.0,
            alpha: CGFloat(max(0, min(1, a)))
        )
    }
}
