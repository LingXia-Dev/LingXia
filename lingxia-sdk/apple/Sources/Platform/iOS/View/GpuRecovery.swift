import Foundation
#if os(iOS)
import UIKit
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
        let panel = UIView()
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
        // The button row centres as a whole under the message, not stretched to its width.
        stack.alignment = .center
        stack.translatesAutoresizingMaskIntoConstraints = false
        let buttons = UIStackView()
        buttons.axis = .horizontal
        buttons.spacing = 24
        buttons.alignment = .center
        if back {
            let button = UIButton(type: .system)
            button.setTitle(L10n.string("lx_gpu_recovery_back"), for: .normal)
            button.accessibilityIdentifier = "lx-recovery-back"
            Self.touchTarget(button)
            button.addTarget(self, action: #selector(recoveryBack(_:)), for: .touchUpInside)
            buttons.addArrangedSubview(button)
        }
        if Self.actions.indices.contains(Int(action)) {
            let button = UIButton(type: .system)
            button.setTitle(L10n.string("lx_gpu_recovery_\(Self.actions[Int(action)])"), for: .normal)
            button.accessibilityIdentifier = "lx-recovery-action"
            button.tag = Int(action)
            Self.touchTarget(button)
            button.addTarget(self, action: #selector(recoveryAction(_:)), for: .touchUpInside)
            buttons.addArrangedSubview(button)
        }
        if !buttons.arrangedSubviews.isEmpty {
            stack.addArrangedSubview(buttons)
        }
        panel.addSubview(stack)
        addSubview(panel)
        let guide = safeAreaLayoutGuide
        // The page's content area: below the bars that cover the top edge.
        let content = UILayoutGuide()
        panel.addLayoutGuide(content)
        var constraints = [
            content.topAnchor.constraint(equalTo: topAnchor, constant: max(chromeTop, safeAreaInsets.top)),
            content.bottomAnchor.constraint(equalTo: guide.bottomAnchor),
            panel.leadingAnchor.constraint(equalTo: leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: trailingAnchor),
            panel.bottomAnchor.constraint(equalTo: bottomAnchor),
            stack.centerXAnchor.constraint(equalTo: guide.centerXAnchor),
            stack.widthAnchor.constraint(lessThanOrEqualTo: guide.widthAnchor, constant: -32),
        ]
        // A banner leaves a retained frame visible above it, its buttons clear of the home
        // indicator; otherwise nothing of the page shows.
        constraints += covers
            ? [panel.topAnchor.constraint(equalTo: topAnchor),
               stack.centerYAnchor.constraint(equalTo: content.centerYAnchor)]
            : [stack.bottomAnchor.constraint(equalTo: guide.bottomAnchor, constant: -16),
               panel.topAnchor.constraint(equalTo: stack.topAnchor, constant: -16)]
        NSLayoutConstraint.activate(constraints)
        recoveryPanel = panel
        recoveryCovers = covers
        recolorRecovery()
        UIAccessibility.post(notification: .screenChanged, argument: text)
    }

    /// A text button at least 44 pt each way, as the platform asks of any tap target.
    private static func touchTarget(_ button: UIButton) {
        // Message 17 (body), buttons 16 medium (callout): the same panel on every platform.
        button.titleLabel?.font = UIFontMetrics(forTextStyle: .callout)
            .scaledFont(for: .systemFont(ofSize: 16, weight: .medium))
        button.titleLabel?.adjustsFontForContentSizeCategory = true
        button.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
            button.widthAnchor.constraint(greaterThanOrEqualToConstant: 44),
        ])
    }

    /// The panel sits on the page's own canvas, in its text and accent colours.
    func recolorRecovery() {
        guard let panel = recoveryPanel else { return }
        panel.backgroundColor = hostChrome?.canvas ?? .systemBackground
        panel.tintColor = hostChrome?.accent
        func walk(_ view: UIView) {
            if let label = view as? UILabel { label.textColor = hostChrome?.content ?? .label }
            view.subviews.forEach(walk)
        }
        walk(panel)
    }

    @objc private func recoveryAction(_ sender: UIButton) {
        guard sender.isEnabled else { return }
        sender.isEnabled = false
        if sender.tag != Self.keepWaiting {
            sender.setTitle(L10n.string("lx_gpu_recovery_retrying"), for: .normal)
        }
        gpuPageRetry(token)
    }

    @objc private func recoveryBack(_ sender: UIButton) {
        guard sender.isEnabled else { return }
        sender.isEnabled = false
        if !gpuPageBack(token) { sender.isEnabled = true }
    }
}
#endif
