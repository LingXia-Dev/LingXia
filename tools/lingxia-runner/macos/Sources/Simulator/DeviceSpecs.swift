import CoreGraphics
import Foundation

private struct RunnerDevicesManifest: Decodable {
    let `default`: String
    let devices: [MobileDeviceSize]
}

/// High-level runner host shape. This is intentionally not a raw mirror of the
/// manifest group: the UI host changes by shape, not by arbitrary device ids.
public enum RunnerDeviceShape: Equatable, Hashable, Sendable {
    case phone
    case pad
    case desktop
}

public enum RunnerDeviceOrientation: String, Equatable, Hashable, CaseIterable, Sendable {
    case portrait
    case landscape

    public var displayName: String {
        switch self {
        case .portrait: return "Portrait"
        case .landscape: return "Landscape"
        }
    }

    public var systemImageName: String {
        switch self {
        case .portrait: return "rectangle.portrait"
        case .landscape: return "rectangle"
        }
    }

    public var toggled: RunnerDeviceOrientation {
        switch self {
        case .portrait: return .landscape
        case .landscape: return .portrait
        }
    }
}

/// Predefined device sizes for Runner window sizing.
public struct MobileDeviceSize: Equatable, Hashable, Decodable, Sendable {
    public let id: String
    public let group: String
    public let name: String
    public let width: CGFloat
    public let height: CGFloat
    public let bezelWidth: CGFloat
    public let outerRadius: CGFloat
    public let screenRadius: CGFloat
    public let notchSpec: iPhoneNotchSpec

    enum CodingKeys: String, CodingKey {
        case id
        case group
        case name
        case width
        case height
        case bezelWidth
        case outerRadius
        case screenRadius
        case notchSpec = "notch"
    }

    public static var iPhoneSE: MobileDeviceSize { device(id: "iphone-se") }
    public static var iPhone13Mini: MobileDeviceSize { device(id: "iphone-13-mini") }
    public static var iPhone13Pro: MobileDeviceSize { device(id: "iphone-13-pro") }
    public static var iPhone15Pro: MobileDeviceSize { device(id: "iphone-15-pro") }
    public static var iPhone11: MobileDeviceSize { device(id: "iphone-11") }
    public static var iPhone15ProMax: MobileDeviceSize { device(id: "iphone-15-pro-max") }

    public static var allCases: [MobileDeviceSize] {
        manifest.devices
    }

    public static var defaultDevice: MobileDeviceSize {
        device(id: manifest.default)
    }

    public var shape: RunnerDeviceShape {
        switch group {
        case "phone":
            return .phone
        case "tablet", "pad":
            return .pad
        case "desktop":
            return .desktop
        default:
            return .desktop
        }
    }

    public var usesPhoneChrome: Bool {
        shape == .phone
    }

    public var usesSurfaceShell: Bool {
        shape == .pad || shape == .desktop
    }

    public var isResizableDesktop: Bool {
        shape == .desktop
    }

    public var supportsOrientation: Bool {
        shape == .phone || shape == .pad
    }

    public var orientation: RunnerDeviceOrientation {
        supportsOrientation && width > height ? .landscape : .portrait
    }

    public var displayName: String {
        name
    }

    public var orientedDisplayName: String {
        supportsOrientation ? "\(name) \(orientation.displayName)" : name
    }

    public var sizeDescription: String {
        "\(Int(width)) x \(Int(height))"
    }

    public var size: CGSize {
        CGSize(width: width, height: height)
    }

    public func oriented(_ orientation: RunnerDeviceOrientation) -> MobileDeviceSize {
        guard supportsOrientation else { return self }

        let portraitWidth = min(width, height)
        let portraitHeight = max(width, height)
        let targetWidth = orientation == .portrait ? portraitWidth : portraitHeight
        let targetHeight = orientation == .portrait ? portraitHeight : portraitWidth
        let targetNotchSpec: iPhoneNotchSpec =
            shape == .phone && orientation == .landscape ? .landscapePhone : notchSpec

        return MobileDeviceSize(
            id: id,
            group: group,
            name: name,
            width: targetWidth,
            height: targetHeight,
            bezelWidth: bezelWidth,
            outerRadius: outerRadius,
            screenRadius: screenRadius,
            notchSpec: targetNotchSpec
        )
    }

    private static func device(id: String) -> MobileDeviceSize {
        guard let device = allCases.first(where: { $0.id == id }) else {
            fatalError("Missing runner device preset: \(id)")
        }
        return device
    }

    private static let manifest: RunnerDevicesManifest = {
        let url = Bundle.module.url(forResource: "devices", withExtension: "json")
            ?? Bundle.module.url(
                forResource: "devices",
                withExtension: "json",
                subdirectory: "Resources"
            )
        guard let url else {
            fatalError("Missing runner devices.json resource")
        }
        do {
            let data = try Data(contentsOf: url)
            return try JSONDecoder().decode(RunnerDevicesManifest.self, from: data)
        } catch {
            fatalError("Invalid runner devices.json: \(error)")
        }
    }()
}

/// iPhone notch specifications for accurate system status bar simulation.
public struct iPhoneNotchSpec: Equatable, Hashable, Decodable, Sendable {
    public let width: CGFloat
    public let height: CGFloat
    public let cornerRadius: CGFloat
    public let statusBarHeight: CGFloat

    public static let iPhoneSE = iPhoneNotchSpec(
        width: 0,
        height: 0,
        cornerRadius: 0,
        statusBarHeight: 20
    )

    public static let landscapePhone = iPhoneNotchSpec(
        width: 0,
        height: 0,
        cornerRadius: 0,
        statusBarHeight: 0
    )
}
