import Foundation

#if os(iOS)
import UIKit
typealias NativeComponentColor = UIColor
#elseif os(macOS)
import AppKit
typealias NativeComponentColor = NSColor
#endif

enum NativeComponentColorStyle {
    static func extractColorFromStyle(_ style: String) -> String? {
        for segment in style.split(separator: ";") {
            let parts = segment.split(separator: ":", maxSplits: 1)
            guard parts.count == 2 else { continue }
            let key = String(parts[0]).trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if key == "color" {
                let value = String(parts[1]).trimmingCharacters(in: .whitespacesAndNewlines)
                if !value.isEmpty {
                    return value
                }
            }
        }
        return nil
    }

    static func parseColor(_ raw: String) -> NativeComponentColor? {
        let text = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if text.isEmpty {
            return nil
        }
        if text.hasPrefix("#"), let color = parseHexColor(text) {
            return color
        }
        if text.hasPrefix("rgb("), text.hasSuffix(")") {
            return parseRgbColor(text)
        }
        if text.hasPrefix("rgba("), text.hasSuffix(")") {
            return parseRgbaColor(text)
        }
        return nil
    }

    private static func parseHexColor(_ text: String) -> NativeComponentColor? {
        let hex = String(text.dropFirst())
        guard hex.count == 6 || hex.count == 8 else { return nil }
        var value: UInt64 = 0
        guard Scanner(string: hex).scanHexInt64(&value) else { return nil }
        if hex.count == 6 {
            let r = CGFloat((value >> 16) & 0xFF) / 255.0
            let g = CGFloat((value >> 8) & 0xFF) / 255.0
            let b = CGFloat(value & 0xFF) / 255.0
            #if os(iOS)
            return UIColor(red: r, green: g, blue: b, alpha: 1)
            #elseif os(macOS)
            return NSColor(calibratedRed: r, green: g, blue: b, alpha: 1)
            #endif
        }
        let r = CGFloat((value >> 24) & 0xFF) / 255.0
        let g = CGFloat((value >> 16) & 0xFF) / 255.0
        let b = CGFloat((value >> 8) & 0xFF) / 255.0
        let a = CGFloat(value & 0xFF) / 255.0
        #if os(iOS)
        return UIColor(red: r, green: g, blue: b, alpha: a)
        #elseif os(macOS)
        return NSColor(calibratedRed: r, green: g, blue: b, alpha: a)
        #endif
    }

    private static func parseRgbColor(_ text: String) -> NativeComponentColor? {
        let body = text.dropFirst(4).dropLast()
        let parts = body.split(separator: ",").map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        guard parts.count == 3,
              let r = Double(parts[0]),
              let g = Double(parts[1]),
              let b = Double(parts[2]) else {
            return nil
        }
        #if os(iOS)
        return UIColor(
            red: CGFloat(max(0, min(255, r))) / 255.0,
            green: CGFloat(max(0, min(255, g))) / 255.0,
            blue: CGFloat(max(0, min(255, b))) / 255.0,
            alpha: 1
        )
        #elseif os(macOS)
        return NSColor(
            calibratedRed: CGFloat(max(0, min(255, r))) / 255.0,
            green: CGFloat(max(0, min(255, g))) / 255.0,
            blue: CGFloat(max(0, min(255, b))) / 255.0,
            alpha: 1
        )
        #endif
    }

    private static func parseRgbaColor(_ text: String) -> NativeComponentColor? {
        let body = text.dropFirst(5).dropLast()
        let parts = body.split(separator: ",").map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
        guard parts.count == 4,
              let r = Double(parts[0]),
              let g = Double(parts[1]),
              let b = Double(parts[2]),
              let a = Double(parts[3]) else {
            return nil
        }
        #if os(iOS)
        return UIColor(
            red: CGFloat(max(0, min(255, r))) / 255.0,
            green: CGFloat(max(0, min(255, g))) / 255.0,
            blue: CGFloat(max(0, min(255, b))) / 255.0,
            alpha: CGFloat(max(0, min(1, a)))
        )
        #elseif os(macOS)
        return NSColor(
            calibratedRed: CGFloat(max(0, min(255, r))) / 255.0,
            green: CGFloat(max(0, min(255, g))) / 255.0,
            blue: CGFloat(max(0, min(255, b))) / 255.0,
            alpha: CGFloat(max(0, min(1, a)))
        )
        #endif
    }
}
