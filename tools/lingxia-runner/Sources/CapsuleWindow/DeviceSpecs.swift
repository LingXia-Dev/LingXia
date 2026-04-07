import Foundation
import CoreGraphics

/// Predefined device sizes for Runner window sizing
public enum MobileDeviceSize: Equatable, CaseIterable {
    // ── iPhone ──
    case iPhoneSE           // 375 × 667
    case iPhone13Mini       // 375 × 812
    case iPhone13Pro        // 390 × 844
    case iPhone15Pro        // 393 × 852
    case iPhone11           // 414 × 896
    case iPhone15ProMax     // 430 × 932
    // ── iPad ──
    case iPad               // 768 × 1024
    case iPadPro            // 1024 × 1366
    // ── Desktop ──
    case desktop1280        // 1280 × 800
    case desktop1440        // 1440 × 900
    case desktop1920        // 1920 × 1080

    // CaseIterable conformance
    public static var allCases: [MobileDeviceSize] {
        return [
            .iPhoneSE, .iPhone13Mini, .iPhone13Pro, .iPhone15Pro, .iPhone11, .iPhone15ProMax,
            .iPad, .iPadPro,
            .desktop1280, .desktop1440, .desktop1920,
        ]
    }

    /// True for iPad and desktop sizes — skips phone-shell UI overlay
    public var isDesktop: Bool {
        switch self {
        case .iPad, .iPadPro, .desktop1280, .desktop1440, .desktop1920: return true
        default: return false
        }
    }

    public var width: CGFloat {
        switch self {
        case .iPhoneSE, .iPhone13Mini: return 375
        case .iPhone13Pro: return 390
        case .iPhone15Pro: return 393
        case .iPhone11: return 414
        case .iPhone15ProMax: return 430
        case .iPad: return 768
        case .iPadPro: return 1024
        case .desktop1280: return 1280
        case .desktop1440: return 1440
        case .desktop1920: return 1920
        }
    }

    public var height: CGFloat {
        switch self {
        case .iPhoneSE: return 667
        case .iPhone13Mini: return 812
        case .iPhone13Pro: return 844
        case .iPhone15Pro: return 852
        case .iPhone11: return 896
        case .iPhone15ProMax: return 932
        case .iPad: return 1024
        case .iPadPro: return 1366
        case .desktop1280: return 800
        case .desktop1440: return 900
        case .desktop1920: return 1080
        }
    }

    /// Display name for UI
    public var displayName: String {
        switch self {
        case .iPhoneSE: return "iPhone SE"
        case .iPhone13Mini: return "iPhone 13 mini"
        case .iPhone13Pro: return "iPhone 13 Pro"
        case .iPhone15Pro: return "iPhone 15 Pro"
        case .iPhone11: return "iPhone 11"
        case .iPhone15ProMax: return "iPhone 15 Pro Max"
        case .iPad: return "iPad"
        case .iPadPro: return "iPad Pro 12.9\""
        case .desktop1280: return "Desktop 1280"
        case .desktop1440: return "Desktop 1440"
        case .desktop1920: return "Desktop 1920"
        }
    }

    /// Screen size description
    public var sizeDescription: String {
        return "\(Int(width)) × \(Int(height))"
    }

    public var notchSpec: iPhoneNotchSpec {
        switch self {
        case .iPhone11: return .iPhone11
        case .iPhone13Mini: return .iPhone13Mini
        case .iPhone13Pro: return .iPhone13Pro
        case .iPhone15Pro: return .iPhone15Pro
        case .iPhone15ProMax: return .iPhone15ProMax
        // No notch for SE, iPad, desktop
        default: return .iPhoneSE
        }
    }

    public var size: CGSize {
        return CGSize(width: width, height: height)
    }
}

/// iPhone notch specifications for accurate system status bar simulation
public enum iPhoneNotchSpec: Sendable {
    case iPhone11           // Standard notch
    case iPhone13Mini       // Standard notch
    case iPhone13Pro        // Standard notch
    case iPhone15Pro        // Dynamic Island
    case iPhone15ProMax     // Dynamic Island (larger)
    case iPhoneSE           // No notch

    public var width: CGFloat {
        switch self {
        case .iPhone11: return 210
        case .iPhone13Mini: return 210
        case .iPhone13Pro: return 210
        case .iPhone15Pro, .iPhone15ProMax: return 126
        case .iPhoneSE: return 0
        }
    }

    public var height: CGFloat {
        switch self {
        case .iPhone11: return 30
        case .iPhone13Mini: return 30
        case .iPhone13Pro: return 30
        case .iPhone15Pro, .iPhone15ProMax: return 37
        case .iPhoneSE: return 0
        }
    }

    public var cornerRadius: CGFloat {
        switch self {
        case .iPhone11, .iPhone13Mini, .iPhone13Pro: return 15
        case .iPhone15Pro, .iPhone15ProMax: return 18.5
        case .iPhoneSE: return 0
        }
    }

    public var statusBarHeight: CGFloat {
        switch self {
        case .iPhone11: return 44
        case .iPhone13Mini: return 44
        case .iPhone13Pro: return 47
        case .iPhone15Pro, .iPhone15ProMax: return 54
        case .iPhoneSE: return 20
        }
    }
}
