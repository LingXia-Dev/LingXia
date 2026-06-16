import AppKit

/// Device frame view - Xcode Simulator style
/// Realistic iPhone frame with bezel, screen, and shadow
@MainActor
public class DeviceFrame: NSView {
    
    // MARK: - Layout Constants

    public struct Layout {
        // Phone device bezel (the black frame around screen)
        public static let bezelWidth: CGFloat = 4
        // Desktop/tablet: just a thin border
        public static let desktopBezelWidth: CGFloat = 1

        // Shadow
        public static let shadowRadius: CGFloat = 20
        public static let shadowOpacity: Float = 0.4
        public static let shadowOffset = CGSize(width: 0, height: -6)

        // Corner radius for device frame (outer)
        public static func frameCornerRadius(for device: MobileDeviceSize) -> CGFloat {
            device.outerRadius
        }

        // Corner radius for screen (inner, slightly smaller)
        public static func screenCornerRadius(for device: MobileDeviceSize) -> CGFloat {
            device.screenRadius
        }

        public static func bezelWidth(for device: MobileDeviceSize) -> CGFloat {
            return device.bezelWidth
        }
    }
    
    // MARK: - Properties
    
    private var deviceBezel: NSView!     // The black frame
    private var screenContainer: NSView!  // The screen area
    private var deviceSize: MobileDeviceSize = .defaultDevice
    
    /// The content view that should contain the phone screen content
    public var contentView: NSView? {
        return screenContainer
    }
    
    // MARK: - Initialization
    
    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        setupUI()
    }
    
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
    
    // MARK: - Setup
    
    private func setupUI() {
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        
        // Device bezel (the phone frame)
        deviceBezel = NSView()
        deviceBezel.wantsLayer = true
        deviceBezel.layer?.backgroundColor = NSColor(white: 0.08, alpha: 1.0).cgColor
        deviceBezel.layer?.cornerRadius = Layout.frameCornerRadius(for: deviceSize)
        deviceBezel.translatesAutoresizingMaskIntoConstraints = false
        
        // Shadow on bezel
        deviceBezel.shadow = NSShadow()
        deviceBezel.layer?.shadowColor = NSColor.black.cgColor
        deviceBezel.layer?.shadowOpacity = Layout.shadowOpacity
        deviceBezel.layer?.shadowOffset = Layout.shadowOffset
        deviceBezel.layer?.shadowRadius = Layout.shadowRadius
        
        addSubview(deviceBezel)
        
        // Screen container inside bezel
        screenContainer = NSView()
        screenContainer.wantsLayer = true
        screenContainer.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        screenContainer.layer?.cornerRadius = Layout.screenCornerRadius(for: deviceSize)
        screenContainer.layer?.masksToBounds = true
        screenContainer.translatesAutoresizingMaskIntoConstraints = false
        
        deviceBezel.addSubview(screenContainer)
        
        let bezel = Layout.bezelWidth
        
        NSLayoutConstraint.activate([
            // Bezel fills the frame
            deviceBezel.topAnchor.constraint(equalTo: topAnchor),
            deviceBezel.leadingAnchor.constraint(equalTo: leadingAnchor),
            deviceBezel.trailingAnchor.constraint(equalTo: trailingAnchor),
            deviceBezel.bottomAnchor.constraint(equalTo: bottomAnchor),
            
            // Screen inset from bezel
            screenContainer.topAnchor.constraint(equalTo: deviceBezel.topAnchor, constant: bezel),
            screenContainer.leadingAnchor.constraint(equalTo: deviceBezel.leadingAnchor, constant: bezel),
            screenContainer.trailingAnchor.constraint(equalTo: deviceBezel.trailingAnchor, constant: -bezel),
            screenContainer.bottomAnchor.constraint(equalTo: deviceBezel.bottomAnchor, constant: -bezel)
        ])
    }
    
    public override func layout() {
        super.layout()
        // Update shadow path
        let cornerRadius = Layout.frameCornerRadius(for: deviceSize)
        deviceBezel.layer?.shadowPath = CGPath(roundedRect: deviceBezel.bounds, cornerWidth: cornerRadius, cornerHeight: cornerRadius, transform: nil)
    }
    
    // MARK: - Public API
    
    /// Frame size includes the bezel (device-aware bezel width)
    public static func frameSize(for device: MobileDeviceSize) -> CGSize {
        let bezel = Layout.bezelWidth(for: device) * 2
        return CGSize(width: device.width + bezel, height: device.height + bezel)
    }

    public func setDeviceSize(_ size: MobileDeviceSize) {
        deviceSize = size

        if size.isDesktop {
            // Browser-window style: thin light border
            deviceBezel.layer?.backgroundColor = NSColor(white: 0.22, alpha: 1.0).cgColor
            deviceBezel.layer?.shadowOpacity = 0.25
        } else {
            // Phone style: dark thick bezel
            deviceBezel.layer?.backgroundColor = NSColor(white: 0.08, alpha: 1.0).cgColor
            deviceBezel.layer?.shadowOpacity = Layout.shadowOpacity
        }

        deviceBezel.layer?.cornerRadius = Layout.frameCornerRadius(for: size)
        screenContainer.layer?.cornerRadius = Layout.screenCornerRadius(for: size)

        // Re-apply bezel width for screen inset constraints
        updateBezelConstraints(for: size)

        needsLayout = true
    }

    private func updateBezelConstraints(for size: MobileDeviceSize) {
        let bezel = Layout.bezelWidth(for: size)
        // Update the screen container inset constraints
        for constraint in deviceBezel.constraints {
            guard let first = constraint.firstItem as? NSView, first === screenContainer else { continue }
            switch constraint.firstAttribute {
            case .top: constraint.constant = bezel
            case .leading: constraint.constant = bezel
            case .trailing: constraint.constant = -bezel
            case .bottom: constraint.constant = -bezel
            default: break
            }
        }
    }
}
