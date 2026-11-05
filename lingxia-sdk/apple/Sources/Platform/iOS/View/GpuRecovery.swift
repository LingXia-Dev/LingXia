import Foundation
#if os(iOS)
import UIKit
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
        let panel = UIView()
        panel.backgroundColor = .systemBackground
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.accessibilityIdentifier = "lx-recovery"
        let text = UILabel()
        text.text = L10n.string("lx_gpu_recovery_\(Self.messages[Int(message)])")
        text.numberOfLines = 0
        text.textAlignment = .center
        text.font = .preferredFont(forTextStyle: .body)
        text.adjustsFontForContentSizeCategory = true
        text.accessibilityIdentifier = "lx-recovery-message"
        let stack = UIStackView(arrangedSubviews: [text])
        stack.axis = .vertical
        stack.spacing = 12
        stack.translatesAutoresizingMaskIntoConstraints = false
        if Self.actions.indices.contains(Int(action)) {
            let button = UIButton(type: .system)
            button.setTitle(L10n.string("lx_gpu_recovery_\(Self.actions[Int(action)])"), for: .normal)
            button.accessibilityIdentifier = "lx-recovery-action"
            button.addTarget(self, action: #selector(recoveryAction(_:)), for: .touchUpInside)
            stack.addArrangedSubview(button)
        }
        panel.addSubview(stack)
        addSubview(panel)
        let guide = safeAreaLayoutGuide
        var constraints = [
            panel.leadingAnchor.constraint(equalTo: leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: trailingAnchor),
            panel.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.centerXAnchor.constraint(equalTo: guide.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: panel.centerYAnchor),
            stack.widthAnchor.constraint(lessThanOrEqualTo: guide.widthAnchor, constant: -32),
        ]
        // A banner leaves a retained frame visible above it; otherwise nothing of the page shows.
        constraints.append(covers
            ? panel.topAnchor.constraint(equalTo: topAnchor)
            : panel.heightAnchor.constraint(equalTo: stack.heightAnchor, constant: 32))
        NSLayoutConstraint.activate(constraints)
        recoveryPanel = panel
        recoveryCovers = covers
        UIAccessibility.post(notification: .screenChanged, argument: text)
    }

    @objc private func recoveryAction(_ sender: UIButton) {
        guard sender.isEnabled else { return }
        sender.isEnabled = false
        sender.setTitle(L10n.string("lx_gpu_recovery_retrying"), for: .normal)
        gpuPageRetry(token)
    }
}
#endif
