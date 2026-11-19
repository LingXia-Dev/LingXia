import Foundation
#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Host-owned recovery chrome; page content cannot replace its action.
extension GpuPageView {
    /// The core's message codes, then its action codes (0 retry, 1 reload, 2 keep waiting).
    private static let messages = ["update_required", "session_failed", "update_failed", "display_failed", "slow_load"]
    private static let actions = ["retry", "reload", "keep_waiting"]
    private static let keepWaiting = 2

    func showRecovery(message: Int32, action: Int32, covers: Bool, back: Bool) {
        recoveryPanel?.removeFromSuperview()
        recoveryPanel = nil
        recoveryCovers = false
        guard Self.messages.indices.contains(Int(message)) else { return }
        let panel = NSView()
        panel.wantsLayer = true
        panel.setAccessibilityElement(true)
        panel.setAccessibilityRole(.group)
        panel.setAccessibilityIdentifier("lx-recovery")
        let text = NSTextField(wrappingLabelWithString: L10n.string("lx_gpu_recovery_\(Self.messages[Int(message)])"))
        text.alignment = .center
        text.font = .systemFont(ofSize: 15)
        text.setAccessibilityIdentifier("lx-recovery-message")
        let stack = NSStackView(views: [text])
        stack.orientation = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        let buttons = NSStackView()
        buttons.orientation = .horizontal
        buttons.spacing = 24
        if back {
            let button = NSButton(
                title: L10n.string("lx_gpu_recovery_back"), target: self, action: #selector(recoveryBack(_:)))
            button.isBordered = false
            button.setAccessibilityIdentifier("lx-recovery-back")
            buttons.addArrangedSubview(button)
        }
        if Self.actions.indices.contains(Int(action)) {
            let button = NSButton(
                title: L10n.string("lx_gpu_recovery_\(Self.actions[Int(action)])"), target: self,
                action: #selector(recoveryAction(_:)))
            button.isBordered = false
            button.setAccessibilityIdentifier("lx-recovery-action")
            button.tag = Int(action)
            buttons.addArrangedSubview(button)
        }
        if !buttons.arrangedSubviews.isEmpty {
            stack.addArrangedSubview(buttons)
        }
        panel.addSubview(stack)
        addSubview(panel)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: panel.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: panel.centerYAnchor),
            stack.widthAnchor.constraint(lessThanOrEqualTo: panel.widthAnchor, constant: -32),
            text.widthAnchor.constraint(lessThanOrEqualToConstant: 360),
        ])
        recoveryPanel = panel
        recoveryCovers = covers
        recolorRecovery()
        layoutRecovery()
        NSAccessibility.post(element: panel, notification: .layoutChanged)
    }

    /// Placed by frame: the page's bounds may be a simulated viewport, which constraints to
    /// its edges do not follow. A banner leaves a retained frame visible above it; otherwise
    /// nothing of the page shows.
    func layoutRecovery() {
        guard let panel = recoveryPanel else { return }
        var frame = bounds
        if !recoveryCovers, let stack = panel.subviews.first {
            frame.size.height = min(bounds.height, stack.fittingSize.height + 32)
            frame.origin.y = bounds.maxY - frame.height
        }
        panel.frame = frame
    }

    /// The panel sits on the page's own canvas, in its text colour, with text buttons in its
    /// accent: the same panel every platform draws.
    func recolorRecovery() {
        guard let panel = recoveryPanel else { return }
        panel.layer?.backgroundColor = (chrome?.canvas ?? .windowBackgroundColor).cgColor
        func walk(_ view: NSView) {
            if let label = view as? NSTextField { label.textColor = chrome?.content ?? .labelColor }
            if let button = view as? NSButton { button.contentTintColor = chrome?.accent ?? .controlAccentColor }
            view.subviews.forEach(walk)
        }
        walk(panel)
    }

    @objc private func recoveryAction(_ sender: NSButton) {
        guard sender.isEnabled else { return }
        sender.isEnabled = false
        if sender.tag != Self.keepWaiting {
            sender.title = L10n.string("lx_gpu_recovery_retrying")
        }
        gpuPageRetry(token)
    }

    @objc private func recoveryBack(_ sender: NSButton) {
        guard sender.isEnabled else { return }
        sender.isEnabled = false
        if !gpuPageBack(token) { sender.isEnabled = true }
    }
}
#endif
