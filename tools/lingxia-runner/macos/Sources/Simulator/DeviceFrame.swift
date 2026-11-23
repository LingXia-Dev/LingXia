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
    private var hingeOverlay: RunnerHingeOverlayView!
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
        
        // Device bezel (the phone frame). No drop shadow: on the transparent
        // simulator window the shadow renders as a gray "ghost" hugging the
        // rounded corners, so the device is drawn flat.
        deviceBezel = NSView()
        deviceBezel.wantsLayer = true
        deviceBezel.layer?.backgroundColor = NSColor(white: 0.08, alpha: 1.0).cgColor
        deviceBezel.layer?.cornerRadius = Layout.frameCornerRadius(for: deviceSize)
        deviceBezel.translatesAutoresizingMaskIntoConstraints = false

        addSubview(deviceBezel)
        
        // Screen container inside bezel
        screenContainer = NSView()
        screenContainer.wantsLayer = true
        screenContainer.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        screenContainer.layer?.cornerRadius = Layout.screenCornerRadius(for: deviceSize)
        screenContainer.layer?.masksToBounds = true
        screenContainer.translatesAutoresizingMaskIntoConstraints = false
        
        deviceBezel.addSubview(screenContainer)

        hingeOverlay = RunnerHingeOverlayView()
        hingeOverlay.translatesAutoresizingMaskIntoConstraints = false
        screenContainer.addSubview(hingeOverlay)
        
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
            screenContainer.bottomAnchor.constraint(equalTo: deviceBezel.bottomAnchor, constant: -bezel),

            hingeOverlay.topAnchor.constraint(equalTo: screenContainer.topAnchor),
            hingeOverlay.leadingAnchor.constraint(equalTo: screenContainer.leadingAnchor),
            hingeOverlay.trailingAnchor.constraint(equalTo: screenContainer.trailingAnchor),
            hingeOverlay.bottomAnchor.constraint(equalTo: screenContainer.bottomAnchor)
        ])
    }

    public override func layout() {
        super.layout()
        // Keep the optical hinge above any content added into screenContainer.
        if let hingeOverlay, screenContainer.subviews.last !== hingeOverlay {
            screenContainer.addSubview(hingeOverlay, positioned: .above, relativeTo: nil)
        }
    }
    
    // MARK: - Public API
    
    /// Frame size includes the bezel (device-aware bezel width)
    public static func frameSize(for device: MobileDeviceSize) -> CGSize {
        let bezel = Layout.bezelWidth(for: device) * 2
        return CGSize(width: device.width + bezel, height: device.height + bezel)
    }

    public func setDeviceSize(_ size: MobileDeviceSize) {
        deviceSize = size

        if !size.usesPhoneChrome {
            // Browser-window style: thin light border
            deviceBezel.layer?.backgroundColor = NSColor(white: 0.22, alpha: 1.0).cgColor
        } else {
            // Phone style: dark thick bezel
            deviceBezel.layer?.backgroundColor = NSColor(white: 0.08, alpha: 1.0).cgColor
        }

        deviceBezel.layer?.cornerRadius = Layout.frameCornerRadius(for: size)
        screenContainer.layer?.cornerRadius = Layout.screenCornerRadius(for: size)

        // Re-apply bezel width for screen inset constraints
        updateBezelConstraints(for: size)
        hingeOverlay?.setHinge(size.activeHinge)

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

/// Subtle, non-interactive optical hinge crease rendered over foldable screens.
@MainActor
final class RunnerHingeOverlayView: NSView {
    private(set) var hinge: RunnerHingeSpec?

    override var isFlipped: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        layer?.backgroundColor = NSColor.clear.cgColor
        isHidden = true
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    // Never intercept pointer or scroll events from the simulated app.
    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    func setHinge(_ hinge: RunnerHingeSpec?) {
        self.hinge = hinge
        isHidden = hinge == nil
        needsDisplay = true
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let hinge, let context = NSGraphicsContext.current?.cgContext else { return }

        let isDark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let isHalfOpened = hinge.posture == .halfOpened
        let shadowAlpha: CGFloat = isHalfOpened ? (isDark ? 0.32 : 0.20) : (isDark ? 0.20 : 0.11)
        let highlightAlpha: CGFloat = isHalfOpened ? (isDark ? 0.16 : 0.24) : (isDark ? 0.10 : 0.16)
        let lineAlpha: CGFloat = isHalfOpened ? (isDark ? 0.28 : 0.20) : (isDark ? 0.14 : 0.10)

        let shadowColor = NSColor.black.withAlphaComponent(shadowAlpha)
        let highlightColor = NSColor.white.withAlphaComponent(highlightAlpha)
        let clearColor = NSColor.clear

        guard let gradient = NSGradient(
            colorsAndLocations:
                (clearColor, 0.0),
                (shadowColor, 0.35),
                (highlightColor, 0.5),
                (shadowColor, 0.65),
                (clearColor, 1.0)
        ) else { return }

        switch hinge.orientation {
        case .vertical:
            let centerX = hinge.x + hinge.width / 2
            let bandWidth = max(hinge.width, isHalfOpened ? 14 : 10)
            let bandRect = NSRect(
                x: centerX - bandWidth / 2,
                y: hinge.y,
                width: bandWidth,
                height: hinge.height > 0 ? hinge.height : bounds.height
            )
            gradient.draw(in: bandRect, angle: 0)

            context.saveGState()
            context.setStrokeColor(
                (isDark ? NSColor.white : NSColor.black).withAlphaComponent(lineAlpha).cgColor
            )
            context.setLineWidth(isHalfOpened ? 1.0 : 0.5)
            if isHalfOpened {
                context.setLineDash(phase: 0, lengths: [4, 3])
            }
            context.move(to: CGPoint(x: centerX, y: bandRect.minY))
            context.addLine(to: CGPoint(x: centerX, y: bandRect.maxY))
            context.strokePath()
            context.restoreGState()

        case .horizontal:
            let centerY = hinge.y + hinge.height / 2
            let bandHeight = max(hinge.height, isHalfOpened ? 16 : 10)
            let bandRect = NSRect(
                x: hinge.x,
                y: centerY - bandHeight / 2,
                width: hinge.width > 0 ? hinge.width : bounds.width,
                height: bandHeight
            )
            gradient.draw(in: bandRect, angle: 90)

            context.saveGState()
            context.setStrokeColor(
                (isDark ? NSColor.white : NSColor.black).withAlphaComponent(lineAlpha).cgColor
            )
            context.setLineWidth(isHalfOpened ? 1.0 : 0.5)
            if isHalfOpened {
                context.setLineDash(phase: 0, lengths: [4, 3])
            }
            context.move(to: CGPoint(x: bandRect.minX, y: centerY))
            context.addLine(to: CGPoint(x: bandRect.maxX, y: centerY))
            context.strokePath()
            context.restoreGState()
        }
    }
}
