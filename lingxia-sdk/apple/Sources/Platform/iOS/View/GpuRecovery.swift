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
    private static let hairline = "lx-recovery-line"
    private static let detailId = "lx-recovery-detail"

    /// `detail`, when not empty, is the error itself (a dev build): small, monospaced and
    /// selectable under the message.
    func showRecovery(message: Int32, action: Int32, covers: Bool, back: Bool, detail: String = "") {
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
        var rows: [UIView] = [text]
        if !detail.isEmpty {
            let error = UITextView()
            error.text = detail
            error.isEditable = false
            error.isSelectable = true
            error.isScrollEnabled = false
            error.backgroundColor = .clear
            error.textAlignment = .center
            error.font = UIFontMetrics(forTextStyle: .footnote)
                .scaledFont(for: .monospacedSystemFont(ofSize: 12, weight: .regular))
            error.adjustsFontForContentSizeCategory = true
            error.textContainer.maximumNumberOfLines = 6
            error.textContainer.lineBreakMode = .byTruncatingTail
            error.textContainerInset = .zero
            error.accessibilityIdentifier = Self.detailId
            rows.append(error)
        }
        let stack = UIStackView(arrangedSubviews: rows)
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
        var line: UIView?
        if !covers {
            // A banner over the kept page is a plate of its own, set off by the bars' hairline.
            let hairline = UIView()
            hairline.translatesAutoresizingMaskIntoConstraints = false
            hairline.accessibilityIdentifier = Self.hairline
            panel.addSubview(hairline)
            line = hairline
        }
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
        if let line {
            constraints += [
                line.topAnchor.constraint(equalTo: panel.topAnchor),
                line.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
                line.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
                line.heightAnchor.constraint(equalToConstant: 1 / max(traitCollection.displayScale, 1)),
            ]
        }
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

    /// The panel sits on the page's own canvas, a banner on the bars' plate; text and accent
    /// colours are the page's.
    func recolorRecovery() {
        guard let panel = recoveryPanel else { return }
        panel.backgroundColor = recoveryCovers
            ? hostChrome?.canvas ?? .systemBackground
            : hostChrome?.bar ?? .secondarySystemBackground
        panel.subviews.first { $0.accessibilityIdentifier == Self.hairline }?.backgroundColor =
            hostChrome?.barLine ?? .separator
        panel.tintColor = hostChrome?.accent
        func walk(_ view: UIView) {
            if let label = view as? UILabel { label.textColor = hostChrome?.content ?? .label }
            view.subviews.forEach(walk)
        }
        walk(panel)
        if let error = panel.subviews.first?.subviews.compactMap({ $0 as? UITextView }).first {
            error.textColor = (hostChrome?.content ?? .label).withAlphaComponent(0.72)
        }
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
