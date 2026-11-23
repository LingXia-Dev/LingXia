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
    case fold
    case pad
    case desktop
}

public enum RunnerHingeOrientation: String, Equatable, Hashable, Codable, Sendable {
    case vertical
    case horizontal
}

public enum RunnerHingePosture: String, Equatable, Hashable, Codable, Sendable {
    case flat
    case halfOpened = "half-opened"
}

public struct RunnerHingeSpec: Equatable, Hashable, Codable, Sendable {
    public let x: CGFloat
    public let y: CGFloat
    public let width: CGFloat
    public let height: CGFloat
    public let orientation: RunnerHingeOrientation
    public let posture: RunnerHingePosture

    public func scaled(by scale: CGFloat) -> RunnerHingeSpec {
        RunnerHingeSpec(
            x: x * scale,
            y: y * scale,
            width: width * scale,
            height: height * scale,
            orientation: orientation,
            posture: posture
        )
    }

    public func rotated() -> RunnerHingeSpec {
        RunnerHingeSpec(
            x: y,
            y: x,
            width: height,
            height: width,
            orientation: orientation == .vertical ? .horizontal : .vertical,
            posture: posture
        )
    }
}

public struct RunnerDevicePosture: Equatable, Hashable, Codable, Sendable {
    public let id: String
    public let label: String
    public let width: CGFloat
    public let height: CGFloat
    public let hinge: RunnerHingeSpec?
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
    public let browserProfile: RunnerBrowserProfile
    public let name: String
    public let width: CGFloat
    public let height: CGFloat
    /// The device's pixels per point: what a GPU page renders at.
    public let pixelRatio: CGFloat
    public let bezelWidth: CGFloat
    public let outerRadius: CGFloat
    public let screenRadius: CGFloat
    public let notchSpec: iPhoneNotchSpec
    /// What the device's host gives a page, in device points; nil for a shape the surface
    /// shell hosts.
    public let chrome: DeviceChrome?
    /// Optional folding postures declared for multi-posture devices.
    public let postures: [RunnerDevicePosture]?
    /// Window points per device point: below 1 once shrunk to fit a short screen. The skin
    /// draws the chrome at this; the page is told the device's own.
    public var fit: CGFloat = 1
    /// Currently active posture for a foldable device; nil for flat devices.
    public var activePosture: RunnerDevicePosture? = nil
    /// Currently active hinge geometry (in the same coordinate space as `width`/`height`).
    public var activeHinge: RunnerHingeSpec? = nil

    enum CodingKeys: String, CodingKey {
        case id
        case group
        case browserProfile
        case name
        case width
        case height
        case pixelRatio
        case bezelWidth
        case outerRadius
        case screenRadius
        case notchSpec = "notch"
        case chrome
        case postures
    }

