#if os(macOS)
import AppKit

/// The sidebar's expanded layout floated over the workspace from a committed
/// rail. Only the sidebar view widens — its own width replaces the one tying
/// it to its slot — so the workspace, docked panels and reported surface
/// metrics never see the peek.
@MainActor
final class SidebarPeekPresentation {
    private static let slideDuration: TimeInterval = 0.15

    private weak var window: NSWindow?
    private weak var container: NSView?
    private weak var sidebar: SidebarView?
    private let widthConstraint: NSLayoutConstraint
    private let followsSlot: NSLayoutConstraint
    private let backdrop = LxAppHostThemeLayerView(role: .windowBackground)
    /// The container's stacking before the peek lifted the sidebar.
    private var savedOrder: [ObjectIdentifier: Int] = [:]
    private var monitors: [Any] = []
    private var resignObserver: NSObjectProtocol?
    private(set) var isDismissing = false
    private var isPinned = false

    var onDismissRequested: (() -> Void)?

    init(
        window: NSWindow, container: NSView, sidebar: SidebarView,
        widthConstraint: NSLayoutConstraint, followsSlot: NSLayoutConstraint
    ) {
        self.window = window
        self.container = container
        self.sidebar = sidebar
        self.widthConstraint = widthConstraint
        self.followsSlot = followsSlot
    }

    func present(from collapsedWidth: CGFloat, to expandedWidth: CGFloat) {
        guard let container, let sidebar else { return }
        sidebar.setPeeking(true)

        backdrop.translatesAutoresizingMaskIntoConstraints = false
        backdrop.layer?.shadowColor = NSColor.black.cgColor
        backdrop.layer?.shadowOpacity = 0.18
        backdrop.layer?.shadowRadius = 10
        backdrop.layer?.shadowOffset = CGSize(width: 2, height: 0)
        container.addSubview(backdrop)
        NSLayoutConstraint.activate([
            backdrop.topAnchor.constraint(equalTo: sidebar.topAnchor),
            backdrop.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
            backdrop.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
            backdrop.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor),
        ])
        liftAboveWorkspace(container: container, sidebar: sidebar)

        widthConstraint.constant = collapsedWidth
        followsSlot.isActive = false
        widthConstraint.isActive = true
        container.layoutSubtreeIfNeeded()
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            widthConstraint.constant = expandedWidth
        } else {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = Self.slideDuration
                context.timingFunction = CAMediaTimingFunction(name: .easeOut)
                context.allowsImplicitAnimation = true
                widthConstraint.animator().constant = expandedWidth
                container.layoutSubtreeIfNeeded()
            }
        }
        installDismissTriggers()
    }

    /// The peek became the pinned sidebar: it keeps its width and shadow until
    /// the shell finishes moving the workspace, then `dismiss` tears it down.
    func finishAsPin() {
        isPinned = true
        removeDismissTriggers()
        sidebar?.setPeeking(false)
    }

    func dismiss(to collapsedWidth: CGFloat, animated: Bool, completion: @escaping () -> Void) {
        isDismissing = true
        removeDismissTriggers()
        let animate = animated && !isPinned
            && !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        guard animate, let container else {
            tearDown()
            completion()
            return
        }
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = Self.slideDuration
            context.timingFunction = CAMediaTimingFunction(name: .easeIn)
            context.allowsImplicitAnimation = true
            widthConstraint.animator().constant = collapsedWidth
            container.layoutSubtreeIfNeeded()
        }, completionHandler: {
            MainActor.assumeIsolated {
                self.tearDown()
                completion()
            }
        })
    }

    private func tearDown() {
        widthConstraint.isActive = false
        followsSlot.isActive = true
        sidebar?.setPeeking(false)
        backdrop.removeFromSuperview()
        if let container { restoreOrder(container: container) }
    }

    // MARK: - Stacking

    /// Later siblings (the workspace card, docked panels) would cover the peek
    /// and take its clicks; reorder rather than re-add, which drops constraints.
    private func liftAboveWorkspace(container: NSView, sidebar: SidebarView) {
        let subviews = container.subviews
        savedOrder = Dictionary(
            uniqueKeysWithValues: subviews.enumerated().map { (ObjectIdentifier($1), $0) })
        var order = savedOrder
        let top = subviews.count
        order[ObjectIdentifier(backdrop)] = top
        order[ObjectIdentifier(sidebar)] = top + 1
        Self.sort(container, by: order)
    }

    private func restoreOrder(container: NSView) {
        var order: [ObjectIdentifier: Int] = [:]
        let base = savedOrder.count
        for (index, view) in container.subviews.enumerated() {
            let id = ObjectIdentifier(view)
            // Views added during the peek stay above the ones it found.
            order[id] = savedOrder[id] ?? base + index
        }
        Self.sort(container, by: order)
    }

    private static func sort(_ container: NSView, by order: [ObjectIdentifier: Int]) {
        let box = Unmanaged.passRetained(SubviewOrder(order))
        defer { box.release() }
        container.sortSubviews({ a, b, context in
            guard let context else { return .orderedSame }
            let order = Unmanaged<SubviewOrder>.fromOpaque(context).takeUnretainedValue().index
            let lhs = order[ObjectIdentifier(a)] ?? Int.max
            let rhs = order[ObjectIdentifier(b)] ?? Int.max
            return lhs < rhs ? .orderedAscending : lhs > rhs ? .orderedDescending : .orderedSame
        }, context: box.toOpaque())
    }

    // MARK: - Dismissal

    private func installDismissTriggers() {
        let keys = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            let isEscape = event.keyCode == 53
            let windowNumber = event.windowNumber
            let consumed = MainActor.assumeIsolated { () -> Bool in
                guard let self, isEscape, windowNumber == self.window?.windowNumber else { return false }
                self.onDismissRequested?()
                return true
            }
            return consumed ? nil : event
        }
        let clicks = NSEvent.addLocalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]
        ) { [weak self] event in
            let windowNumber = event.windowNumber
            let location = event.locationInWindow
            MainActor.assumeIsolated {
                guard let self, windowNumber == self.window?.windowNumber, let sidebar = self.sidebar
                else { return }
                if !sidebar.bounds.contains(sidebar.convert(location, from: nil)) {
                    self.onDismissRequested?()
                }
            }
            return event
        }
        monitors = [keys, clicks].compactMap { $0 }
        resignObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didResignKeyNotification, object: window, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.onDismissRequested?() }
        }
    }

    private func removeDismissTriggers() {
        monitors.forEach(NSEvent.removeMonitor)
        monitors = []
        resignObserver.map(NotificationCenter.default.removeObserver)
        resignObserver = nil
    }
}

private final class SubviewOrder {
    let index: [ObjectIdentifier: Int]
    init(_ index: [ObjectIdentifier: Int]) { self.index = index }
}
#endif
