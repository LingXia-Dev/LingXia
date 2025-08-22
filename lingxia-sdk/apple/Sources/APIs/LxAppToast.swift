import SwiftUI
import Foundation
import os.log

/// Toast icon types
public enum ToastIcon {
    case success
    case error
    case loading
    case none
}

/// Toast configuration
public struct ToastConfig {
    public let title: String
    public let icon: ToastIcon
    public let image: String?
    public let duration: TimeInterval
    public let mask: Bool

    public init(
        title: String,
        icon: ToastIcon = .success,
        image: String? = nil,
        duration: TimeInterval = 1.5,
        mask: Bool = false
    ) {
        self.title = title
        self.icon = icon
        self.image = image
        self.duration = duration
        self.mask = mask
    }
}

/// Extension for ToastIcon to add UI properties
extension ToastIcon {
    public var systemImageName: String? {
        switch self {
        case .success:
            return "checkmark.circle.fill"
        case .error:
            return "xmark.circle.fill"
        case .loading:
            return "arrow.2.circlepath"
        case .none:
            return nil
        }
    }

    public var color: Color {
        switch self {
        case .success:
            return .green
        case .error:
            return .red
        case .loading:
            return .blue
        case .none:
            return .primary
        }
    }

    /// Convert from string
    public static func fromString(_ string: String) -> ToastIcon {
        switch string.lowercased() {
        case "success":
            return .success
        case "error":
            return .error
        case "loading":
            return .loading
        default:
            return .none
        }
    }

    /// Convert from int (FFI interface)
    public static func fromInt(_ value: Int) -> ToastIcon {
        switch value {
        case 0: return .success
        case 1: return .error
        case 2: return .loading
        default: return .none
        }
    }
}

/// Toast position options
public enum ToastPosition {
    case top
    case center
    case bottom

    public var alignment: Alignment {
        switch self {
        case .top:
            return .top
        case .center:
            return .center
        case .bottom:
            return .bottom
        }
    }

    /// Create position from string
    public static func fromString(_ string: String) -> ToastPosition {
        switch string.lowercased() {
        case "top":
            return .top
        case "bottom":
            return .bottom
        default:
            return .center
        }
    }
}

/// Main Toast API class
@MainActor
public class LxAppToast {

    private static let log = OSLog(subsystem: "LingXia", category: "Toast")

    /// Current toast state
    @MainActor private static var currentToastTimer: Timer?
    @MainActor private static var toastWindow: ToastWindow?

    /// Show toast
    /// - Parameters:
    ///   - title: Toast message content
    ///   - icon: Icon type (success, error, loading, none)
    ///   - image: Custom local image name (overrides icon parameter)
    ///   - duration: Display duration in seconds (default: 1.5)
    ///   - mask: Whether to show transparent mask to prevent touch through
    ///   - position: Toast position (default: center)
    public static func showToast(
        title: String,
        icon: ToastIcon = .success,
        image: String? = nil,
        duration: TimeInterval = 1.5,
        mask: Bool = false,
        position: ToastPosition = .center
    ) {
        os_log(.info, log: log, "Showing toast: %{public}@", title)

        // Hide any existing toast first
        hideToast()

        let config = ToastConfig(
            title: title,
            icon: icon,
            image: image,
            duration: duration,
            mask: mask
        )

        showToastWindow(config: config, position: position)
    }

    /// Hide current toast immediately
    public static func hideToast() {
        os_log(.info, log: log, "Hiding toast")
        currentToastTimer?.invalidate()
        currentToastTimer = nil
        toastWindow?.hide()
        toastWindow = nil
    }

    /// Dictionary-based API for JavaScript compatibility
    public static func showToast(_ options: [String: Any]) {
        let title = options["title"] as? String ?? ""
        let iconString = options["icon"] as? String ?? "success"
        let icon = ToastIcon.fromString(iconString)
        let image = options["image"] as? String
        let duration = options["duration"] as? TimeInterval ?? 1.5
        let mask = options["mask"] as? Bool ?? false
        let positionString = options["position"] as? String ?? "center"
        let position = ToastPosition.fromString(positionString)

        showToast(
            title: title,
            icon: icon,
            image: image,
            duration: duration,
            mask: mask,
            position: position
        )
    }

    /// Show toast window using pure SwiftUI
    private static func showToastWindow(config: ToastConfig, position: ToastPosition) {
        toastWindow = ToastWindow(config: config, position: position)
        toastWindow?.show()

        // Auto-hide after duration
        if config.duration > 0 {
            currentToastTimer = Timer.scheduledTimer(withTimeInterval: config.duration, repeats: false) { _ in
                Task { @MainActor in
                    hideToast()
                }
            }
        }
    }
}

/// Pure SwiftUI Toast Window
@MainActor
class ToastWindow {
    private let config: ToastConfig
    private let position: ToastPosition

