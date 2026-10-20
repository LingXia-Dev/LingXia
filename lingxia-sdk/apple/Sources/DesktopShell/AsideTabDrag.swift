#if os(macOS)
import AppKit

/// What an aside tab carries while it is dragged onto the sidebar.
struct AsideTabDragPayload: Codable, Equatable {
    enum Kind: String, Codable {
        case lxapp
        case browser
    }

    let kind: Kind
    let surfaceId: String

    func write(to item: NSPasteboardItem) -> Bool {
        guard let data = try? JSONEncoder().encode(self) else { return false }
        return item.setData(data, forType: .lxAsideTab)
    }

    static func read(from pasteboard: NSPasteboard) -> AsideTabDragPayload? {
        guard let data = pasteboard.data(forType: .lxAsideTab) else { return nil }
        return try? JSONDecoder().decode(AsideTabDragPayload.self, from: data)
    }
}

extension NSPasteboard.PasteboardType {
    static let lxAsideTab = NSPasteboard.PasteboardType("dev.lingxia.aside-tab")
}

/// Title button of an aside tab chip. A click still selects the tab; moving
/// past the threshold with the button held starts a drag instead.
@MainActor
final class AsideTabTitleButton: NSButton, NSDraggingSource {
    private static let dragThreshold: CGFloat = 4

    var dragPayload: AsideTabDragPayload?
    /// The chip drawn under the pointer while dragging; the button by default.
    weak var dragImageSource: NSView?

    override func mouseDown(with event: NSEvent) {
        guard dragPayload != nil, isEnabled, let window else {
            super.mouseDown(with: event)
            return
        }
        let origin = event.locationInWindow
        while let next = window.nextEvent(matching: [.leftMouseDragged, .leftMouseUp]) {
            if next.type == .leftMouseUp {
                sendAction(action, to: target)
                return
            }
            let point = next.locationInWindow
            if hypot(point.x - origin.x, point.y - origin.y) >= Self.dragThreshold {
                beginDrag(with: next)
                return
            }
        }
    }

    private func beginDrag(with event: NSEvent) {
        let item = NSPasteboardItem()
        guard let dragPayload, dragPayload.write(to: item) else { return }
        let source = dragImageSource ?? self
        let draggingItem = NSDraggingItem(pasteboardWriter: item)
        draggingItem.setDraggingFrame(
            source.convert(source.bounds, to: self),
            contents: Self.snapshot(of: source)
        )
        let session = beginDraggingSession(with: [draggingItem], event: event, source: self)
        session.animatesToStartingPositionsOnCancelOrFail = true
    }

    private static func snapshot(of view: NSView) -> NSImage? {
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        let image = NSImage(size: view.bounds.size)
        image.addRepresentation(rep)
        return image
    }

    func draggingSession(
        _ session: NSDraggingSession,
        sourceOperationMaskFor context: NSDraggingContext
    ) -> NSDragOperation {
        context == .withinApplication ? .move : []
    }
}

/// Tint laid over the sidebar while an aside tab hovers it. Purely visual:
/// the sidebar itself is the drag destination.
@MainActor
final class SidebarAsideDropHighlightView: NSView {
    var targeted = false {
        didSet {
            guard targeted != oldValue else { return }
            needsDisplay = true
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func draw(_ dirtyRect: NSRect) {
        guard targeted else { return }
        let shape = NSBezierPath(roundedRect: bounds.insetBy(dx: 4, dy: 4), xRadius: 8, yRadius: 8)
        LxAppHostTheme.accent.withAlphaComponent(0.12).setFill()
        shape.fill()
        LxAppHostTheme.accent.withAlphaComponent(0.6).setStroke()
        shape.lineWidth = 1.5
        shape.stroke()
    }
}
#endif
