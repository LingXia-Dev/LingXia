import AppKit

/// Simulator-style window for Runner
/// Borderless, transparent window like Xcode Simulator
public class SimulatorWindow: NSWindow {
    
    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        // Borderless, but miniaturizable so the toolbar's yellow button can
        // programmatically minimize to the Dock (plain `.borderless` can't).
        super.init(contentRect: contentRect, styleMask: [.borderless, .miniaturizable], backing: backingStoreType, defer: flag)
        configureSimulatorStyle()
    }
    
    private func configureSimulatorStyle() {
        // Transparent background - only content is visible
        isOpaque = false
        backgroundColor = NSColor.clear
        hasShadow = false  // We'll add shadows to individual elements
        
        // Allow dragging
        isMovableByWindowBackground = true
        
        // Window level - normal
        level = .normal
        
        // Collection behavior
        collectionBehavior = [.managed, .participatesInCycle]
    }
    
    public override var canBecomeKey: Bool {
        return true
    }
    
    public override var canBecomeMain: Bool {
        return true
    }
}

/// Draggable view for window title bar area
class DraggableView: NSView {
    weak var targetWindow: NSWindow?
    
    override func mouseDown(with event: NSEvent) {
        targetWindow?.performDrag(with: event)
    }
}