    /// Unified adaptive breakpoint: `< 600` is compact, `>= 600` is regular.
    public static let regularWidthThreshold: CGFloat = 600

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
        // Dev override: `LINGXIA_RUNNER_DEVICE=<id>` launches at that device (e.g.
        // a Desktop size) instead of the manifest default — handy for exercising
        // the expanded/docked layout without clicking the device selector.
        if let override = ProcessInfo.processInfo.environment["LINGXIA_RUNNER_DEVICE"]?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            !override.isEmpty
        {
            let sep = override.firstIndex(where: { $0 == ":" || $0 == "@" })
            let deviceId = sep.map { String(override[..<$0]) } ?? override
            let postureId = sep.map { String(override[override.index(after: $0)...]) }
            if let matched = allCases.first(where: { $0.id == deviceId }) {
                if let postureId, !postureId.isEmpty, let posture = matched.posture(id: postureId) {
                    return matched.withPosture(posture)
                }
                return matched
            }
        }
        return device(id: manifest.default)
    }

    public var shape: RunnerDeviceShape {
        switch group {
        case "phone":
            return .phone
        case "fold", "foldable":
            return .fold
        case "tablet", "pad":
            return .pad
        case "desktop":
            return .desktop
        default:
            return .desktop
        }
    }

    /// Width in unscaled device points.
    public var deviceWidth: CGFloat {
        fit > 0 ? width / fit : width
    }

    public var isRegularWidth: Bool {
        deviceWidth >= Self.regularWidthThreshold
    }

    public var hasPostures: Bool {
        !(postures ?? []).isEmpty
    }

    /// Default posture when selecting a foldable device: prefers `"open"`, else first declared.
    public var defaultPosture: RunnerDevicePosture? {
        guard let postures, !postures.isEmpty else { return nil }
        return postures.first(where: { $0.id.caseInsensitiveCompare("open") == .orderedSame })
            ?? postures.first
    }

    public func posture(id: String) -> RunnerDevicePosture? {
        postures?.first(where: { $0.id.caseInsensitiveCompare(id) == .orderedSame })
    }

    public func withPosture(_ posture: RunnerDevicePosture?) -> MobileDeviceSize {
        guard let posture else { return self }
        return MobileDeviceSize(
            id: id,
            group: group,
            browserProfile: browserProfile,
            name: name,
            width: posture.width,
            height: posture.height,
            pixelRatio: pixelRatio,
            bezelWidth: bezelWidth,
            outerRadius: outerRadius,
            screenRadius: screenRadius,
            notchSpec: notchSpec,
            chrome: chrome,
            postures: postures,
            fit: 1,
            activePosture: posture,
            activeHinge: posture.hinge
        )
    }

    public var usesPhoneChrome: Bool {
        switch shape {
        case .phone:
            return true
        case .fold:
            return !isRegularWidth
        case .pad, .desktop:
            return false
        }
    }

    public var usesSurfaceShell: Bool {
        switch shape {
        case .phone:
            return false
        case .fold:
            return isRegularWidth
        case .pad, .desktop:
            return true
        }
    }

    public var isResizableDesktop: Bool {
        shape == .desktop
    }

    public var supportsOrientation: Bool {
        shape == .phone || shape == .fold || shape == .pad || shape == .desktop
    }

    public var orientation: RunnerDeviceOrientation {
        supportsOrientation && width > height ? .landscape : .portrait
    }

    public var displayName: String {
        name
    }

    public var orientedDisplayName: String {
        if let activePosture {
            return "\(name) \(activePosture.label)"
        }
        return supportsOrientation ? "\(name) \(orientation.displayName)" : name
    }

    public var sizeDescription: String {
        "\(Int(width)) x \(Int(height))"
    }

    public var size: CGSize {
        CGSize(width: width, height: height)
    }

    public func oriented(_ orientation: RunnerDeviceOrientation) -> MobileDeviceSize {
        guard supportsOrientation else { return self }

        let currentOrientation: RunnerDeviceOrientation = width > height ? .landscape : .portrait
        let swapsDimensions = orientation != currentOrientation
        let portraitWidth = min(width, height)
        let portraitHeight = max(width, height)
        let targetWidth = orientation == .portrait ? portraitWidth : portraitHeight
        let targetHeight = orientation == .portrait ? portraitHeight : portraitWidth
        let targetNotchSpec: iPhoneNotchSpec =
            shape == .phone && orientation == .landscape ? .landscapePhone : notchSpec
        let targetHinge = swapsDimensions ? activeHinge?.rotated() : activeHinge

        return MobileDeviceSize(
            id: id,
            group: group,
            browserProfile: browserProfile,
            name: name,
            width: targetWidth,
            height: targetHeight,
            pixelRatio: pixelRatio,
            bezelWidth: bezelWidth,
            outerRadius: outerRadius,
            screenRadius: screenRadius,
            notchSpec: targetNotchSpec,
            chrome: chrome,
            postures: postures,
            fit: fit,
            activePosture: activePosture,
            activeHinge: targetHinge
        )
    }

    /// Uniformly shrink the device so a tall phone fits a short screen while
    /// everything still lays out 1:1 (no view transform → input stays correct).
    public func scaled(by scale: CGFloat) -> MobileDeviceSize {
        guard scale < 1 else { return self }
        return MobileDeviceSize(
            id: id,
            group: group,
            browserProfile: browserProfile,
            name: name,
            // Round the size-bearing dimensions together so the frame lands on
            // whole pixels (radii are cosmetic curves — leave them exact).
            width: (width * scale).rounded(),
            height: (height * scale).rounded(),
            pixelRatio: pixelRatio,
            bezelWidth: (bezelWidth * scale).rounded(),
            outerRadius: outerRadius * scale,
            screenRadius: screenRadius * scale,
            notchSpec: notchSpec.scaled(by: scale),
            chrome: chrome,
            postures: postures,
            fit: fit * scale,
            activePosture: activePosture,
            activeHinge: activeHinge?.scaled(by: scale)
        )
    }

    /// The safe area in this orientation, in device points.
    public var safeArea: DeviceChrome.Insets {
        guard let chrome else {
            return DeviceChrome.Insets(top: notchSpec.statusBarHeight / fit, right: 0, bottom: 0, left: 0)
        }
        return orientation == .landscape ? chrome.landscapeSafeArea : chrome.portraitSafeArea
    }

    /// The host's navigation bar, in device points.
    public var navigationBarHeight: CGFloat { chrome?.navigationBar ?? DeviceChrome.iOSNavigationBar }

    /// The host's tab bar along an edge, in device points.
    public var tabBarHeight: CGFloat { chrome?.tabBar ?? DeviceChrome.iOSTabBar }

    /// Device points between the screen's bottom edge and the host's tab bar: the Android host
    /// stacks the bar on the system navigation bar; the others draw it to the edge.
    public var tabBarLift: CGFloat {
        chrome?.host == "android" ? safeArea.bottom : 0
    }

    /// Device points the host's tab bar grows by below its items: through the iOS home-indicator
    /// strip, or the HarmonyOS host's `tabBarExtraBottomHeight` (TabBar.ets).
    public var tabBarGrowth: CGFloat {
        switch chrome?.host {
        case "ios": safeArea.bottom
        case "harmony": max(0, max(12, safeArea.bottom - 6) - 12)
        default: 0
        }
    }

    /// The tab bar's reach above the screen's bottom edge, as the page measures it.
    public var tabBarReach: CGFloat { tabBarLift + tabBarHeight + tabBarGrowth }

    /// The skin draws the system bar along the bottom edge itself: on Android and HarmonyOS
    /// phones; an iPhone's home indicator is left to the page.
    public var drawsSystemBar: Bool {
        chrome?.host == "android" || chrome?.host == "harmony"
    }

    private static func device(id: String) -> MobileDeviceSize {
        guard let device = allCases.first(where: { $0.id == id }) else {
            fatalError("Missing runner device preset: \(id)")
        }
        return device
    }

    private static let manifest: RunnerDevicesManifest = {
        // `lingxia build` merges the Runner's resources into the main bundle.
        guard let url = Bundle.main.url(forResource: "devices", withExtension: "json") else {
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

/// A device's system insets and its host's bars, in device points, as the host app on that
/// device gives them to a page. The bars are the host's own constants: iOS `LxAppTheme` and the
/// Apple FFI's tab bar; Android `LxAppActivity` and its FFI; HarmonyOS `NavigationBar.ets` and
/// its FFI. `provisional` in the JSON names the device metrics not yet read off the device.
public struct DeviceChrome: Equatable, Hashable, Decodable, Sendable {
    public struct Insets: Equatable, Hashable, Sendable {
        public let top: CGFloat
        public let right: CGFloat
        public let bottom: CGFloat
        public let left: CGFloat
    }

    private struct SafeAreas: Equatable, Hashable, Decodable, Sendable {
        /// `[top, right, bottom, left]`.
        let portrait: [CGFloat]
        let landscape: [CGFloat]
    }

    static let iOSNavigationBar: CGFloat = 44
    static let iOSTabBar: CGFloat = 64

    /// The platform whose host the bars follow: `ios`, `android` or `harmony`.
    public let host: String
    private let safeArea: SafeAreas
    public let navigationBar: CGFloat
    public let tabBar: CGFloat

    public var portraitSafeArea: Insets { Self.insets(safeArea.portrait) }
    public var landscapeSafeArea: Insets { Self.insets(safeArea.landscape) }

    private static func insets(_ edges: [CGFloat]) -> Insets {
        let edge = { (index: Int) in index < edges.count ? edges[index] : 0 }
        return Insets(top: edge(0), right: edge(1), bottom: edge(2), left: edge(3))
    }
}

/// iPhone notch specifications for accurate system status bar simulation.
public struct iPhoneNotchSpec: Equatable, Hashable, Decodable, Sendable {
    public let width: CGFloat
    public let height: CGFloat
    public let cornerRadius: CGFloat
    public let statusBarHeight: CGFloat

    public func scaled(by scale: CGFloat) -> iPhoneNotchSpec {
        iPhoneNotchSpec(
            width: width * scale,
            height: height * scale,
            cornerRadius: cornerRadius * scale,
            statusBarHeight: statusBarHeight * scale
        )
    }

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
