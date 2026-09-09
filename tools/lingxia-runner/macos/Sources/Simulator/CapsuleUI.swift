import AppKit
@_spi(Runner) import lingxia

/// Colors for the runner's capsule click sheet. Matches the iOS overlay palette
/// (`LxAppAppearanceRegistry.overlayColors`) against the simulated host scheme.
@MainActor
struct CapsuleOverlayPalette {
    let scrim: NSColor
    let surface: NSColor
    let title: NSColor
    let secondary: NSColor
    let separator: NSColor
    let icon: NSColor

    static func current() -> CapsuleOverlayPalette {
        let dark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        if dark {
            return CapsuleOverlayPalette(
                scrim: NSColor.black.withAlphaComponent(0.55),
                surface: NSColor(srgbRed: 0.11, green: 0.11, blue: 0.12, alpha: 1),
                title: .white,
                secondary: NSColor(white: 0.63, alpha: 1),
                separator: NSColor(white: 1, alpha: 0.12),
                icon: NSColor(white: 0.90, alpha: 1)
            )
        }
        return CapsuleOverlayPalette(
            scrim: NSColor.black.withAlphaComponent(0.4),
            surface: .white,
            title: .black,
            secondary: NSColor(white: 0.60, alpha: 1),
            separator: NSColor(srgbRed: 0.93, green: 0.93, blue: 0.93, alpha: 1),
            icon: NSColor(srgbRed: 0.20, green: 0.20, blue: 0.20, alpha: 1)
        )
    }
}

/// Capsule button images for Runner - uses SDK's LxIcon to load PDF icons
@MainActor
public struct CapsuleButtonImages {

    /// Get capsule menu icon (three dots)
    public static func createThreeDotsImage() -> NSImage? {
        templateImage(named: "icon_capsule_menu", size: CGSize(width: 20, height: 14))
    }

    /// Get capsule close icon
    public static func createCloseButtonImage() -> NSImage? {
        templateImage(named: "icon_capsule_close", size: CGSize(width: 20, height: 14))
    }

    private static func templateImage(named name: String, size: CGSize) -> NSImage? {
        guard let source = RunnerSupport.Assets.image(named: name, size: size) else {
            return nil
        }
        let image = (source.copy() as? NSImage) ?? source
        image.isTemplate = true
        return image
    }
    
    
    /// Get minimize button image (drawn manually as there's no SVG for this)
    public static func createMinimizeButtonImage() -> NSImage {
        let size = CGSize(width: 24, height: 24)
        let image = NSImage(size: size)
        image.lockFocus()
        
        if let context = NSGraphicsContext.current?.cgContext {
            context.setShouldAntialias(true)
            context.setLineWidth(2.5)
            context.setLineCap(.round)
            context.setStrokeColor(NSColor.black.cgColor)
            
            let lineWidth: CGFloat = 10
            context.move(to: CGPoint(x: (size.width - lineWidth) / 2, y: size.height / 2))
            context.addLine(to: CGPoint(x: (size.width + lineWidth) / 2, y: size.height / 2))
            context.strokePath()
        }
        
        image.unlockFocus()
        return image
    }
    
    /// Get back button image
    public static func createBackButtonImage(color: NSColor = .black) -> NSImage? {
        guard let image = RunnerSupport.Assets.image(named: "icon_back", size: CGSize(width: 24, height: 24)) else {
            return nil
        }
        return tintImage(image, color: color)
    }
    
    /// Get home button image
    public static func createHomeButtonImage(color: NSColor = .black) -> NSImage? {
        guard let image = RunnerSupport.Assets.image(named: "icon_home", size: CGSize(width: 24, height: 24)) else {
            return nil
        }
        return tintImage(image, color: color)
    }
    
    /// Tint a template image with specific color
    private static func tintImage(_ image: NSImage, color: NSColor) -> NSImage {
        let tinted = NSImage(size: image.size)
        tinted.lockFocus()
        color.set()
        let rect = NSRect(origin: .zero, size: image.size)
        image.draw(in: rect, from: rect, operation: .sourceOver, fraction: 1.0)
        rect.fill(using: .sourceAtop)
        tinted.unlockFocus()
        tinted.isTemplate = false
        return tinted
    }
}
