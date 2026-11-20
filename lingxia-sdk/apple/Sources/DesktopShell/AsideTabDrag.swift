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

/// What a sidebar group (an lxapp main) carries while it is dragged toward
/// the main's edge: dropping it there moves the live app into the side panel
/// (the reverse of the aside tab's drop on the sidebar).
struct SidebarMainDragPayload: Codable, Equatable {
    let appId: String

    func write(to item: NSPasteboardItem) -> Bool {
        guard let data = try? JSONEncoder().encode(self) else { return false }
        return item.setData(data, forType: .lxSidebarMain)
    }

    static func read(from pasteboard: NSPasteboard) -> SidebarMainDragPayload? {
        guard let data = pasteboard.data(forType: .lxSidebarMain) else { return nil }
        return try? JSONDecoder().decode(SidebarMainDragPayload.self, from: data)
    }
}

extension NSPasteboard.PasteboardType {
    static let lxAsideTab = NSPasteboard.PasteboardType("dev.lingxia.aside-tab")
    static let lxSidebarMain = NSPasteboard.PasteboardType("dev.lingxia.sidebar-main")
}

/// The chip drawn under the pointer while a shell row is dragged.
@MainActor
enum DragSnapshot {
    static func image(of view: NSView) -> NSImage? {
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        let image = NSImage(size: view.bounds.size)
        image.addRepresentation(rep)
        return image
    }
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
            contents: DragSnapshot.image(of: source)
        )
        let session = beginDraggingSession(with: [draggingItem], event: event, source: self)
        session.animatesToStartingPositionsOnCancelOrFail = true
    }

    func draggingSession(
        _ session: NSDraggingSession,
        sourceOperationMaskFor context: NSDraggingContext
    ) -> NSDragOperation {
        context == .withinApplication ? .move : []
    }
}

/// The band along the main area's edge that takes a dragged sidebar group:
/// dropping an lxapp main there moves it into the side panel at that edge,
/// alive. Invisible and untouchable until a drag hovers it; then it tints
/// like the sidebar's aside drop target.
@MainActor
final class MainAsideDropZoneView: NSView {
    let edge: String
    var onDrop: ((SidebarMainDragPayload, String) -> Bool)?
    private var targeted = false {
        didSet {
            guard targeted != oldValue else { return }
            needsDisplay = true
        }
    }

    init(edge: String) {
        self.edge = edge
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        registerForDraggedTypes([.lxSidebarMain])
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

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

    private func accepts(_ sender: any NSDraggingInfo) -> NSDragOperation {
        let ok = onDrop != nil && SidebarMainDragPayload.read(from: sender.draggingPasteboard) != nil
        targeted = ok
        return ok ? .move : []
    }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation { accepts(sender) }
    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation { accepts(sender) }
    override func draggingExited(_ sender: (any NSDraggingInfo)?) { targeted = false }
    override func draggingEnded(_ sender: any NSDraggingInfo) { targeted = false }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        targeted = false
        guard let payload = SidebarMainDragPayload.read(from: sender.draggingPasteboard) else {
            return false
        }
        return onDrop?(payload, edge) ?? false
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
