import Foundation
#if os(macOS)
import AppKit
import CLingXiaRustAPI

/// Host-owned recovery chrome; page content cannot replace its action.
extension GpuPageView {
    /// The core's message codes, then its action codes (0 retry, 1 reload).
    private static let messages = ["update_required", "session_failed", "update_failed", "display_failed"]
    private static let actions = ["retry", "reload"]

    func showRecovery(message: Int32, action: Int32, covers: Bool) {
        recoveryPanel?.removeFromSuperview()
        recoveryPanel = nil
        recoveryCovers = false
        guard Self.messages.indices.contains(Int(message)) else { return }
        let panel = NSView()
        panel.wantsLayer = true
        panel.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        panel.translatesAutoresizingMaskIntoConstraints = false
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
        if Self.actions.indices.contains(Int(action)) {
            let button = NSButton(
                title: L10n.string("lx_gpu_recovery_\(Self.actions[Int(action)])"), target: self,
                action: #selector(recoveryAction(_:)))
            button.bezelStyle = .rounded
            button.setAccessibilityIdentifier("lx-recovery-action")
            stack.addArrangedSubview(button)
        }
        panel.addSubview(stack)
        addSubview(panel)
        var constraints = [
            panel.leadingAnchor.constraint(equalTo: leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: trailingAnchor),
            panel.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.centerXAnchor.constraint(equalTo: panel.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: panel.centerYAnchor),
            stack.widthAnchor.constraint(lessThanOrEqualTo: panel.widthAnchor, constant: -32),
            text.widthAnchor.constraint(lessThanOrEqualToConstant: 360),
        ]
        // A banner leaves a retained frame visible above it; otherwise nothing of the page shows.
        constraints.append(covers
            ? panel.topAnchor.constraint(equalTo: topAnchor)
            : panel.heightAnchor.constraint(equalTo: stack.heightAnchor, constant: 32))
        NSLayoutConstraint.activate(constraints)
        recoveryPanel = panel
        recoveryCovers = covers
        NSAccessibility.post(element: panel, notification: .layoutChanged)
    }

    @objc private func recoveryAction(_ sender: NSButton) {
        guard sender.isEnabled else { return }
        sender.isEnabled = false
        sender.title = L10n.string("lx_gpu_recovery_retrying")
        gpuPageRetry(token)
    }
}
#endif