    #if os(iOS)
    private var window: UIWindow?
    #elseif os(macOS)
    private var window: NSWindow?
    #endif

    init(config: ToastConfig, position: ToastPosition) {
        self.config = config
        self.position = position
    }

    func show() {
        #if os(iOS)
        showOnIOS()
        #elseif os(macOS)
        showOnMacOS()
        #endif
    }

    func hide() {
        #if os(iOS)
        window?.isHidden = true
        window = nil
        #elseif os(macOS)
        window?.orderOut(nil)
        window = nil
        #endif
    }

    #if os(iOS)
    private func showOnIOS() {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { return }

        window = UIWindow(windowScene: windowScene)
        window?.windowLevel = UIWindow.Level.alert + 1
        window?.backgroundColor = UIColor.clear
        window?.isHidden = false

        let hostingController = UIHostingController(rootView: ToastContentView(config: config, position: position))
        hostingController.view.backgroundColor = UIColor.clear
        window?.rootViewController = hostingController
    }
    #endif

    #if os(macOS)
    private func showOnMacOS() {
        guard let mainWindow = NSApplication.shared.mainWindow else { return }

        let contentView = ToastContentView(config: config, position: position)
        let hostingView = NSHostingView(rootView: contentView)

        window = NSWindow(
            contentRect: mainWindow.frame,
            styleMask: [],
            backing: .buffered,
            defer: false
        )

        window?.level = .floating
        window?.backgroundColor = NSColor.clear
        window?.isOpaque = false
        window?.hasShadow = false
        window?.ignoresMouseEvents = !config.mask
        window?.contentView = hostingView
        window?.orderFront(nil)
    }
    #endif
}

/// SwiftUI Toast Content View
struct ToastContentView: View {
    let config: ToastConfig
    let position: ToastPosition

    var body: some View {
        ZStack {
            // Background mask
            if config.mask {
                Color.black.opacity(0.3)
                    .ignoresSafeArea()
            }

            // Toast content
            VStack(spacing: 8) {
                // Icon or custom image
                if let imagePath = config.image, !imagePath.isEmpty {
                    buildToastImage(imagePath: imagePath)
                } else if let systemImageName = config.icon.systemImageName {
                    Image(systemName: systemImageName)
                        .font(.title2)
                        .foregroundColor(config.icon.color)
                        .rotationEffect(config.icon == .loading ? .degrees(0) : .degrees(0))
                        .animation(
                            config.icon == .loading ?
                            Animation.linear(duration: 1).repeatForever(autoreverses: false) :
                            nil,
                            value: config.icon == .loading
                        )
                }

                // Title text
                Text(config.title)
                    .font(.system(size: 16, weight: .medium))
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)
                    .lineLimit(config.icon == .none ? 2 : 1)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 16)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(Color.black.opacity(0.8))
            )
            .frame(minWidth: 120, maxWidth: 200)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: position.alignment)
        .allowsHitTesting(config.mask)
    }

    /// Build toast image with full path support
    @ViewBuilder
    private func buildToastImage(imagePath: String) -> some View {
        if imagePath.hasPrefix("SF:") {
            // System SF Symbol
            let symbolName = String(imagePath.dropFirst(3))
            Image(systemName: symbolName)
                .font(.title2)
                .foregroundColor(.white)
                .frame(width: 32, height: 32)
        } else if imagePath.hasPrefix("/") {
            // Absolute path
            if let image = loadPlatformImage(from: imagePath) {
                image
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 32, height: 32)
            } else {
                // Fallback to default icon
                Image(systemName: "photo")
                    .font(.title2)
                    .foregroundColor(.gray)
                    .frame(width: 32, height: 32)
            }
        } else {
            // Try bundle first
            if let bundleImage = loadBundleImage(named: imagePath) {
                bundleImage
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 32, height: 32)
            } else {
                // Fallback to bundle name only
                Image(imagePath)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 32, height: 32)
            }
        }
    }

    /// Load platform-specific image from path
    private func loadPlatformImage(from path: String) -> Image? {
        #if os(iOS)
        if let uiImage = UIImage(contentsOfFile: path) {
            return Image(uiImage: uiImage)
        }
        #else
        if let nsImage = NSImage(contentsOfFile: path) {
            return Image(nsImage: nsImage)
        }
        #endif
        return nil
    }

    /// Load platform-specific image from bundle
    private func loadBundleImage(named name: String) -> Image? {
        #if os(iOS)
        if let uiImage = UIImage(named: name) {
            return Image(uiImage: uiImage)
        }
        #else
        if let nsImage = NSImage(named: name) {
            return Image(nsImage: nsImage)
        }
        #endif
        return nil
    }
}

/// Toast-related errors
public enum ToastError: Error, LocalizedError {
    case noWindow
    case invalidImage

    public var errorDescription: String? {
        switch self {
        case .noWindow:
            return "No window available to display toast"
        case .invalidImage:
            return "Invalid image path provided"
        }
    }
}
