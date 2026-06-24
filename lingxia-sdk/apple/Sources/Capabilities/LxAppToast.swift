import SwiftUI
import Foundation
import CLingXiaSwiftAPI

/// Toast configuration
struct ToastConfig {
    let title: String
    let icon: ToastIcon
    let image: String?
    let duration: TimeInterval
    let mask: Bool

    init(
        title: String,
        icon: ToastIcon = .Success,
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
    var systemImageName: String? {
        switch self {
        case .Success:
            return "checkmark.circle.fill"
        case .Error:
            return "xmark.circle.fill"
        case .Loading:
            return "arrow.2.circlepath"
        case .None:
            return nil
        }
    }

    var color: Color {
        switch self {
        case .Success:
            return .green
        case .Error:
            return .red
        case .Loading:
            return .blue
        case .None:
            return .primary
        }
    }
}

/// Extension for ToastPosition to add UI properties
extension ToastPosition {
    var alignment: Alignment {
        switch self {
        case .Top:
            return .top
        case .Center:
            return .center
        case .Bottom:
            return .bottom
        }
    }
}

/// Main Toast API class
@MainActor
class LxAppToast {

    /// Current toast state
    @MainActor private static var currentToastTimer: Timer?
    @MainActor private static var toastOverlay: ToastOverlayManager?

    /// Show toast
    /// - Parameters:
    ///   - title: Toast message content
    ///   - icon: Icon type (success, error, loading, none)
    ///   - image: Custom local image name (overrides icon parameter)
    ///   - duration: Display duration in seconds (default: 1.5)
    ///   - mask: Whether to show transparent mask to prevent touch through
    ///   - position: Toast position (default: center)
    static func showToast(
        title: String,
        icon: ToastIcon = .Success,
        image: String? = nil,
        duration: TimeInterval = 1.5,
        mask: Bool = false,
        position: ToastPosition = .Center
    ) {
        LXLog.info("Showing toast: \(title)", category: "Toast")

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
    static func hideToast() {
        LXLog.info("Hiding toast", category: "Toast")
        currentToastTimer?.invalidate()
        currentToastTimer = nil
        toastOverlay?.hide()
        toastOverlay = nil
    }

    /// Show toast overlay using existing view hierarchy
    private static func showToastWindow(config: ToastConfig, position: ToastPosition) {
        toastOverlay = ToastOverlayManager(config: config, position: position)
        toastOverlay?.show()

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

/// Toast Overlay Manager - Uses existing view hierarchy instead of new window
@MainActor
class ToastOverlayManager {
    private let config: ToastConfig
    private let position: ToastPosition

#if os(iOS)
    private var overlayViewController: UIViewController?
    private var overlayWindow: PassthroughWindow?
#endif

    init(config: ToastConfig, position: ToastPosition) {
        self.config = config
        self.position = position
    }

    func show() {
        #if os(iOS)
        showOnIOS()
        #endif
    }

    func hide() {
        #if os(iOS)
        if let controller = overlayViewController {
            if controller.presentingViewController != nil {
                controller.dismiss(animated: false)
            } else if controller.parent != nil {
                controller.willMove(toParent: nil)
                controller.view.removeFromSuperview()
                controller.removeFromParent()
            }
            overlayViewController = nil
        }
        overlayWindow?.isHidden = true
        overlayWindow = nil
        #endif
    }

    #if os(iOS)
    private func showOnIOS() {
        guard let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = windowScene.windows.first(where: { $0.isKeyWindow }) ?? windowScene.windows.first,
              let rootViewController = window.rootViewController else {
            return
        }

        // Find the topmost view controller
        var topViewController = rootViewController
        while let presentedViewController = topViewController.presentedViewController {
            topViewController = presentedViewController
        }

        // Create overlay view controller
        let toastView = ToastContentView(config: config, position: position)
        let hostingController = UIHostingController(rootView: toastView)
        hostingController.view.backgroundColor = UIColor.clear
        hostingController.modalPresentationStyle = .overFullScreen
        hostingController.modalTransitionStyle = .crossDissolve

        if config.mask {
            overlayViewController = hostingController
            topViewController.present(hostingController, animated: true)
        } else {
            let toastWindow: PassthroughWindow
            if #available(iOS 13.0, *), let scene = window.windowScene {
                toastWindow = PassthroughWindow(windowScene: scene)
            } else {
                toastWindow = PassthroughWindow(frame: window.bounds)
            }
            toastWindow.windowLevel = window.windowLevel + 1
            toastWindow.backgroundColor = .clear
            toastWindow.rootViewController = hostingController
            toastWindow.isHidden = false
            overlayWindow = toastWindow
        }
    }
    #endif
}

#if os(iOS)
final class PassthroughWindow: UIWindow {
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        return false
    }
}
#endif

/// SwiftUI Toast Content View
struct ToastContentView: View {
    let config: ToastConfig
    let position: ToastPosition
    @State private var isVisible = false

    var body: some View {
        ZStack {
            // Background mask
            if config.mask {
                Color.black.opacity(0.3)
                    .ignoresSafeArea()
            }

            // Toast content
            VStack(spacing: config.icon == .None ? 0 : 12) {
                // Icon or custom image
                if let imagePath = config.image, !imagePath.isEmpty {
                    buildToastImage(imagePath: imagePath)
                } else if let systemImageName = config.icon.systemImageName {
                    if config.icon == .Loading {
                        LoadingIconView()
                    } else {
                        Image(systemName: systemImageName)
                            .font(.system(size: 24, weight: .medium))
                            .foregroundColor(config.icon.color)
                    }
                }

                // Title text
                Text(config.title)
                    .font(.system(size: 16, weight: .medium))
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)
                    .lineLimit(config.icon == .None ? 3 : 2)
                    .truncationMode(.tail)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, config.icon == .None ? 16 : 20)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(Color.black.opacity(0.8))
            )
            .frame(minWidth: 120, maxWidth: 280, minHeight: config.icon == .None ? 60 : 100)
            .scaleEffect(isVisible ? 1.0 : 0.8)
            .opacity(isVisible ? 1.0 : 0.0)
            .animation(.easeOut(duration: 0.2), value: isVisible)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: position.alignment)
        .allowsHitTesting(config.mask)
        .onAppear {
            isVisible = true
        }
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
            // Absolute path only
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
            // Unsupported path format - show fallback
            Image(systemName: "photo")
                .font(.title2)
                .foregroundColor(.gray)
                .frame(width: 32, height: 32)
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


}

/// Simple Loading Icon View with rotation animation
struct LoadingIconView: View {
    @State private var rotation: Double = 0

    var body: some View {
        Image(systemName: "arrow.2.circlepath")
            .font(.system(size: 24, weight: .medium))
            .foregroundColor(.blue)
            .rotationEffect(.degrees(rotation))
            .onAppear {
                withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) {
                    rotation = 360
                }
            }
    }
}

/// Toast-related errors
enum ToastError: Error, LocalizedError {
    case noWindow
    case invalidImage

    var errorDescription: String? {
        switch self {
        case .noWindow:
            return "No window available to display toast"
        case .invalidImage:
            return "Invalid image path provided"
        }
    }
}
