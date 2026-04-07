import AppKit
import lingxia

/// Capsule button images for Runner - uses SDK's LxIcon to load PDF icons
@MainActor
public struct CapsuleButtonImages {
    
    /// Get capsule menu icon (three dots)
    public static func createThreeDotsImage() -> NSImage? {
        guard let image = RunnerSupport.Assets.image(named: "icon_capsule_menu", size: CGSize(width: 20, height: 14)) else {
            return nil
        }
        // Disable template mode to show original black color
        image.isTemplate = false
        return image
    }
    
    /// Get capsule close icon
    public static func createCloseButtonImage() -> NSImage? {
        guard let image = RunnerSupport.Assets.image(named: "icon_capsule_close", size: CGSize(width: 20, height: 14)) else {
            return nil
        }
        // Disable template mode to show original black color
        image.isTemplate = false
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
