#if os(macOS)
import SwiftUI
import Foundation
import CLingXiaRustAPI

/// NSWindow class for LxApp Tab mode
class LxAppWindow: NSWindow {
    nonisolated(unsafe) private var titlebarObserver: Any?
    private var lastTrafficLightContainerHeight: CGFloat?
    var trafficLightsHidden: Bool = false {
        didSet {
            applyTrafficLightVisibility()
        }
    }
    var trafficLightCenterYFromTop: CGFloat = 19 {
        didSet {
            guard abs(oldValue - trafficLightCenterYFromTop) > 0.5 else { return }
            adjustTrafficLightPositions()
        }
    }

    func effectiveTrafficLightCenterYFromTop() -> CGFloat {
        guard let container = standardWindowButton(.closeButton)?.superview,
              let button = standardWindowButton(.closeButton),
              container.frame.height > 0,
              button.frame.height > 0 else {
            return trafficLightCenterYFromTop
        }
        let maxCenterYFromTop = container.frame.height - button.frame.height / 2
        return min(trafficLightCenterYFromTop, maxCenterYFromTop)
    }

    override init(contentRect: NSRect, styleMask style: NSWindow.StyleMask, backing backingStoreType: NSWindow.BackingStoreType, defer flag: Bool) {
        super.init(contentRect: contentRect, styleMask: style, backing: backingStoreType, defer: flag)
    }

    func setTrafficLightsHidden(_ hidden: Bool) {
        trafficLightsHidden = hidden
    }

    func configureForTabStyle() {
        styleMask.insert(.fullSizeContentView)
        titlebarAppearsTransparent = true
        titleVisibility = .hidden
        isMovableByWindowBackground = false
        backgroundColor = .clear

        if let observer = titlebarObserver {
            NotificationCenter.default.removeObserver(observer)
            titlebarObserver = nil
        }

        if let button = standardWindowButton(.closeButton), let container = button.superview {
            lastTrafficLightContainerHeight = nil
            container.postsFrameChangedNotifications = true
            titlebarObserver = NotificationCenter.default.addObserver(
                forName: NSView.frameDidChangeNotification, object: container, queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.adjustTrafficLightPositionsIfTitlebarHeightChanged()
                }
            }
        }

        adjustTrafficLightPositions()
        applyTrafficLightVisibility()
        DispatchQueue.main.async { [weak self] in
            self?.adjustTrafficLightPositions()
            self?.applyTrafficLightVisibility()
        }
    }

    private func applyTrafficLightVisibility() {
        for type: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
            standardWindowButton(type)?.isHidden = trafficLightsHidden
        }
    }

    private func adjustTrafficLightPositionsIfTitlebarHeightChanged() {
        guard let container = standardWindowButton(.closeButton)?.superview,
              container.frame.height > 0 else { return }
        let height = container.frame.height
        guard lastTrafficLightContainerHeight == nil
            || abs((lastTrafficLightContainerHeight ?? 0) - height) > 0.5 else {
            return
        }
        adjustTrafficLightPositions()
    }

    private func adjustTrafficLightPositions() {
        guard !styleMask.contains(.fullScreen) else { return }
        guard let container = standardWindowButton(.closeButton)?.superview,
              container.frame.height > 0 else { return }

        let containerHeight = container.frame.height
        lastTrafficLightContainerHeight = containerHeight
        let targetCenterY = effectiveTrafficLightCenterYFromTop()
        for type: NSWindow.ButtonType in [.closeButton, .miniaturizeButton, .zoomButton] {
            guard let button = standardWindowButton(type), button.frame.height > 0 else { continue }
            let y = max(0, containerHeight - targetCenterY - button.frame.height / 2)
            if abs(button.frame.origin.y - y) > 0.5 {
                button.setFrameOrigin(NSPoint(x: button.frame.origin.x, y: y))
            }
        }
    }

    deinit {
        titlebarObserver.map(NotificationCenter.default.removeObserver)
    }

    override var canBecomeKey: Bool {
        return true
    }

    override var canBecomeMain: Bool {
        return true
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        let isDevtoolsShortcut = modifiers == [.command, .option]
            && (event.keyCode == 34 || event.charactersIgnoringModifiers?.lowercased() == "i")
        if isDevtoolsShortcut {
            if let controller = windowController as? LxAppShell,
               controller.toggleActiveDevTools() {
                return true
            }
        }

        // Backspace (keyCode 51) for back navigation
        if event.keyCode == 51 && event.modifierFlags.intersection(.deviceIndependentFlagsMask) == [] {
            // Don't intercept if typing in a native text field
            if let responder = firstResponder, responder is NSText {
                return super.performKeyEquivalent(with: event)
            }
            // Only navigate back when back button is available
            if let state = NavigationBarStateManager.shared.currentState, state.show_back_button {
                if let appId = LxAppTabManager.shared.activeTab?.appId {
                    let _ = onLxappEvent(appId, LxAppEvent.navigationClick, LxAppEvent.navigationActionBack)
                    return true
                }
            }
        }
        return super.performKeyEquivalent(with: event)
    }
}

#endif
